use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{
    decode, encode, Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::{
    models::user::UserResponse,
    services::{UserService, UserServiceError},
};

#[derive(Error, Debug)]
pub enum AuthServiceError {
    #[error("Authentication failed: invalid credentials")]
    AuthenticationFailed,

    #[error("Invalid or expired token")]
    InvalidToken,

    #[error("Session not found")]
    SessionNotFound,

    #[error("Session expired")]
    SessionExpired,

    #[error("User service error: {0}")]
    UserServiceError(#[from] UserServiceError),

    #[error("Token creation failed: {0}")]
    TokenCreationFailed(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Internal error: {0}")]
    InternalError(#[from] anyhow::Error),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,      // Subject (user ID)
    pub username: String, // Username for convenience
    pub email: String,    // Email for convenience
    pub iat: i64,         // Issued at
    pub exp: i64,         // Expiration time
    pub jti: String,      // JWT ID for token revocation
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Session {
    pub user_id: Uuid,
    pub username: String,
    pub email: String,
    pub token: String,
    pub refresh_token: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResponse {
    pub user: UserResponse,
    pub token: String,
    pub refresh_token: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenRefreshResponse {
    pub token: String,
    pub refresh_token: String,
    pub expires_at: DateTime<Utc>,
}

pub struct AuthService {
    user_service: Arc<UserService>,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    session_file_path: PathBuf,
    token_expiry_duration: Duration,
    refresh_token_expiry_duration: Duration,
}

impl AuthService {
    #[allow(dead_code)]
    pub fn new(
        user_service: Arc<UserService>,
        jwt_secret: &str,
        session_dir: Option<PathBuf>,
    ) -> Result<Self, AuthServiceError> {
        let encoding_key = EncodingKey::from_secret(jwt_secret.as_bytes());
        let decoding_key = DecodingKey::from_secret(jwt_secret.as_bytes());

        // Default session directory
        let session_dir = session_dir.unwrap_or_else(|| {
            let mut path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            path.push(".todo-cli");
            path
        });

        // Create session directory if it doesn't exist
        if !session_dir.exists() {
            fs::create_dir_all(&session_dir).context("Failed to create session directory")?;
        }

        let mut session_file_path = session_dir;
        session_file_path.push("session.json");

        Ok(Self {
            user_service,
            encoding_key,
            decoding_key,
            session_file_path,
            token_expiry_duration: Duration::hours(24), // 24 hours for access token
            refresh_token_expiry_duration: Duration::days(30), // 30 days for refresh token
        })
    }

    /// Login with username/email and password
    pub async fn login(
        &self,
        identifier: &str,
        password: &str,
    ) -> Result<LoginResponse, AuthServiceError> {
        info!("Login attempt for user: {}", identifier);

        // Authenticate user via UserService
        let user = self
            .user_service
            .authenticate(identifier, password)
            .await
            .map_err(|e| match e {
                UserServiceError::AuthenticationFailed => AuthServiceError::AuthenticationFailed,
                UserServiceError::UserNotFound => AuthServiceError::AuthenticationFailed,
                other => AuthServiceError::UserServiceError(other),
            })?;

        // Generate tokens
        let (token, refresh_token, expires_at) = self.generate_tokens(&user)?;

        // Create and save session
        let session = Session {
            user_id: user.id,
            username: user.username.clone(),
            email: user.email.clone(),
            token: token.clone(),
            refresh_token: refresh_token.clone(),
            created_at: Utc::now(),
            expires_at,
            last_accessed: Utc::now(),
        };

        self.save_session(&session)?;

        info!("User {} logged in successfully", user.username);

        Ok(LoginResponse {
            user,
            token,
            refresh_token,
            expires_at,
        })
    }

    /// Logout and clear session
    pub async fn logout(&self) -> Result<(), AuthServiceError> {
        info!("Logging out user");

        if self.session_file_path.exists() {
            fs::remove_file(&self.session_file_path).context("Failed to remove session file")?;
            info!("Session cleared successfully");
        }

        Ok(())
    }

    /// Validate token and return user information
    pub async fn validate_token(&self, token: &str) -> Result<UserResponse, AuthServiceError> {
        debug!("Validating token");

        // Decode and validate JWT
        let token_data = self.decode_token(token)?;
        let claims = token_data.claims;

        // Check if token is still valid
        let now = Utc::now().timestamp();
        if claims.exp < now {
            warn!("Token expired for user: {}", claims.username);
            return Err(AuthServiceError::InvalidToken);
        }

        // Parse user ID
        let user_id = Uuid::parse_str(&claims.sub).map_err(|_| AuthServiceError::InvalidToken)?;

        // Get current user data (for up-to-date information)
        let user = self.user_service.get_profile(&user_id).await?;

        debug!("Token validated successfully for user: {}", user.username);
        Ok(user)
    }

    /// Refresh access token using refresh token
    #[allow(dead_code)]
    pub async fn refresh_token(
        &self,
        refresh_token: &str,
    ) -> Result<TokenRefreshResponse, AuthServiceError> {
        debug!("Refreshing token");

        // Load current session
        let mut session = self.load_session()?;

        // Verify refresh token matches stored one
        if session.refresh_token != refresh_token {
            warn!("Invalid refresh token provided");
            return Err(AuthServiceError::InvalidToken);
        }

        // Check if session is expired
        if session.expires_at <= Utc::now() {
            warn!("Refresh token expired for user: {}", session.username);
            self.logout().await?; // Clear expired session
            return Err(AuthServiceError::SessionExpired);
        }

        // Get user for token generation
        let user = UserResponse {
            id: session.user_id,
            username: session.username.clone(),
            email: session.email.clone(),
            created_at: Utc::now(), // This will be ignored in token generation
            updated_at: Utc::now(), // This will be ignored in token generation
        };

        // Generate new tokens
        let (new_token, new_refresh_token, new_expires_at) = self.generate_tokens(&user)?;

        // Update session
        session.token = new_token.clone();
        session.refresh_token = new_refresh_token.clone();
        session.expires_at = new_expires_at;
        session.last_accessed = Utc::now();

        self.save_session(&session)?;

        info!(
            "Token refreshed successfully for user: {}",
            session.username
        );

        Ok(TokenRefreshResponse {
            token: new_token,
            refresh_token: new_refresh_token,
            expires_at: new_expires_at,
        })
    }

    /// Get current session if exists and valid
    pub async fn get_current_session(&self) -> Result<Option<UserResponse>, AuthServiceError> {
        if !self.session_file_path.exists() {
            return Ok(None);
        }

        match self.load_session() {
            Ok(session) => {
                // Check if session is expired
                if session.expires_at <= Utc::now() {
                    debug!("Session expired, clearing it");
                    self.logout().await?;
                    return Ok(None);
                }

                // Validate token
                match self.validate_token(&session.token).await {
                    Ok(user) => {
                        // Update last accessed time
                        let mut updated_session = session;
                        updated_session.last_accessed = Utc::now();
                        self.save_session(&updated_session)?;

                        debug!("Current session valid for user: {}", user.username);
                        Ok(Some(user))
                    }
                    Err(_) => {
                        debug!("Invalid session token, clearing session");
                        self.logout().await?;
                        Ok(None)
                    }
                }
            }
            Err(_) => {
                debug!("Failed to load session");
                Ok(None)
            }
        }
    }

    /// Check if user is currently authenticated
    #[allow(dead_code)]
    pub async fn is_authenticated(&self) -> bool {
        self.get_current_session()
            .await
            .map(|session| session.is_some())
            .unwrap_or(false)
    }

    /// Get current user if authenticated
    #[allow(dead_code)]
    pub async fn get_current_user(&self) -> Result<UserResponse, AuthServiceError> {
        self.get_current_session()
            .await?
            .ok_or(AuthServiceError::SessionNotFound)
    }

    // Private helper methods

    /// Generate JWT access token and refresh token
    #[allow(dead_code)]
    fn generate_tokens(
        &self,
        user: &UserResponse,
    ) -> Result<(String, String, DateTime<Utc>), AuthServiceError> {
        let now = Utc::now();
        let access_token_exp = now + self.token_expiry_duration;
        let refresh_token_exp = now + self.refresh_token_expiry_duration;

        // Generate access token
        let access_claims = Claims {
            sub: user.id.to_string(),
            username: user.username.clone(),
            email: user.email.clone(),
            iat: now.timestamp(),
            exp: access_token_exp.timestamp(),
            jti: Uuid::new_v4().to_string(),
        };

        let access_token = encode(&Header::default(), &access_claims, &self.encoding_key)
            .map_err(|e| AuthServiceError::TokenCreationFailed(e.to_string()))?;

        // Generate refresh token (longer lived)
        let refresh_claims = Claims {
            sub: user.id.to_string(),
            username: user.username.clone(),
            email: user.email.clone(),
            iat: now.timestamp(),
            exp: refresh_token_exp.timestamp(),
            jti: Uuid::new_v4().to_string(),
        };

        let refresh_token = encode(&Header::default(), &refresh_claims, &self.encoding_key)
            .map_err(|e| AuthServiceError::TokenCreationFailed(e.to_string()))?;

        Ok((access_token, refresh_token, refresh_token_exp))
    }

    /// Decode and validate JWT token
    #[allow(dead_code)]
    fn decode_token(&self, token: &str) -> Result<TokenData<Claims>, AuthServiceError> {
        let validation = Validation::new(Algorithm::HS256);

        decode::<Claims>(token, &self.decoding_key, &validation).map_err(|e| {
            debug!("Token decode failed: {}", e);
            AuthServiceError::InvalidToken
        })
    }

    /// Save session to file with secure permissions
    #[allow(dead_code)]
    fn save_session(&self, session: &Session) -> Result<(), AuthServiceError> {
        let json_data = serde_json::to_string_pretty(session)?;

        let mut file = fs::File::create(&self.session_file_path)?;
        file.write_all(json_data.as_bytes())?;
        file.flush()?;

        // Set restrictive permissions (readable only by owner)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = file.metadata()?.permissions();
            perms.set_mode(0o600); // Owner read/write only
            fs::set_permissions(&self.session_file_path, perms)?;
        }

        debug!("Session saved successfully");
        Ok(())
    }

    /// Load session from file
    #[allow(dead_code)]
    fn load_session(&self) -> Result<Session, AuthServiceError> {
        if !self.session_file_path.exists() {
            return Err(AuthServiceError::SessionNotFound);
        }

        let json_data = fs::read_to_string(&self.session_file_path)?;
        let session: Session = serde_json::from_str(&json_data)?;

        debug!("Session loaded successfully for user: {}", session.username);
        Ok(session)
    }
}

// Configuration for AuthService
#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub session_dir: Option<PathBuf>,
    pub token_expiry_hours: i64,
    pub refresh_token_expiry_days: i64,
}

#[allow(dead_code)]
impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: std::env::var("JWT_SECRET")
                .unwrap_or_else(|_| "default-secret-change-in-production".to_string()),
            session_dir: None,
            token_expiry_hours: 24,
            refresh_token_expiry_days: 30,
        }
    }
}

impl AuthService {
    /// Create AuthService with custom configuration
    #[allow(dead_code)]
    pub fn with_config(
        user_service: Arc<UserService>,
        config: AuthConfig,
    ) -> Result<Self, AuthServiceError> {
        let mut service = Self::new(user_service, &config.jwt_secret, config.session_dir)?;

        service.token_expiry_duration = Duration::hours(config.token_expiry_hours);
        service.refresh_token_expiry_duration = Duration::days(config.refresh_token_expiry_days);

        Ok(service)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::repositories::user_repository::{UserRepository, UserRepositoryError};
    use crate::models::user::{StoreUserRequest, UpdateUserRequest, User};
    use crate::services::user_service::UserService;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use tempfile::TempDir;

    // Mock UserRepository for testing
    struct MockUserRepository {
        users: Arc<Mutex<HashMap<Uuid, User>>>,
        usernames: Arc<Mutex<HashMap<String, Uuid>>>,
        emails: Arc<Mutex<HashMap<String, Uuid>>>,
    }

    impl MockUserRepository {
        #[allow(dead_code)]
        fn new() -> Self {
            Self {
                users: Arc::new(Mutex::new(HashMap::new())),
                usernames: Arc::new(Mutex::new(HashMap::new())),
                emails: Arc::new(Mutex::new(HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl UserRepository for MockUserRepository {
        async fn store(&self, request: StoreUserRequest) -> Result<User, UserRepositoryError> {
            let user_id = Uuid::new_v4();
            let now = chrono::Utc::now();

            let user = User {
                id: user_id,
                username: request.username.clone(),
                email: request.email.clone(),
                password_hash: request.password_hash,
                created_at: now,
                updated_at: now,
            };

            self.users.lock().unwrap().insert(user_id, user.clone());
            self.usernames
                .lock()
                .unwrap()
                .insert(request.username, user_id);
            self.emails.lock().unwrap().insert(request.email, user_id);

            Ok(user)
        }

        async fn find_by_id(&self, id: &Uuid) -> Result<Option<User>, UserRepositoryError> {
            Ok(self.users.lock().unwrap().get(id).cloned())
        }

        async fn find_by_username(
            &self,
            username: &str,
        ) -> Result<Option<User>, UserRepositoryError> {
            if let Some(user_id) = self.usernames.lock().unwrap().get(username) {
                Ok(self.users.lock().unwrap().get(user_id).cloned())
            } else {
                Ok(None)
            }
        }

        async fn find_by_email(&self, email: &str) -> Result<Option<User>, UserRepositoryError> {
            if let Some(user_id) = self.emails.lock().unwrap().get(email) {
                Ok(self.users.lock().unwrap().get(user_id).cloned())
            } else {
                Ok(None)
            }
        }

        async fn update(
            &self,
            id: &Uuid,
            updates: UpdateUserRequest,
        ) -> Result<User, UserRepositoryError> {
            let mut users = self.users.lock().unwrap();
            if let Some(user) = users.get_mut(id) {
                if let Some(email) = updates.email {
                    user.email = email;
                }
                user.updated_at = chrono::Utc::now();
                Ok(user.clone())
            } else {
                Err(UserRepositoryError::NotFound)
            }
        }

        async fn delete(&self, id: &Uuid) -> Result<bool, UserRepositoryError> {
            Ok(self.users.lock().unwrap().remove(id).is_some())
        }

        async fn exists_by_username(&self, username: &str) -> Result<bool, UserRepositoryError> {
            Ok(self.usernames.lock().unwrap().contains_key(username))
        }

        async fn exists_by_email(&self, email: &str) -> Result<bool, UserRepositoryError> {
            Ok(self.emails.lock().unwrap().contains_key(email))
        }
    }

    #[tokio::test]
    async fn test_login_success() {
        let temp_dir = TempDir::new().unwrap();
        let session_path = temp_dir.path().to_path_buf();

        let user_repo = Arc::new(MockUserRepository::new());
        let user_service = Arc::new(UserService::new(user_repo));
        let auth_service =
            AuthService::new(user_service.clone(), "test-secret", Some(session_path)).unwrap();

        // Create a test user
        let user_request = StoreUserRequest::new(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "password123".to_string(),
        )
        .unwrap();

        user_service.register(user_request).await.unwrap();

        // Test login
        let login_result = auth_service.login("testuser", "password123").await;
        assert!(login_result.is_ok());

        let login_response = login_result.unwrap();
        assert_eq!(login_response.user.username, "testuser");
        assert!(!login_response.token.is_empty());
        assert!(!login_response.refresh_token.is_empty());
    }

    #[tokio::test]
    async fn test_login_failure() {
        let temp_dir = TempDir::new().unwrap();
        let session_path = temp_dir.path().to_path_buf();

        let user_repo = Arc::new(MockUserRepository::new());
        let user_service = Arc::new(UserService::new(user_repo));
        let auth_service =
            AuthService::new(user_service, "test-secret", Some(session_path)).unwrap();

        // Test login with non-existent user
        let login_result = auth_service.login("nonexistent", "password").await;
        assert!(matches!(
            login_result,
            Err(AuthServiceError::AuthenticationFailed)
        ));
    }

    #[tokio::test]
    async fn test_token_validation() {
        let temp_dir = TempDir::new().unwrap();
        let session_path = temp_dir.path().to_path_buf();

        let user_repo = Arc::new(MockUserRepository::new());
        let user_service = Arc::new(UserService::new(user_repo));
        let auth_service =
            AuthService::new(user_service.clone(), "test-secret", Some(session_path)).unwrap();

        // Create and login user
        let user_request = StoreUserRequest::new(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "password123".to_string(),
        )
        .unwrap();

        user_service.register(user_request).await.unwrap();
        let login_response = auth_service.login("testuser", "password123").await.unwrap();

        // Validate token
        let validation_result = auth_service.validate_token(&login_response.token).await;
        assert!(validation_result.is_ok());

        let validated_user = validation_result.unwrap();
        assert_eq!(validated_user.username, "testuser");
    }

    #[tokio::test]
    async fn test_session_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let session_path = temp_dir.path().to_path_buf();

        let user_repo = Arc::new(MockUserRepository::new());
        let user_service = Arc::new(UserService::new(user_repo));

        // Create first auth service instance
        let auth_service1 = AuthService::new(
            user_service.clone(),
            "test-secret",
            Some(session_path.clone()),
        )
        .unwrap();

        // Create and login user
        let user_request = StoreUserRequest::new(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "password123".to_string(),
        )
        .unwrap();

        user_service.register(user_request).await.unwrap();
        auth_service1
            .login("testuser", "password123")
            .await
            .unwrap();

        // Create second auth service instance (simulating app restart)
        let auth_service2 =
            AuthService::new(user_service, "test-secret", Some(session_path)).unwrap();

        // Check if session persists
        let current_session = auth_service2.get_current_session().await.unwrap();
        assert!(current_session.is_some());
        assert_eq!(current_session.unwrap().username, "testuser");
    }
}
