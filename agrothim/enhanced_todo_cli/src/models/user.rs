use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::{Validate, ValidationError};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Validate, Deserialize)]
pub struct StoreUserRequest {
    #[validate(length(
        min = 3,
        max = 50,
        message = "Username must be between 3 and 50 characters"
    ))]
    #[validate(regex(
        path = "USERNAME_REGEX",
        message = "Username can only contain letters, numbers, and underscores"
    ))]
    pub username: String,

    #[validate(email(message = "Invalid email format"))]
    pub email: String,

    #[validate(custom = "validate_password")]
    pub password: String,

    #[serde(skip)]
    pub password_hash: String,
}

#[derive(Debug, Validate, Deserialize)]
pub struct UpdateUserRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: Option<String>,

    #[validate(custom = "validate_password")]
    pub password: Option<String>,

    #[serde(skip)]
    pub password_hash: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

lazy_static::lazy_static! {
    static ref USERNAME_REGEX: Regex = Regex::new(r"^[a-zA-Z0-9_]+$").unwrap();
}

fn validate_password(password: &str) -> Result<(), ValidationError> {
    if password.len() < 8 {
        return Err(ValidationError::new("password_too_short"));
    }

    if password.len() > 128 {
        return Err(ValidationError::new("password_too_long"));
    }

    let has_letter = password.chars().any(|c| c.is_alphabetic());
    let has_number = password.chars().any(|c| c.is_numeric());

    if !has_letter || !has_number {
        return Err(ValidationError::new("password_complexity"));
    }

    Ok(())
}

impl StoreUserRequest {
    pub fn new(
        username: String,
        email: String,
        password: String,
    ) -> Result<Self, ValidationError> {
        Self::store(username, email, password)
    }

    pub fn store(
        username: String,
        email: String,
        password: String,
    ) -> Result<Self, ValidationError> {
        let password_hash = hash(&password, DEFAULT_COST)
            .map_err(|_| ValidationError::new("password_hash_failed"))?;

        let request = Self {
            username,
            email,
            password,
            password_hash,
        };

        request
            .validate()
            .map_err(|_| ValidationError::new("validation_failed"))?;
        Ok(request)
    }
}

impl UpdateUserRequest {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            email: None,
            password: None,
            password_hash: None,
        }
    }

    #[allow(dead_code)]
    pub fn email(mut self, email: String) -> Self {
        self.email = Some(email);
        self
    }

    #[allow(dead_code)]
    pub fn password(mut self, password: String) -> Self {
        self.password = Some(password);
        self
    }
}

impl User {
    #[allow(dead_code)]
    pub fn verify_password(&self, password: &str) -> bool {
        verify(password, &self.password_hash).unwrap_or(false)
    }

    #[allow(dead_code)]
    pub fn to_response(&self) -> UserResponse {
        UserResponse {
            id: self.id.clone(),
            username: self.username.clone(),
            email: self.email.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_create_user_request() {
        let request = StoreUserRequest::new(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "password123".to_string(),
        );
        assert!(request.is_ok());
    }

    #[test]
    fn test_invalid_username_too_short() {
        let request = StoreUserRequest::new(
            "ab".to_string(),
            "test@example.com".to_string(),
            "password123".to_string(),
        );
        assert!(request.is_err());
    }

    #[test]
    fn test_invalid_email_format() {
        let request = StoreUserRequest::new(
            "testuser".to_string(),
            "invalid-email".to_string(),
            "password123".to_string(),
        );
        assert!(request.is_err());
    }

    #[test]
    fn test_password_verification() {
        let request = StoreUserRequest::new(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "password123".to_string(),
        )
        .unwrap();

        let user = User {
            id: Uuid::new_v4(),
            username: request.username,
            email: request.email,
            password_hash: request.password_hash,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(user.verify_password("password123"));
        assert!(!user.verify_password("wrongpassword"));
    }
}
