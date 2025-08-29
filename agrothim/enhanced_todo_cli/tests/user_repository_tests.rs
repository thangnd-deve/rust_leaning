use sqlx::{Executor, PgPool};
use tokio;
use uuid::Uuid;
use url::Url;

use enhanced_todo_cli::database::repositories::user_repository::{
    PostgresUserRepository, UserRepository,
};
use enhanced_todo_cli::models::user::{StoreUserRequest, UpdateUserRequest};

async fn setup_test_db() -> (PgPool, String) {
    let base_url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
        // Match docker-compose default database
        "postgres://todo_user:todo_pass@localhost:5432/todo_cli".to_string()
    });

    // Create a unique schema per test
    let schema = format!("test_{}", Uuid::new_v4().simple());

    // Use a temporary pool to create the schema
    let admin_pool = PgPool::connect(&base_url)
        .await
        .expect("Failed to connect to test database");
    admin_pool
        .execute(&*format!("CREATE SCHEMA IF NOT EXISTS {}", schema))
        .await
        .unwrap();

    // Build a URL that sets search_path to the new schema
    let mut url = Url::parse(&base_url).expect("Invalid TEST_DATABASE_URL");
    let mut qp: Vec<(String, String)> = url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    qp.push((
        "options".to_string(),
        format!("-csearch_path={}", schema),
    ));
    url.query_pairs_mut().clear().extend_pairs(qp.iter().map(|(k, v)| (&**k, &**v)));

    let pool = PgPool::connect(url.as_str())
        .await
        .expect("Failed to connect to test database with search_path");

    // Clean up and recreate tables for each test within the schema
    pool.execute("DROP TABLE IF EXISTS tasks CASCADE").await.unwrap();
    pool.execute("DROP TABLE IF EXISTS users CASCADE").await.unwrap();

    // Enable UUID extension first - ignore error if already exists
    let _ = pool.execute("CREATE EXTENSION IF NOT EXISTS \"pgcrypto\"")
        .await;

    // Create users table
    pool
        .execute(
            r#"
        CREATE TABLE users (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            username VARCHAR(50) UNIQUE NOT NULL,
            email VARCHAR(255) UNIQUE NOT NULL,
            password_hash TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
    "#,
        )
        .await
        .unwrap();

    (pool, schema)
}

async fn drop_test_schema(schema: &str) {
    let base_url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
        "postgres://todo_user:todo_pass@localhost:5432/todo_cli".to_string()
    });
    if let Ok(admin_pool) = PgPool::connect(&base_url).await {
        let _ = admin_pool
            .execute(&*format!("DROP SCHEMA IF EXISTS {} CASCADE", schema))
            .await;
    }
}

#[tokio::test]
async fn test_create_user_success() {
    let (pool, schema) = setup_test_db().await;
    let repo = PostgresUserRepository::new(pool);

    let request = StoreUserRequest::new(
        "testuser".to_string(),
        "test@example.com".to_string(),
        "password123".to_string(),
    )
    .unwrap();

    let user = repo.store(request).await.unwrap();

    assert_eq!(user.username, "testuser");
    assert_eq!(user.email, "test@example.com");
    assert!(!user.id.is_nil());
    assert!(user.verify_password("password123"));

    drop_test_schema(&schema).await;
}

#[tokio::test]
async fn test_create_user_duplicate_username() {
    let (pool, schema) = setup_test_db().await;
    let repo = PostgresUserRepository::new(pool);

    let request1 = StoreUserRequest::new(
        "testuser".to_string(),
        "test1@example.com".to_string(),
        "password123".to_string(),
    )
    .unwrap();
    repo.store(request1).await.unwrap();

    let request2 = StoreUserRequest::new(
        "testuser".to_string(),
        "test2@example.com".to_string(),
        "password123".to_string(),
    )
    .unwrap();

    let result = repo.store(request2).await;
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        enhanced_todo_cli::database::repositories::user_repository::UserRepositoryError::UsernameExists { .. }
    ));

    drop_test_schema(&schema).await;
}

#[tokio::test]
async fn test_create_user_duplicate_email() {
    let (pool, schema) = setup_test_db().await;
    let repo = PostgresUserRepository::new(pool);

    let request1 = StoreUserRequest::new(
        "testuser1".to_string(),
        "test@example.com".to_string(),
        "password123".to_string(),
    )
    .unwrap();
    repo.store(request1).await.unwrap();

    let request2 = StoreUserRequest::new(
        "testuser2".to_string(),
        "test@example.com".to_string(),
        "password123".to_string(),
    )
    .unwrap();

    let result = repo.store(request2).await;
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        enhanced_todo_cli::database::repositories::user_repository::UserRepositoryError::EmailExists { .. }
    ));

    drop_test_schema(&schema).await;
}

#[tokio::test]
async fn test_find_by_id_success() {
    let (pool, schema) = setup_test_db().await;
    let repo = PostgresUserRepository::new(pool);

    let request = StoreUserRequest::new(
        "testuser".to_string(),
        "test@example.com".to_string(),
        "password123".to_string(),
    )
    .unwrap();

    let created_user = repo.store(request).await.unwrap();
    let found_user = repo.find_by_id(&created_user.id).await.unwrap();

    assert!(found_user.is_some());
    let found_user = found_user.unwrap();
    assert_eq!(found_user.id, created_user.id);
    assert_eq!(found_user.username, created_user.username);

    drop_test_schema(&schema).await;
}

#[tokio::test]
async fn test_find_by_id_not_found() {
    let (pool, schema) = setup_test_db().await;
    let repo = PostgresUserRepository::new(pool);

    let non_existent_id = Uuid::new_v4();
    let result = repo.find_by_id(&non_existent_id).await.unwrap();
    assert!(result.is_none());

    drop_test_schema(&schema).await;
}

#[tokio::test]
async fn test_find_by_username_success() {
    let (pool, schema) = setup_test_db().await;
    let repo = PostgresUserRepository::new(pool);

    let request = StoreUserRequest::new(
        "testuser".to_string(),
        "test@example.com".to_string(),
        "password123".to_string(),
    )
    .unwrap();

    let created_user = repo.store(request).await.unwrap();
    let found_user = repo.find_by_username("testuser").await.unwrap();

    assert!(found_user.is_some());
    let found_user = found_user.unwrap();
    assert_eq!(found_user.id, created_user.id);
    assert_eq!(found_user.username, "testuser");

    drop_test_schema(&schema).await;
}

#[tokio::test]
async fn test_update_user_email() {
    let (pool, schema) = setup_test_db().await;
    let repo = PostgresUserRepository::new(pool);

    let request = StoreUserRequest::new(
        "testuser".to_string(),
        "test@example.com".to_string(),
        "password123".to_string(),
    )
    .unwrap();

    let created_user = repo.store(request).await.unwrap();

    let update_request = UpdateUserRequest::new().email("newemail@example.com".to_string());

    let updated_user = repo.update(&created_user.id, update_request).await.unwrap();

    assert_eq!(updated_user.email, "newemail@example.com");
    assert_eq!(updated_user.username, created_user.username); // Unchanged

    drop_test_schema(&schema).await;
}

#[tokio::test]
async fn test_update_user_not_found() {
    let (pool, schema) = setup_test_db().await;
    let repo = PostgresUserRepository::new(pool);

    let update_request = UpdateUserRequest::new().email("test@example.com".to_string());

    let non_existent_id = Uuid::new_v4();
    let result = repo.update(&non_existent_id, update_request).await;
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        enhanced_todo_cli::database::repositories::user_repository::UserRepositoryError::NotFound
    ));

    drop_test_schema(&schema).await;
}

#[tokio::test]
async fn test_delete_user_success() {
    let (pool, schema) = setup_test_db().await;
    let repo = PostgresUserRepository::new(pool);

    let request = StoreUserRequest::new(
        "testuser".to_string(),
        "test@example.com".to_string(),
        "password123".to_string(),
    )
    .unwrap();

    let created_user = repo.store(request).await.unwrap();
    let deleted = repo.delete(&created_user.id).await.unwrap();

    assert!(deleted);

    let found_user = repo.find_by_id(&created_user.id).await.unwrap();
    assert!(found_user.is_none());

    drop_test_schema(&schema).await;
}

#[tokio::test]
async fn test_delete_user_not_found() {
    let (pool, schema) = setup_test_db().await;
    let repo = PostgresUserRepository::new(pool);

    let non_existent_id = Uuid::new_v4();
    let deleted = repo.delete(&non_existent_id).await.unwrap();
    assert!(!deleted);

    drop_test_schema(&schema).await;
}


