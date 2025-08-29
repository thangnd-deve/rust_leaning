use chrono::Utc;
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use validator::Validate;

use crate::{
    database::repositories::{TaskRepository, TaskRepositoryError},
    models::task::{
        StoreTaskRequest, Task, TaskFilter, TaskStatistics, TaskStatus, UpdateTaskRequest,
    },
};

#[derive(Error, Debug)]
pub enum TaskServiceError {
    #[error("Validation error: {message}")]
    ValidationError { message: String },

    #[error("Task not found")]
    TaskNotFound,

    #[error("Task access denied for user")]
    AccessDenied,

    #[error("Bulk operation failed: {failed_count} out of {total_count} operations failed")]
    BulkOperationPartialFailure {
        failed_count: usize,
        total_count: usize,
    },

    #[error("Internal service error: {0}")]
    InternalError(#[from] anyhow::Error),

    #[error("Repository error: {0}")]
    RepositoryError(#[from] TaskRepositoryError),
}

pub struct TaskService {
    task_repository: Arc<dyn TaskRepository>,
}

impl TaskService {
    #[allow(dead_code)]
    pub fn new(task_repository: Arc<dyn TaskRepository>) -> Self {
        Self { task_repository }
    }

    /// Create a new task with comprehensive validation and business rules
    pub async fn create_task(
        &self,
        user_id: &Uuid,
        request: StoreTaskRequest,
    ) -> Result<Task, TaskServiceError> {
        info!(
            "Creating task for user: {} with title: '{}'",
            user_id, request.title
        );

        // Validate request
        request
            .validate()
            .map_err(|e| TaskServiceError::ValidationError {
                message: format!("Task creation validation failed: {}", e),
            })?;

        // Business rule: due date must be in future if specified
        if let Some(due_date) = request.due_date {
            if due_date <= Utc::now() {
                return Err(TaskServiceError::ValidationError {
                    message: "Due date must be in the future".to_string(),
                });
            }
        }

        let task = self
            .task_repository
            .store(request, user_id)
            .await
            .map_err(|e| {
                error!("Failed to create task in repository: {}", e);
                TaskServiceError::RepositoryError(e)
            })?;

        info!("Successfully created task with ID: {}", task.id);
        Ok(task)
    }

    /// Get tasks for a user with advanced filtering and performance optimization
    pub async fn get_tasks(
        &self,
        user_id: &Uuid,
        filter: TaskFilter,
    ) -> Result<Vec<Task>, TaskServiceError> {
        debug!(
            "Fetching tasks for user: {} with filter: {:?}",
            user_id, filter
        );

        let tasks = match filter {
            TaskFilter {
                status: Some(status),
                priority: None,
                overdue_only: false,
                search_term: None,
            } => {
                // Optimized path for status-only filtering
                self.task_repository.find_by_status(user_id, status).await?
            }
            TaskFilter {
                status: None,
                priority: None,
                overdue_only: true,
                search_term: None,
            } => {
                // Optimized path for overdue tasks
                self.task_repository.find_overdue_by_user(user_id).await?
            }
            TaskFilter {
                status: None,
                priority: None,
                overdue_only: false,
                search_term: Some(ref term),
            } => {
                // Optimized path for search
                self.task_repository.search_tasks(user_id, term).await?
            }
            _ => {
                // General case: get all tasks and filter in memory for complex conditions
                let mut tasks = self.task_repository.find_by_user_id(user_id).await?;
                self.apply_complex_filter(&mut tasks, &filter);
                tasks
            }
        };

        debug!("Retrieved {} tasks for user: {}", tasks.len(), user_id);
        Ok(tasks)
    }

    /// Get a specific task with authorization check
    pub async fn get_task(&self, user_id: &Uuid, task_id: &Uuid) -> Result<Task, TaskServiceError> {
        let task = self
            .task_repository
            .find_by_id(task_id)
            .await?
            .ok_or(TaskServiceError::TaskNotFound)?;

        // Authorization check
        if task.user_id != *user_id {
            warn!(
                "Access denied: User {} tried to access task {} owned by {}",
                user_id, task_id, task.user_id
            );
            return Err(TaskServiceError::AccessDenied);
        }

        Ok(task)
    }

    /// Update a task with authorization and validation
    pub async fn update_task(
        &self,
        user_id: &Uuid,
        task_id: &Uuid,
        updates: UpdateTaskRequest,
    ) -> Result<Task, TaskServiceError> {
        info!("Updating task {} for user {}", task_id, user_id);

        // Validate updates
        updates
            .validate()
            .map_err(|e| TaskServiceError::ValidationError {
                message: format!("Task update validation failed: {}", e),
            })?;

        // Business rule: due date must be in future if specified
        if let Some(due_date) = updates.due_date {
            if due_date <= Utc::now() {
                return Err(TaskServiceError::ValidationError {
                    message: "Due date must be in the future".to_string(),
                });
            }
        }

        let updated_task = self
            .task_repository
            .update(task_id, user_id, updates)
            .await
            .map_err(|e| match e {
                TaskRepositoryError::NotFound => TaskServiceError::TaskNotFound,
                other => TaskServiceError::RepositoryError(other),
            })?;

        info!("Successfully updated task: {}", task_id);
        Ok(updated_task)
    }

    /// Delete a task with authorization check
    pub async fn delete_task(
        &self,
        user_id: &Uuid,
        task_id: &Uuid,
    ) -> Result<bool, TaskServiceError> {
        info!("Deleting task {} for user {}", task_id, user_id);

        let deleted = self.task_repository.delete(task_id, user_id).await?;

        if deleted {
            info!("Successfully deleted task: {}", task_id);
        } else {
            warn!("Task not found or access denied: {}", task_id);
        }

        Ok(deleted)
    }

    /// Complete a task (optimized operation)
    pub async fn complete_task(
        &self,
        user_id: &Uuid,
        task_id: &Uuid,
    ) -> Result<Task, TaskServiceError> {
        info!("Completing task {} for user {}", task_id, user_id);

        // Use general update method for task completion
        let update_request = UpdateTaskRequest {
            title: None, // Will be ignored in specialized update
            description: None,
            status: Some(TaskStatus::Completed),
            priority: None, // Will be ignored
            due_date: None,
        };

        let completed_task = self
            .task_repository
            .update(task_id, user_id, update_request)
            .await
            .map_err(|e| match e {
                TaskRepositoryError::NotFound => TaskServiceError::TaskNotFound,
                other => TaskServiceError::RepositoryError(other),
            })?;

        info!("Successfully completed task: {}", task_id);
        Ok(completed_task)
    }

    /// Get overdue tasks for a user (performance optimized)
    #[allow(dead_code)]
    pub async fn get_overdue_tasks(&self, user_id: &Uuid) -> Result<Vec<Task>, TaskServiceError> {
        debug!("Fetching overdue tasks for user: {}", user_id);

        let tasks = self.task_repository.find_overdue_by_user(user_id).await?;

        debug!("Found {} overdue tasks for user: {}", tasks.len(), user_id);
        Ok(tasks)
    }

    /// Get comprehensive task statistics for a user
    #[allow(dead_code)]
    pub async fn get_task_statistics(
        &self,
        user_id: &Uuid,
    ) -> Result<TaskStatistics, TaskServiceError> {
        debug!("Calculating task statistics for user: {}", user_id);

        // Calculate stats from all tasks
        let tasks = self.task_repository.find_by_user_id(user_id).await?;
        let stats = self.calculate_statistics(&tasks);

        debug!(
            "Statistics for user {}: {} total, {} completed, {} overdue",
            user_id, stats.total_tasks, stats.completed_tasks, stats.overdue_tasks
        );

        Ok(stats)
    }

    /// Bulk operations for better performance when dealing with multiple tasks
    #[allow(dead_code)]
    pub async fn bulk_update_status(
        &self,
        user_id: &Uuid,
        task_ids: Vec<Uuid>,
        new_status: TaskStatus,
    ) -> Result<Vec<Task>, TaskServiceError> {
        info!(
            "Bulk updating status for {} tasks to {:?} for user {}",
            task_ids.len(),
            new_status,
            user_id
        );

        let mut updated_tasks = Vec::new();
        let mut failed_count = 0;
        let total_count = task_ids.len();

        for task_id in task_ids {
            // Create minimal update request focusing only on status
            let update_request = UpdateTaskRequest {
                title: None, // Will use existing value in optimized update
                description: None,
                status: Some(new_status),
                priority: None, // Will use existing value
                due_date: None,
            };

            match self
                .task_repository
                .update(&task_id, user_id, update_request)
                .await
            {
                Ok(task) => updated_tasks.push(task),
                Err(e) => {
                    warn!("Failed to update task {}: {}", task_id, e);
                    failed_count += 1;
                }
            }
        }

        if failed_count > 0 {
            if failed_count == total_count {
                return Err(TaskServiceError::BulkOperationPartialFailure {
                    failed_count,
                    total_count,
                });
            }
            warn!(
                "Bulk operation partially failed: {}/{} operations failed",
                failed_count, total_count
            );
        }

        info!(
            "Bulk update completed: {}/{} tasks updated successfully",
            updated_tasks.len(),
            total_count
        );

        Ok(updated_tasks)
    }

    /// Bulk delete multiple tasks
    #[allow(dead_code)]
    pub async fn bulk_delete_tasks(
        &self,
        user_id: &Uuid,
        task_ids: Vec<Uuid>,
    ) -> Result<usize, TaskServiceError> {
        info!(
            "Bulk deleting {} tasks for user {}",
            task_ids.len(),
            user_id
        );

        let mut deleted_count = 0;
        let mut failed_count = 0;
        let total_count = task_ids.len();

        for task_id in task_ids {
            match self.task_repository.delete(&task_id, user_id).await {
                Ok(true) => deleted_count += 1,
                Ok(false) => {
                    warn!(
                        "Task {} not found or access denied for user {}",
                        task_id, user_id
                    );
                    failed_count += 1;
                }
                Err(e) => {
                    error!("Failed to delete task {}: {}", task_id, e);
                    failed_count += 1;
                }
            }
        }

        if failed_count > 0 && deleted_count == 0 {
            return Err(TaskServiceError::BulkOperationPartialFailure {
                failed_count,
                total_count,
            });
        }

        info!(
            "Bulk delete completed: {}/{} tasks deleted successfully",
            deleted_count, total_count
        );

        Ok(deleted_count)
    }

    /// Search tasks with performance optimization
    #[allow(dead_code)]
    pub async fn search_tasks(
        &self,
        user_id: &Uuid,
        search_term: &str,
        limit: Option<usize>,
    ) -> Result<Vec<Task>, TaskServiceError> {
        if search_term.trim().is_empty() {
            return Ok(Vec::new());
        }

        debug!(
            "Searching tasks for user {} with term: '{}'",
            user_id, search_term
        );

        let mut tasks = self
            .task_repository
            .search_tasks(user_id, search_term.trim())
            .await?;

        // Apply limit if specified for performance
        if let Some(limit) = limit {
            tasks.truncate(limit);
        }

        debug!("Search returned {} tasks for user {}", tasks.len(), user_id);
        Ok(tasks)
    }

    // Private helper methods

    /// Apply complex filtering in memory (for cases where database filtering is not optimal)
    fn apply_complex_filter(&self, tasks: &mut Vec<Task>, filter: &TaskFilter) {
        tasks.retain(|task| {
            // Status filter
            if let Some(status) = filter.status {
                if task.status != status {
                    return false;
                }
            }

            // Priority filter
            if let Some(priority) = filter.priority {
                if task.priority != priority {
                    return false;
                }
            }

            // Overdue filter
            if filter.overdue_only && !task.is_overdue() {
                return false;
            }

            // Search term filter
            if let Some(ref term) = filter.search_term {
                let term_lower = term.to_lowercase();
                let title_match = task.title.to_lowercase().contains(&term_lower);
                let desc_match = task
                    .description
                    .as_ref()
                    .map(|d| d.to_lowercase().contains(&term_lower))
                    .unwrap_or(false);

                if !title_match && !desc_match {
                    return false;
                }
            }

            true
        });
    }

    /// Calculate statistics from a collection of tasks (fallback method)
    fn calculate_statistics(&self, tasks: &[Task]) -> TaskStatistics {
        let total_tasks = tasks.len() as i64;
        let mut pending_tasks = 0;
        let mut in_progress_tasks = 0;
        let mut completed_tasks = 0;
        let mut overdue_tasks = 0;

        for task in tasks {
            match task.status {
                TaskStatus::Pending => pending_tasks += 1,
                TaskStatus::InProgress => in_progress_tasks += 1,
                TaskStatus::Completed => completed_tasks += 1,
            }

            if task.is_overdue() && !task.is_completed() {
                overdue_tasks += 1;
            }
        }

        TaskStatistics {
            total_tasks,
            pending_tasks,
            in_progress_tasks,
            completed_tasks,
            overdue_tasks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::task::{StoreTaskRequest, TaskPriority};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;

    // Mock repository for testing
    struct MockTaskRepository {
        tasks: Arc<Mutex<HashMap<Uuid, Task>>>,
        next_id: Arc<Mutex<usize>>,
    }

    impl MockTaskRepository {
        fn new() -> Self {
            Self {
                tasks: Arc::new(Mutex::new(HashMap::new())),
                next_id: Arc::new(Mutex::new(0)),
            }
        }
    }

    #[async_trait]
    impl TaskRepository for MockTaskRepository {
        async fn store(
            &self,
            request: StoreTaskRequest,
            user_id: &Uuid,
        ) -> Result<Task, TaskRepositoryError> {
            let task = Task::new(request, *user_id)
                .map_err(|e| TaskRepositoryError::ValidationError(e))?;

            self.tasks.lock().unwrap().insert(task.id, task.clone());
            Ok(task)
        }

        async fn find_by_id(&self, id: &Uuid) -> Result<Option<Task>, TaskRepositoryError> {
            Ok(self.tasks.lock().unwrap().get(id).cloned())
        }

        async fn find_by_user_id(&self, user_id: &Uuid) -> Result<Vec<Task>, TaskRepositoryError> {
            let tasks: Vec<Task> = self
                .tasks
                .lock()
                .unwrap()
                .values()
                .filter(|task| task.user_id == *user_id)
                .cloned()
                .collect();
            Ok(tasks)
        }

        async fn find_overdue_by_user(
            &self,
            user_id: &Uuid,
        ) -> Result<Vec<Task>, TaskRepositoryError> {
            let tasks: Vec<Task> = self
                .tasks
                .lock()
                .unwrap()
                .values()
                .filter(|task| {
                    task.user_id == *user_id && task.is_overdue() && !task.is_completed()
                })
                .cloned()
                .collect();
            Ok(tasks)
        }

        async fn find_by_status(
            &self,
            user_id: &Uuid,
            status: TaskStatus,
        ) -> Result<Vec<Task>, TaskRepositoryError> {
            let tasks: Vec<Task> = self
                .tasks
                .lock()
                .unwrap()
                .values()
                .filter(|task| task.user_id == *user_id && task.status == status)
                .cloned()
                .collect();
            Ok(tasks)
        }

        async fn search_tasks(
            &self,
            user_id: &Uuid,
            search_term: &str,
        ) -> Result<Vec<Task>, TaskRepositoryError> {
            let search_lower = search_term.to_lowercase();
            let tasks: Vec<Task> = self
                .tasks
                .lock()
                .unwrap()
                .values()
                .filter(|task| {
                    task.user_id == *user_id
                        && (task.title.to_lowercase().contains(&search_lower)
                            || task
                                .description
                                .as_ref()
                                .map(|d| d.to_lowercase().contains(&search_lower))
                                .unwrap_or(false))
                })
                .cloned()
                .collect();
            Ok(tasks)
        }

        async fn update(
            &self,
            id: &Uuid,
            user_id: &Uuid,
            request: UpdateTaskRequest,
        ) -> Result<Task, TaskRepositoryError> {
            let mut tasks = self.tasks.lock().unwrap();
            if let Some(task) = tasks.get_mut(id) {
                if task.user_id != *user_id {
                    return Err(TaskRepositoryError::NotFound);
                }
                task.update(request);
                Ok(task.clone())
            } else {
                Err(TaskRepositoryError::NotFound)
            }
        }

        async fn delete(&self, id: &Uuid, user_id: &Uuid) -> Result<bool, TaskRepositoryError> {
            let mut tasks = self.tasks.lock().unwrap();
            if let Some(task) = tasks.get(id) {
                if task.user_id == *user_id {
                    tasks.remove(id);
                    Ok(true)
                } else {
                    Ok(false)
                }
            } else {
                Ok(false)
            }
        }

        async fn count_by_user(&self, user_id: &Uuid) -> Result<i64, TaskRepositoryError> {
            let count = self
                .tasks
                .lock()
                .unwrap()
                .values()
                .filter(|task| task.user_id == *user_id)
                .count() as i64;
            Ok(count)
        }
    }

    #[tokio::test]
    async fn test_create_task_success() {
        let repo = Arc::new(MockTaskRepository::new());
        let service = TaskService::new(repo);
        let user_id = Uuid::new_v4();

        let request = StoreTaskRequest {
            title: "Test Task".to_string(),
            description: Some("Test description".to_string()),
            status: TaskStatus::Pending,
            priority: TaskPriority::Medium,
            due_date: Some(Utc::now() + chrono::Duration::days(1)),
        };

        let result = service.create_task(&user_id, request).await;
        assert!(result.is_ok());

        let task = result.unwrap();
        assert_eq!(task.title, "Test Task");
        assert_eq!(task.user_id, user_id);
    }

    #[tokio::test]
    async fn test_get_task_authorization() {
        let repo = Arc::new(MockTaskRepository::new());
        let service = TaskService::new(repo.clone());

        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();

        // Create task for user1
        let request = StoreTaskRequest {
            title: "User1 Task".to_string(),
            description: None,
            status: TaskStatus::Pending,
            priority: TaskPriority::Medium,
            due_date: None,
        };

        let task = service.create_task(&user1, request).await.unwrap();

        // user1 should be able to access their task
        let result1 = service.get_task(&user1, &task.id).await;
        assert!(result1.is_ok());

        // user2 should not be able to access user1's task
        let result2 = service.get_task(&user2, &task.id).await;
        assert!(matches!(result2, Err(TaskServiceError::AccessDenied)));
    }

    #[tokio::test]
    async fn test_bulk_operations() {
        let repo = Arc::new(MockTaskRepository::new());
        let service = TaskService::new(repo.clone());
        let user_id = Uuid::new_v4();

        // Create multiple tasks
        let mut task_ids = Vec::new();
        for i in 0..3 {
            let request = StoreTaskRequest {
                title: format!("Task {}", i),
                description: None,
                status: TaskStatus::Pending,
                priority: TaskPriority::Medium,
                due_date: None,
            };
            let task = service.create_task(&user_id, request).await.unwrap();
            task_ids.push(task.id);
        }

        // Bulk update status
        let result = service
            .bulk_update_status(&user_id, task_ids.clone(), TaskStatus::Completed)
            .await;
        assert!(result.is_ok());
        let updated_tasks = result.unwrap();
        assert_eq!(updated_tasks.len(), 3);

        // Verify all tasks are completed
        for task in updated_tasks {
            assert_eq!(task.status, TaskStatus::Completed);
        }

        // Bulk delete
        let delete_result = service.bulk_delete_tasks(&user_id, task_ids).await;
        assert!(delete_result.is_ok());
        assert_eq!(delete_result.unwrap(), 3);
    }
}
