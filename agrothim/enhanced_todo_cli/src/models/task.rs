use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use sqlx::{Decode, Encode, Postgres, Type};
use validator::{Validate, ValidationError};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[repr(i16)]
pub enum TaskStatus {
    Pending = 0,
    InProgress = 1,
    Completed = 2,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[repr(i16)]
pub enum TaskPriority {
    Low = 0,
    Medium = 1,
    High = 2,
}

// Implement manual From/TryFrom for database conversion
impl From<TaskStatus> for i16 {
    fn from(status: TaskStatus) -> Self {
        status as i16
    }
}

impl TryFrom<i16> for TaskStatus {
    type Error = ();

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(TaskStatus::Pending),
            1 => Ok(TaskStatus::InProgress),
            2 => Ok(TaskStatus::Completed),
            _ => Err(()),
        }
    }
}

impl From<TaskPriority> for i16 {
    fn from(priority: TaskPriority) -> Self {
        priority as i16
    }
}

impl TryFrom<i16> for TaskPriority {
    type Error = ();

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(TaskPriority::Low),
            1 => Ok(TaskPriority::Medium),
            2 => Ok(TaskPriority::High),
            _ => Err(()),
        }
    }
}

// SQLx implementations for TaskStatus
impl Type<Postgres> for TaskStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <i16 as Type<Postgres>>::type_info()
    }
}

impl<'r> Decode<'r, Postgres> for TaskStatus {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
        let int_val = <i16 as Decode<Postgres>>::decode(value)?;
        TaskStatus::try_from(int_val).map_err(|_| "Invalid TaskStatus value".into())
    }
}

impl<'q> Encode<'q, Postgres> for TaskStatus {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> sqlx::encode::IsNull {
        <i16 as Encode<Postgres>>::encode_by_ref(&(*self as i16), buf)
    }
}

// SQLx implementations for TaskPriority
impl Type<Postgres> for TaskPriority {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <i16 as Type<Postgres>>::type_info()
    }
}

impl<'r> Decode<'r, Postgres> for TaskPriority {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
        let int_val = <i16 as Decode<Postgres>>::decode(value)?;
        TaskPriority::try_from(int_val).map_err(|_| "Invalid TaskPriority value".into())
    }
}

impl<'q> Encode<'q, Postgres> for TaskPriority {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> sqlx::encode::IsNull {
        <i16 as Encode<Postgres>>::encode_by_ref(&(*self as i16), buf)
    }
}

impl Default for TaskStatus {
    fn default() -> Self {
        TaskStatus::Pending
    }
}

impl Default for TaskPriority {
    fn default() -> Self {
        TaskPriority::Medium
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::InProgress => write!(f, "in progress"),
            TaskStatus::Completed => write!(f, "completed"),
        }
    }
}

impl std::fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskPriority::Low => write!(f, "low"),
            TaskPriority::Medium => write!(f, "medium"),
            TaskPriority::High => write!(f, "high"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub due_date: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Type alias for response - in this case it's the same as Task
pub type TaskResponse = Task;

// request dto
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct StoreTaskRequest {
    #[validate(length(min = 1, max = 255, message = "Title must be 1-255 characters"))]
    #[validate(custom = "validate_title")]
    pub title: String,

    #[validate(length(max = 1000, message = "Description must be less than 1000 characters"))]
    pub description: Option<String>,

    pub status: TaskStatus,

    pub priority: TaskPriority,

    pub due_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Validate, Default)]
pub struct UpdateTaskRequest {
    #[validate(length(min = 1, max = 255, message = "Title must be 1-255 characters"))]
    pub title: Option<String>,

    #[validate(length(max = 1000, message = "Description must be less than 1000 characters"))]
    pub description: Option<String>,

    pub status: Option<TaskStatus>,

    pub priority: Option<TaskPriority>,

    pub due_date: Option<DateTime<Utc>>,
}

fn validate_title(title: &str) -> Result<(), ValidationError> {
    if title.trim().is_empty() {
        return Err(ValidationError::new("Title is required"));
    }
    Ok(())
}

// custom error
#[derive(Debug, thiserror::Error)]
pub enum TaskError {
    #[error("Validation error: {0}")]
    ValidationError(#[from] validator::ValidationErrors),
}

impl Task {
    pub fn new(request: StoreTaskRequest, user_id: Uuid) -> Result<Self, TaskError> {
        request
            .validate()
            .map_err(|e| TaskError::ValidationError(e))?;

        let completed_at = if matches!(request.status, TaskStatus::Completed) {
            Some(Utc::now())
        } else {
            None
        };

        Ok(Self {
            id: Uuid::new_v4(),
            title: request.title.trim().to_string(),
            description: request.description.map(|d| d.trim().to_string()),
            status: request.status,
            priority: request.priority,
            user_id,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            due_date: request.due_date,
            completed_at,
        })
    }
    /**
     * mark as uncomplete
     * if the task is completed, mark it as pending
     * and set the completed_at to None
     * and update the updated_at
     */
    #[allow(dead_code)]
    pub fn uncomplete(&mut self) {
        if matches!(self.status, TaskStatus::Completed) {
            self.status = TaskStatus::Pending;
            self.completed_at = None;
            self.updated_at = Utc::now();
        }
    }

    #[allow(dead_code)]
    pub fn complete(&mut self) {
        if matches!(self.status, TaskStatus::Pending) {
            self.status = TaskStatus::Completed;
            self.completed_at = Some(Utc::now());
            self.updated_at = Utc::now();
        }
    }

    #[allow(dead_code)]
    pub fn set_in_process(&mut self) {
        if matches!(self.status, TaskStatus::Pending) {
            self.status = TaskStatus::InProgress;
            self.updated_at = Utc::now();
        }
    }

    #[allow(dead_code)]
    pub fn is_overdue(&self) -> bool {
        if let Some(due_date) = self.due_date {
            due_date < Utc::now()
        } else {
            false
        }
    }

    #[allow(dead_code)]
    pub fn is_completed(&self) -> bool {
        matches!(self.status, TaskStatus::Completed)
    }

    #[allow(dead_code)]
    pub fn is_in_process(&self) -> bool {
        matches!(self.status, TaskStatus::InProgress)
    }

    #[allow(dead_code)]
    pub fn day_until_due(&self) -> Option<i64> {
        if let Some(due_date) = self.due_date {
            let days = due_date.signed_duration_since(Utc::now()).num_days();
            Some(days)
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn update(&mut self, request: UpdateTaskRequest) {
        let mut updated = false;

        // Only update title if provided and not empty
        if let Some(title) = request.title {
            if !title.trim().is_empty() && self.title != title {
                self.title = title;
                updated = true;
            }
        }

        if self.description != request.description {
            self.description = request.description;
            updated = true;
        }

        if let Some(status) = request.status {
            if self.status != status {
                let old_status = self.status;
                self.status = status;
                
                // Handle completed_at based on status change
                match (old_status, self.status) {
                    (_, TaskStatus::Completed) if old_status != TaskStatus::Completed => {
                        self.completed_at = Some(Utc::now());
                    },
                    (TaskStatus::Completed, _) if self.status != TaskStatus::Completed => {
                        self.completed_at = None;
                    },
                    _ => {}
                }
                updated = true;
            }
        }

        if let Some(priority) = request.priority {
            if self.priority != priority {
                self.priority = priority;
                updated = true;
            }
        }

        if self.due_date != request.due_date {
            self.due_date = request.due_date;
            updated = true;
        }

        if updated {
            self.updated_at = Utc::now();
        }
    }
}

// Task statistics DTO
#[derive(Debug, Serialize, Deserialize)]
pub struct TaskStatistics {
    pub total_tasks: i64,
    pub pending_tasks: i64,
    pub in_progress_tasks: i64,
    pub completed_tasks: i64,
    pub overdue_tasks: i64,
}

// Task Filter for queries
#[derive(Debug, Default)]
pub struct TaskFilter {
    pub status: Option<TaskStatus>,
    pub priority: Option<TaskPriority>,
    pub overdue_only: bool,
    pub search_term: Option<String>,
}

impl TaskFilter {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    pub fn with_status(mut self, status: TaskStatus) -> Self {
        self.status = Some(status);
        self
    }

    #[allow(dead_code)]
    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = Some(priority);
        self
    }

    #[allow(dead_code)]
    pub fn overdue_only(mut self) -> Self {
        self.overdue_only = true;
        self
    }

    #[allow(dead_code)]
    pub fn with_search(mut self, term: String) -> Self {
        self.search_term = Some(term);
        self
    }
}
