use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;
use sqlx::{PgPool, Row};
use thiserror::Error;
use validator::Validate;

use crate::models::task::{StoreTaskRequest, Task, TaskStatus, TaskStatistics, UpdateTaskRequest};

#[derive(Error, Debug)]
pub enum TaskRepositoryError {
    #[error("Not found")]
    NotFound,
    #[error("Validation error: {0}")]
    ValidationError(#[from] crate::models::task::TaskError),
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
}

/// Task repository trait for data access operations
#[async_trait]
pub trait TaskRepository: Send + Sync {
    async fn store(&self, task: StoreTaskRequest, user_id: &Uuid) -> Result<Task, TaskRepositoryError>;
    async fn find_by_id(&self, id: &Uuid) -> Result<Option<Task>, TaskRepositoryError>;
    async fn find_by_user_id(&self, user_id: &Uuid) -> Result<Vec<Task>, TaskRepositoryError>;
    async fn find_overdue_by_user(&self, user_id: &Uuid) -> Result<Vec<Task>, TaskRepositoryError>;
    async fn find_by_status(&self, user_id: &Uuid, status: TaskStatus) -> Result<Vec<Task>, TaskRepositoryError>;
    async fn search_tasks(&self, user_id: &Uuid, search_term: &str) -> Result<Vec<Task>, TaskRepositoryError>;
    async fn update(
        &self,
        id: &Uuid,
        user_id: &Uuid,
        request: UpdateTaskRequest,
    ) -> Result<Task, TaskRepositoryError>;
    async fn delete(&self, id: &Uuid, user_id: &Uuid) -> Result<bool, TaskRepositoryError>;
    #[allow(dead_code)]
    async fn count_by_user(&self, user_id: &Uuid) -> Result<i64, TaskRepositoryError>;
}

/// PostgreSQL implementation of TaskRepository
pub struct PostgresTaskRepository {
    pool: PgPool,
}

impl PostgresTaskRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TaskRepository for PostgresTaskRepository {
    async fn store(&self, request: StoreTaskRequest, user_id: &Uuid) -> Result<Task, TaskRepositoryError> {
        let task = Task::new(request, *user_id)?;

        let query = r#"
            INSERT INTO tasks (id, title, description, status, priority, due_date, completed_at, user_id, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id, title, description, status, priority, due_date, completed_at, user_id, created_at, updated_at
        "#;
        
        let stored_task = sqlx::query_as::<_, Task>(query)
            .bind(&task.id)
            .bind(&task.title)
            .bind(&task.description)
            .bind(&task.status)
            .bind(&task.priority)
            .bind(&task.due_date)
            .bind(&task.completed_at)
            .bind(&task.user_id)
            .bind(&task.created_at)
            .bind(&task.updated_at)
            .fetch_one(&self.pool)
            .await
            .map_err(TaskRepositoryError::DatabaseError)?;

        Ok(stored_task)
    }

    async fn find_by_id(&self, id: &Uuid) -> Result<Option<Task>, TaskRepositoryError> {
        let query = r#"
            SELECT * FROM tasks WHERE id = $1
        "#;
        let task = sqlx::query_as::<_, Task>(query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(TaskRepositoryError::DatabaseError)?;

        Ok(task)
    }

    async fn find_by_user_id(&self, user_id: &Uuid) -> Result<Vec<Task>, TaskRepositoryError> {
        let query = r#"
            SELECT * FROM tasks 
            WHERE user_id = $1 
            ORDER BY updated_at DESC
        "#;
        let tasks = sqlx::query_as::<_, Task>(query)
            .bind(user_id)
            .fetch_all(&self.pool)
            .await
            .map_err(TaskRepositoryError::DatabaseError)?;

        Ok(tasks)
    }

    async fn find_overdue_by_user(&self, user_id: &Uuid) -> Result<Vec<Task>, TaskRepositoryError> {
        let query = r#"
            SELECT * FROM tasks 
            WHERE user_id = $1 
            AND due_date < NOW() 
            AND status != 2
            ORDER BY due_date ASC
        "#;
        let tasks = sqlx::query_as::<_, Task>(query)
            .bind(user_id)
            .fetch_all(&self.pool)
            .await
            .map_err(TaskRepositoryError::DatabaseError)?;

        Ok(tasks)
    }

    async fn find_by_status(&self, user_id: &Uuid, status: TaskStatus) -> Result<Vec<Task>, TaskRepositoryError> {
        let query = r#"
            SELECT * FROM tasks 
            WHERE user_id = $1 AND status = $2 
            ORDER BY updated_at DESC
        "#;
        let tasks = sqlx::query_as::<_, Task>(query)
            .bind(user_id)
            .bind(status)
            .fetch_all(&self.pool)
            .await
            .map_err(TaskRepositoryError::DatabaseError)?;

        Ok(tasks)
    }

    async fn search_tasks(&self, user_id: &Uuid, search_term: &str) -> Result<Vec<Task>, TaskRepositoryError> {
        let query = r#"
            SELECT * FROM tasks 
            WHERE user_id = $1 
            AND (
                title ILIKE '%' || $2 || '%' 
                OR description ILIKE '%' || $2 || '%'
            )
            ORDER BY updated_at DESC
        "#;
        let tasks = sqlx::query_as::<_, Task>(query)
            .bind(user_id)
            .bind(search_term)
            .fetch_all(&self.pool)
            .await
            .map_err(TaskRepositoryError::DatabaseError)?;

        Ok(tasks)
    }

    async fn update(
        &self,
        id: &Uuid,
        user_id: &Uuid,
        request: UpdateTaskRequest,
    ) -> Result<Task, TaskRepositoryError> {
        // Validate request before updating
        request
            .validate()
            .map_err(|e| TaskRepositoryError::ValidationError(e.into()))?;

        // Business rule: auto-set completed_at when status = Completed, otherwise NULL
        let completed_at = if matches!(request.status, Some(TaskStatus::Completed)) {
            Some(Utc::now())
        } else {
            None
        };

        let query = r#"
            UPDATE tasks
            SET title = $3,
                description = $4,
                status = $5,
                priority = $6,
                due_date = $7,
                completed_at = $8,
                updated_at = NOW()
            WHERE id = $1 AND user_id = $2
            RETURNING id, title, description, status, priority, due_date, completed_at, user_id, created_at, updated_at
        "#;

        let updated = sqlx::query_as::<_, Task>(query)
            .bind(id)
            .bind(user_id)
            .bind(&request.title)
            .bind(&request.description)
            .bind(&request.status)
            .bind(&request.priority)
            .bind(&request.due_date)
            .bind(&completed_at)
            .fetch_optional(&self.pool)
            .await
            .map_err(TaskRepositoryError::DatabaseError)?;

        updated.ok_or(TaskRepositoryError::NotFound)
    }

    async fn delete(&self, id: &Uuid, user_id: &Uuid) -> Result<bool, TaskRepositoryError> {
        let query = r#"
            DELETE FROM tasks WHERE id = $1 AND user_id = $2
        "#;
        let result = sqlx::query(query)
            .bind(id)
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(TaskRepositoryError::DatabaseError)?;

        Ok(result.rows_affected() > 0)
    }

    #[allow(dead_code)]
    async fn count_by_user(&self, user_id: &Uuid) -> Result<i64, TaskRepositoryError> {
        let query = r#"
            SELECT COUNT(*) FROM tasks WHERE user_id = $1
        "#;
        let count: i64 = sqlx::query_scalar(query)
            .bind(user_id)
            .fetch_one(&self.pool)
            .await
            .map_err(TaskRepositoryError::DatabaseError)?;

        Ok(count)
    }
}

impl PostgresTaskRepository {
    // allow dead_code
    #[allow(dead_code)]
    async fn task_exists_by_id(&self, id: &Uuid) -> Result<bool, TaskRepositoryError> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE id = $1")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(count > 0)
    }

    // Business Logic: Mark task as completed with completed_at timestamp
    #[allow(dead_code)]
    pub async fn mark_complete(&self, id: &Uuid, user_id: &Uuid) -> Result<Task, TaskRepositoryError> {
        let query = r#"
            UPDATE tasks 
            SET status = $3, completed_at = NOW(), updated_at = NOW()
            WHERE id = $1 AND user_id = $2 AND status != 2
            RETURNING id, title, description, status, priority, due_date, completed_at, user_id, created_at, updated_at
        "#;

        let updated = sqlx::query_as::<_, Task>(query)
            .bind(id)
            .bind(user_id)
            .bind(TaskStatus::Completed)
            .fetch_optional(&self.pool)
            .await
            .map_err(TaskRepositoryError::DatabaseError)?;

        updated.ok_or(TaskRepositoryError::NotFound)
    }

    // Business Logic: Mark task as incomplete (remove completed_at)
    #[allow(dead_code)]
    pub async fn mark_incomplete(&self, id: &Uuid, user_id: &Uuid) -> Result<Task, TaskRepositoryError> {
        let query = r#"
            UPDATE tasks 
            SET status = $3, completed_at = NULL, updated_at = NOW()
            WHERE id = $1 AND user_id = $2 AND status = 2
            RETURNING id, title, description, status, priority, due_date, completed_at, user_id, created_at, updated_at
        "#;

        let updated = sqlx::query_as::<_, Task>(query)
            .bind(id)
            .bind(user_id)
            .bind(TaskStatus::Pending)
            .fetch_optional(&self.pool)
            .await
            .map_err(TaskRepositoryError::DatabaseError)?;

        updated.ok_or(TaskRepositoryError::NotFound)
    }

    // Business Logic: Get task statistics for a user
    #[allow(dead_code)]
    pub async fn get_user_statistics(&self, user_id: &Uuid) -> Result<TaskStatistics, TaskRepositoryError> {
        let query = r#"
            SELECT 
                COUNT(*) as total_tasks,
                COUNT(CASE WHEN status = 0 THEN 1 END) as pending_tasks,
                COUNT(CASE WHEN status = 1 THEN 1 END) as in_progress_tasks,
                COUNT(CASE WHEN status = 2 THEN 1 END) as completed_tasks,
                COUNT(CASE WHEN due_date < NOW() AND status != 2 THEN 1 END) as overdue_tasks
            FROM tasks 
            WHERE user_id = $1
        "#;

        let row = sqlx::query(query)
            .bind(user_id)
            .fetch_one(&self.pool)
            .await
            .map_err(TaskRepositoryError::DatabaseError)?;

        Ok(TaskStatistics {
            total_tasks: row.get::<i64, _>("total_tasks"),
            pending_tasks: row.get::<i64, _>("pending_tasks"),
            in_progress_tasks: row.get::<i64, _>("in_progress_tasks"),
            completed_tasks: row.get::<i64, _>("completed_tasks"),
            overdue_tasks: row.get::<i64, _>("overdue_tasks"),
        })
    }
}
