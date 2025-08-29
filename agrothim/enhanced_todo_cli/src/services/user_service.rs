use std::sync::Arc;
use thiserror::Error;
use tracing::{error, info, warn};
use validator::Validate;
use uuid::Uuid;

use crate::{
    database::repositories::{UserRepository, UserRepositoryError},
    models::user::{StoreUserRequest, UpdateUserRequest, User, UserResponse},
};

#[derive(Error, Debug)]
pub enum UserServiceError {
    #[error("Validation error: {message}")]
    ValidationError { message: String },
    
    #[error("User not found")]
    UserNotFound,
    
    #[error("Authentication failed")]
    AuthenticationFailed,
    
    #[error("Username already exists: {username}")]
    UsernameExists { username: String },
    
    #[error("Email already exists: {email}")]
    EmailExists { email: String },
    
    #[error("Internal service error: {0}")]
    InternalError(#[from] anyhow::Error),
    
    #[error("Repository error: {0}")]
    RepositoryError(#[from] UserRepositoryError),
}

pub struct UserService {
    user_repository: Arc<dyn UserRepository>,
}

impl UserService {
    #[allow(dead_code)]
    pub fn new(user_repository: Arc<dyn UserRepository>) -> Self {
        Self { user_repository }
    }

    /// Register a new user with comprehensive validation
    #[allow(dead_code)]
    pub async fn register(&self, request: StoreUserRequest) -> Result<UserResponse, UserServiceError> {
        info!("Attempting to register user: {}", request.username);
        
        // Validate input data
        self.validate_registration_request(&request).await?;
        
        // Check for existing username and email
        self.check_user_uniqueness(&request).await?;
        
        // Create user via repository
        let user = self.user_repository
            .store(request)
            .await
            .map_err(|e| {
                error!("Failed to create user in repository: {}", e);
                UserServiceError::RepositoryError(e)
            })?;

        info!("Successfully registered user with ID: {}", user.id);
        Ok(user.to_response())
    }

    /// Authenticate user with username/email and password
    #[allow(dead_code)]
    pub async fn authenticate(&self, identifier: &str, password: &str) -> Result<UserResponse, UserServiceError> {
        info!("Authentication attempt for user: {}", identifier);
        
        if identifier.is_empty() || password.is_empty() {
            warn!("Authentication failed: empty credentials");
            return Err(UserServiceError::AuthenticationFailed);
        }

        // Try to find user by username or email
        let user = self.find_user_by_identifier(identifier).await?;
        
        // Verify password
        if !user.verify_password(password) {
            warn!("Authentication failed: invalid password for user {}", identifier);
            return Err(UserServiceError::AuthenticationFailed);
        }

        info!("Successfully authenticated user: {}", user.username);
        Ok(user.to_response())
    }

    /// Get user profile by ID
    #[allow(dead_code)]
    pub async fn get_profile(&self, user_id: &Uuid) -> Result<UserResponse, UserServiceError> {
        let user = self.user_repository
            .find_by_id(user_id)
            .await?
            .ok_or(UserServiceError::UserNotFound)?;

        Ok(user.to_response())
    }

    /// Update user profile
    #[allow(dead_code)]
    pub async fn update_profile(
        &self, 
        user_id: &Uuid, 
        updates: UpdateUserRequest
    ) -> Result<UserResponse, UserServiceError> {
        info!("Updating profile for user ID: {}", user_id);
        
        // Validate updates
        updates.validate()
            .map_err(|e| UserServiceError::ValidationError { 
                message: format!("Profile update validation failed: {}", e) 
            })?;

        // Check if email is being updated and if it's already taken
        if let Some(ref email) = updates.email {
            if self.user_repository.exists_by_email(email).await? {
                return Err(UserServiceError::EmailExists { 
                    email: email.clone() 
                });
            }
        }

        let updated_user = self.user_repository
            .update(user_id, updates)
            .await?;

        info!("Successfully updated profile for user: {}", updated_user.username);
        Ok(updated_user.to_response())
    }

    /// Check if username exists
    #[allow(dead_code)]
    pub async fn username_exists(&self, username: &str) -> Result<bool, UserServiceError> {
        Ok(self.user_repository.exists_by_username(username).await?)
    }

    /// Check if email exists
    #[allow(dead_code)]
    pub async fn email_exists(&self, email: &str) -> Result<bool, UserServiceError> {
        Ok(self.user_repository.exists_by_email(email).await?)
    }

    /// Delete user account
    #[allow(dead_code)]
    pub async fn delete_account(&self, user_id: &Uuid) -> Result<bool, UserServiceError> {
        info!("Deleting user account: {}", user_id);
        
        let deleted = self.user_repository.delete(user_id).await?;
        
        if deleted {
            info!("Successfully deleted user account: {}", user_id);
        } else {
            warn!("User account not found for deletion: {}", user_id);
        }
        
        Ok(deleted)
    }

    // Private helper methods

    /// Validate registration request
    async fn validate_registration_request(&self, request: &StoreUserRequest) -> Result<(), UserServiceError> {
        request.validate()
            .map_err(|e| UserServiceError::ValidationError { 
                message: format!("Registration validation failed: {}", e) 
            })?;
        Ok(())
    }

    /// Check if username and email are unique
    async fn check_user_uniqueness(&self, request: &StoreUserRequest) -> Result<(), UserServiceError> {
        // Check username uniqueness
        if self.user_repository.exists_by_username(&request.username).await? {
            return Err(UserServiceError::UsernameExists { 
                username: request.username.clone() 
            });
        }

        // Check email uniqueness
        if self.user_repository.exists_by_email(&request.email).await? {
            return Err(UserServiceError::EmailExists { 
                email: request.email.clone() 
            });
        }

        Ok(())
    }

    /// Find user by username or email
    async fn find_user_by_identifier(&self, identifier: &str) -> Result<User, UserServiceError> {
        // Try username first
        if let Some(user) = self.user_repository.find_by_username(identifier).await? {
            return Ok(user);
        }

        // Try email if username not found
        if let Some(user) = self.user_repository.find_by_email(identifier).await? {
            return Ok(user);
        }

        Err(UserServiceError::UserNotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::user::StoreUserRequest;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;

    // Mock repository for testing
    struct MockUserRepository {
        users: Arc<Mutex<HashMap<Uuid, User>>>,
        usernames: Arc<Mutex<HashMap<String, Uuid>>>,
        emails: Arc<Mutex<HashMap<String, Uuid>>>,
    }

    impl MockUserRepository {
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
            self.usernames.lock().unwrap().insert(request.username, user_id);
            self.emails.lock().unwrap().insert(request.email, user_id);

            Ok(user)
        }

        async fn find_by_id(&self, id: &Uuid) -> Result<Option<User>, UserRepositoryError> {
            Ok(self.users.lock().unwrap().get(id).cloned())
        }

        async fn find_by_username(&self, username: &str) -> Result<Option<User>, UserRepositoryError> {
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

        async fn update(&self, id: &Uuid, updates: UpdateUserRequest) -> Result<User, UserRepositoryError> {
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
    async fn test_user_registration_success() {
        let repo = Arc::new(MockUserRepository::new());
        let service = UserService::new(repo);

        let request = StoreUserRequest::new(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "password123".to_string(),
        ).unwrap();

        let result = service.register(request).await;
        assert!(result.is_ok());
        
        let user_response = result.unwrap();
        assert_eq!(user_response.username, "testuser");
        assert_eq!(user_response.email, "test@example.com");
    }

    #[tokio::test]
    async fn test_user_authentication_success() {
        let repo = Arc::new(MockUserRepository::new());
        let service = UserService::new(repo);

        // Register user first
        let request = StoreUserRequest::new(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "password123".to_string(),
        ).unwrap();
        service.register(request).await.unwrap();

        // Test authentication
        let result = service.authenticate("testuser", "password123").await;
        assert!(result.is_ok());
        
        let user_response = result.unwrap();
        assert_eq!(user_response.username, "testuser");
    }

    #[tokio::test]
    async fn test_authentication_failure() {
        let repo = Arc::new(MockUserRepository::new());
        let service = UserService::new(repo);

        let result = service.authenticate("nonexistent", "password").await;
        assert!(matches!(result, Err(UserServiceError::UserNotFound)));
    }
}
