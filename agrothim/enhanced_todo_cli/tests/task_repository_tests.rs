use chrono::{Duration, Utc};
use sqlx::{Executor, PgPool};
use uuid::Uuid;
use url::Url;

use enhanced_todo_cli::database::repositories::task_repository::{
    PostgresTaskRepository, TaskRepository,
};
use enhanced_todo_cli::models::task::{StoreTaskRequest, TaskPriority, TaskStatus, UpdateTaskRequest};

async fn setup_test_db() -> (PgPool, String, Uuid) {
    let base_url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
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

    // Create tasks table
    pool
        .execute(
            r#"
        CREATE TABLE tasks (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            title VARCHAR(255) NOT NULL,
            description TEXT,
            status SMALLINT NOT NULL DEFAULT 0 CONSTRAINT status_check CHECK (status IN (0, 1, 2)),
            priority SMALLINT NOT NULL DEFAULT 1 CONSTRAINT priority_check CHECK (priority IN (0, 1, 2)),
            due_date TIMESTAMPTZ,
            completed_at TIMESTAMPTZ,
            user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
    "#,
        )
        .await
        .unwrap();

    // Insert a test user and return their UUID
    let user_id = Uuid::new_v4();
    pool.execute(&*format!(
        r#"
        INSERT INTO users (id, username, email, password_hash)
        VALUES ('{}', 'tester', 'tester@example.com', '$2b$12$abcdefghijklmnopqrstuv')
    "#,
        user_id
    ))
    .await
    .unwrap();

    (pool, schema, user_id)
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
async fn test_store_task_success() {
    let (pool, schema, user_id) = setup_test_db().await;
    let repo = PostgresTaskRepository::new(pool);

    let request = StoreTaskRequest {
        title: "Test task".to_string(),
        description: Some("desc".to_string()),
        status: TaskStatus::Pending,
        priority: TaskPriority::Medium,
        due_date: None,
    };

    let task = repo.store(request, &user_id).await.expect("store failed");

    assert_eq!(task.title, "Test task");
    assert_eq!(task.description.as_deref(), Some("desc"));
    assert_eq!(task.status, TaskStatus::Pending);
    assert_eq!(task.priority, TaskPriority::Medium);
    assert_eq!(task.user_id, user_id);
    
    drop_test_schema(&schema).await;
}

#[tokio::test]
async fn test_find_by_id_success() {
    let (pool, schema, user_id) = setup_test_db().await;
    let repo = PostgresTaskRepository::new(pool);

    let created = repo
        .store(StoreTaskRequest {
            title: "Test task".to_string(),
            description: None,
            status: TaskStatus::Pending,
            priority: TaskPriority::Low,
            due_date: None,
        }, &user_id)
        .await
        .unwrap();

    let found = repo.find_by_id(&created.id).await.unwrap();
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.id, created.id);
    assert_eq!(found.title, "Test task");
    
    drop_test_schema(&schema).await;
}

#[tokio::test]
async fn test_find_by_user_id() {
    let (pool, schema, user_id) = setup_test_db().await;
    let repo = PostgresTaskRepository::new(pool);

    // Create multiple tasks
    repo.store(StoreTaskRequest {
        title: "Task 1".to_string(),
        description: None,
        status: TaskStatus::Pending,
        priority: TaskPriority::Low,
        due_date: None,
    }, &user_id).await.unwrap();

    repo.store(StoreTaskRequest {
        title: "Task 2".to_string(),
        description: None,
        status: TaskStatus::Completed,
        priority: TaskPriority::High,
        due_date: None,
    }, &user_id).await.unwrap();

    let tasks = repo.find_by_user_id(&user_id).await.unwrap();
    assert_eq!(tasks.len(), 2);
    
    drop_test_schema(&schema).await;
}

#[tokio::test]
async fn test_find_overdue_by_user() {
    let (pool, schema, user_id) = setup_test_db().await;
    let repo = PostgresTaskRepository::new(pool);

    // Create overdue task
    repo.store(StoreTaskRequest {
        title: "Overdue task".to_string(),
        description: None,
        status: TaskStatus::Pending,
        priority: TaskPriority::Low,
        due_date: Some(Utc::now() - Duration::days(1)),
    }, &user_id).await.unwrap();

    // Create future task
    repo.store(StoreTaskRequest {
        title: "Future task".to_string(),
        description: None,
        status: TaskStatus::Pending,
        priority: TaskPriority::Low,
        due_date: Some(Utc::now() + Duration::days(3)),
    }, &user_id).await.unwrap();

    let overdue = repo.find_overdue_by_user(&user_id).await.unwrap();
    assert_eq!(overdue.len(), 1);
    assert_eq!(overdue[0].title, "Overdue task");
    
    drop_test_schema(&schema).await;
}

#[tokio::test]
async fn test_update_task() {
    let (pool, schema, user_id) = setup_test_db().await;
    let repo = PostgresTaskRepository::new(pool);

    let created = repo.store(StoreTaskRequest {
        title: "Original".to_string(),
        description: None,
        status: TaskStatus::Pending,
        priority: TaskPriority::Low,
        due_date: None,
    }, &user_id).await.unwrap();

    let update_request = UpdateTaskRequest {
        title: Some("Updated".to_string()),
        description: Some("Updated description".to_string()),
        status: Some(TaskStatus::Completed),
        priority: Some(TaskPriority::High),
        due_date: None,
    };

    let updated = repo.update(&created.id, &user_id, update_request).await.unwrap();
    assert_eq!(updated.title, "Updated");
    assert_eq!(updated.status, TaskStatus::Completed);
    assert!(updated.completed_at.is_some());
    
    drop_test_schema(&schema).await;
}

#[tokio::test]
async fn test_delete_task() {
    let (pool, schema, user_id) = setup_test_db().await;
    let repo = PostgresTaskRepository::new(pool);

    let created = repo.store(StoreTaskRequest {
        title: "To delete".to_string(),
        description: None,
        status: TaskStatus::Pending,
        priority: TaskPriority::Low,
        due_date: None,
    }, &user_id).await.unwrap();

    let deleted = repo.delete(&created.id, &user_id).await.unwrap();
    assert!(deleted);

    let found = repo.find_by_id(&created.id).await.unwrap();
    assert!(found.is_none());
    
    drop_test_schema(&schema).await;
}

#[tokio::test]
async fn test_search_tasks() {
    let (pool, schema, user_id) = setup_test_db().await;
    let repo = PostgresTaskRepository::new(pool);

    repo.store(StoreTaskRequest {
        title: "Search this".to_string(),
        description: Some("Important task".to_string()),
        status: TaskStatus::Pending,
        priority: TaskPriority::Low,
        due_date: None,
    }, &user_id).await.unwrap();

    repo.store(StoreTaskRequest {
        title: "Another task".to_string(),
        description: Some("Not important".to_string()),
        status: TaskStatus::Pending,
        priority: TaskPriority::Low,
        due_date: None,
    }, &user_id).await.unwrap();

    let results = repo.search_tasks(&user_id, "Search").await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Search this");
    
    drop_test_schema(&schema).await;
}

#[tokio::test]
async fn test_find_by_status() {
    let (pool, schema, user_id) = setup_test_db().await;
    let repo = PostgresTaskRepository::new(pool);

    repo.store(StoreTaskRequest {
        title: "Pending task".to_string(),
        description: None,
        status: TaskStatus::Pending,
        priority: TaskPriority::Low,
        due_date: None,
    }, &user_id).await.unwrap();

    repo.store(StoreTaskRequest {
        title: "Completed task".to_string(),
        description: None,
        status: TaskStatus::Completed,
        priority: TaskPriority::Low,
        due_date: None,
    }, &user_id).await.unwrap();

    let pending = repo.find_by_status(&user_id, TaskStatus::Pending).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].title, "Pending task");

    let completed = repo.find_by_status(&user_id, TaskStatus::Completed).await.unwrap();
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].title, "Completed task");
    
    drop_test_schema(&schema).await;
}

#[tokio::test]
async fn test_count_by_user() {
    let (pool, schema, user_id) = setup_test_db().await;
    let repo = PostgresTaskRepository::new(pool);

    // Initially should be 0
    let count = repo.count_by_user(&user_id).await.unwrap();
    assert_eq!(count, 0);

    // Add some tasks
    repo.store(StoreTaskRequest {
        title: "Task 1".to_string(),
        description: None,
        status: TaskStatus::Pending,
        priority: TaskPriority::Low,
        due_date: None,
    }, &user_id).await.unwrap();

    repo.store(StoreTaskRequest {
        title: "Task 2".to_string(),
        description: None,
        status: TaskStatus::Completed,
        priority: TaskPriority::High,
        due_date: None,
    }, &user_id).await.unwrap();

    let count = repo.count_by_user(&user_id).await.unwrap();
    assert_eq!(count, 2);
    
    drop_test_schema(&schema).await;
}