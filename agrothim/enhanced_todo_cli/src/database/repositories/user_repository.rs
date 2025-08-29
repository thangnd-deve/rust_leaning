use anyhow::Result;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use thiserror::Error;
use uuid::Uuid;

use crate::models::user::{StoreUserRequest, UpdateUserRequest, User};

#[derive(Error, Debug)]
pub enum UserRepositoryError {
    #[error("User not found")]
    NotFound,
    #[error("Username already exists: {username}")]
    UsernameExists { username: String },
    #[error("Email already exists: {email}")]
    EmailExists { email: String },
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
}

/// User repository trait for data access operations
#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn store(&self, user: StoreUserRequest) -> Result<User, UserRepositoryError>;
    async fn find_by_id(&self, id: &Uuid) -> Result<Option<User>, UserRepositoryError>;
    async fn find_by_username(&self, username: &str) -> Result<Option<User>, UserRepositoryError>;
    async fn find_by_email(&self, email: &str) -> Result<Option<User>, UserRepositoryError>;
    async fn update(
        &self,
        id: &Uuid,
        updates: UpdateUserRequest,
    ) -> Result<User, UserRepositoryError>;
    async fn delete(&self, id: &Uuid) -> Result<bool, UserRepositoryError>;
    async fn exists_by_username(&self, username: &str) -> Result<bool, UserRepositoryError>;
    async fn exists_by_email(&self, email: &str) -> Result<bool, UserRepositoryError>;
}

/// PostgreSQL implementation of UserRepository
pub struct PostgresUserRepository {
    pool: PgPool,
}

impl PostgresUserRepository {
    #[allow(dead_code)]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for PostgresUserRepository {
    async fn store(&self, user: StoreUserRequest) -> Result<User, UserRepositoryError> {
        // Check for existing username
        if self.exists_by_username(&user.username).await? {
            return Err(UserRepositoryError::UsernameExists {
                username: user.username,
            });
        }

        // Check for existing email
        if self.exists_by_email(&user.email).await? {
            return Err(UserRepositoryError::EmailExists { email: user.email });
        }

        let user_id = Uuid::new_v4();
        let now = chrono::Utc::now();

        let query = r#"
            INSERT INTO users (id, username, email, password_hash, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, username, email, password_hash, created_at, updated_at
        "#;

        let user = sqlx::query_as::<_, User>(query)
            .bind(&user_id)
            .bind(&user.username)
            .bind(&user.email)
            .bind(&user.password_hash)
            .bind(now)
            .bind(now)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                tracing::error!("Failed to create user: {}", e);
                UserRepositoryError::DatabaseError(e)
            })?;

        Ok(user)
    }

    async fn find_by_id(&self, id: &Uuid) -> Result<Option<User>, UserRepositoryError> {
        let query = r#"
            SELECT id, username, email, password_hash, created_at, updated_at
            FROM users
            WHERE id = $1
        "#;

        let user = sqlx::query_as::<_, User>(query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(UserRepositoryError::DatabaseError)?;

        Ok(user)
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<User>, UserRepositoryError> {
        let query = r#"
            SELECT id, username, email, password_hash, created_at, updated_at
            FROM users
            WHERE username = $1
        "#;

        let user = sqlx::query_as::<_, User>(query)
            .bind(username)
            .fetch_optional(&self.pool)
            .await
            .map_err(UserRepositoryError::DatabaseError)?;

        Ok(user)
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<User>, UserRepositoryError> {
        let query = r#"
            SELECT id, username, email, password_hash, created_at, updated_at
            FROM users
            WHERE email = $1
        "#;

        let user = sqlx::query_as::<_, User>(query)
            .bind(email)
            .fetch_optional(&self.pool)
            .await
            .map_err(UserRepositoryError::DatabaseError)?;

        Ok(user)
    }

    async fn update(
        &self,
        id: &Uuid,
        updates: UpdateUserRequest,
    ) -> Result<User, UserRepositoryError> {
        // First check if user exists
        if !self.user_exists_by_id(id).await? {
            return Err(UserRepositoryError::NotFound);
        }

        let mut set_clauses = Vec::new();
        let mut param_count = 1;

        // Build dynamic query based on what fields are being updated
        if updates.email.is_some() {
            set_clauses.push(format!("email = ${}", param_count));
            param_count += 1;
        }

        if updates.password_hash.is_some() {
            set_clauses.push(format!("password_hash = ${}", param_count));
            param_count += 1;
        }

        if set_clauses.is_empty() {
            // No updates provided, return current user
            return self
                .find_by_id(id)
                .await?
                .ok_or(UserRepositoryError::NotFound);
        }

        set_clauses.push(format!("updated_at = ${}", param_count));

        let query = format!(
            "UPDATE users SET {} WHERE id = ${} RETURNING id, username, email, password_hash, created_at, updated_at",
            set_clauses.join(", "),
            param_count + 1
        );

        let mut query_builder = sqlx::query_as::<_, User>(&query);

        // Bind parameters in the same order as set_clauses
        if let Some(email) = &updates.email {
            query_builder = query_builder.bind(email);
        }

        if let Some(password_hash) = &updates.password_hash {
            query_builder = query_builder.bind(password_hash);
        }

        query_builder = query_builder.bind(chrono::Utc::now());
        query_builder = query_builder.bind(id);

        let user = query_builder
            .fetch_one(&self.pool)
            .await
            .map_err(UserRepositoryError::DatabaseError)?;

        Ok(user)
    }

    async fn delete(&self, id: &Uuid) -> Result<bool, UserRepositoryError> {
        let query = "DELETE FROM users WHERE id = $1";

        let result = sqlx::query(query)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(UserRepositoryError::DatabaseError)?;

        Ok(result.rows_affected() > 0)
    }

    async fn exists_by_username(&self, username: &str) -> Result<bool, UserRepositoryError> {
        let query = "SELECT EXISTS(SELECT 1 FROM users WHERE username = $1)";

        let row = sqlx::query(query)
            .bind(username)
            .fetch_one(&self.pool)
            .await
            .map_err(UserRepositoryError::DatabaseError)?;

        Ok(row.get::<bool, _>(0))
    }

    async fn exists_by_email(&self, email: &str) -> Result<bool, UserRepositoryError> {
        let query = "SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)";

        let row = sqlx::query(query)
            .bind(email)
            .fetch_one(&self.pool)
            .await
            .map_err(UserRepositoryError::DatabaseError)?;

        Ok(row.get::<bool, _>(0))
    }
}

impl PostgresUserRepository {
    #[allow(dead_code)]
    async fn user_exists_by_id(&self, id: &Uuid) -> Result<bool, UserRepositoryError> {
        let query = "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)";

        let row = sqlx::query(query)
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(UserRepositoryError::DatabaseError)?;

        Ok(row.get::<bool, _>(0))
    }
}

