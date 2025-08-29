use std::sync::Arc;
use anyhow::{Context, Result};
use console::{style, Emoji};
use dialoguer::{Input, Password, Confirm, theme::ColorfulTheme};

use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{
    cli::args::*,
    services::{AuthService, UserService, TaskService, UserServiceError, AuthServiceError},
    models::{
        user::{StoreUserRequest, UserResponse},
        task::{StoreTaskRequest, UpdateTaskRequest, TaskFilter, TaskPriority as ModelTaskPriority, TaskStatus as ModelTaskStatus},
    },
    utils::formatting::{format_task_table, format_date, format_task_detail},
    database::{Database, repositories::{PostgresUserRepository, PostgresTaskRepository}},
};

static CHECKMARK: Emoji<'_, '_> = Emoji("✅ ", "");
static CROSS: Emoji<'_, '_> = Emoji("❌ ", "");
static WARNING: Emoji<'_, '_> = Emoji("⚠️ ", "");
static INFO: Emoji<'_, '_> = Emoji("ℹ️ ", "");
static ROCKET: Emoji<'_, '_> = Emoji("🚀 ", "");

pub struct CliApp {
    auth_service: Arc<AuthService>,
    user_service: Arc<UserService>,
    task_service: Arc<TaskService>,
}

impl CliApp {
    pub async fn new() -> Result<Self> {
        dotenv::dotenv().ok();
        
        let database_url = std::env::var("DATABASE_URL")
            .context("DATABASE_URL must be set")?;
        
        let jwt_secret = std::env::var("JWT_SECRET")
            .unwrap_or_else(|_| "default-secret-change-in-production".to_string());

        // Initialize database and repositories
        let db = Database::from_url(&database_url).await
            .context("Failed to initialize database")?;
        
        let pool = db.pool();
        let user_repo = Arc::new(PostgresUserRepository::new(pool.clone()));
        let task_repo = Arc::new(PostgresTaskRepository::new(pool.clone()));

        // Initialize services
        let user_service = Arc::new(UserService::new(user_repo));
        let task_service = Arc::new(TaskService::new(task_repo));
        let auth_service = Arc::new(AuthService::new(user_service.clone(), &jwt_secret, None)?);

        Ok(Self {
            auth_service,
            user_service,
            task_service,
        })
    }

    pub async fn run(&self, args: Args) -> Result<()> {
        // Setup logging
        if args.verbose {
            tracing_subscriber::fmt()
                .with_env_filter("debug")
                .init();
        } else {
            tracing_subscriber::fmt()
                .with_env_filter("info")
                .init();
        }

        match args.command {
            Commands::Auth { command } => self.handle_auth_command(command).await,
            Commands::Task { command } => self.handle_task_command(command).await,
            Commands::Config { command } => self.handle_config_command(command).await,
            Commands::Export { format, output } => self.handle_export_command(format, output).await,
            Commands::Import { file, merge } => self.handle_import_command(file, merge).await,
            Commands::Search { query, in_description } => self.handle_search_command(query, in_description).await,
            Commands::Stats { period } => self.handle_stats_command(period).await,
        }
    }

    // Authentication Commands
    async fn handle_auth_command(&self, command: AuthCommands) -> Result<()> {
        match command {
            AuthCommands::Register => self.handle_register().await,
            AuthCommands::Login => self.handle_login().await,
            AuthCommands::Logout => self.handle_logout().await,
            AuthCommands::Status => self.handle_auth_status().await,
        }
    }

    async fn handle_register(&self) -> Result<()> {
        println!("{} {}", ROCKET, style("User Registration").bold().cyan());

        let theme = ColorfulTheme::default();

        let username: String = Input::with_theme(&theme)
            .with_prompt("Username")
            .validate_with(|input: &String| -> Result<(), &str> {
                if input.len() < 3 {
                    Err("Username must be at least 3 characters")
                } else if input.len() > 50 {
                    Err("Username must be less than 50 characters")
                } else {
                    Ok(())
                }
            })
            .interact_text()?;

        let email: String = Input::with_theme(&theme)
            .with_prompt("Email")
            .validate_with(|input: &String| -> Result<(), &str> {
                if !input.contains('@') {
                    Err("Please enter a valid email address")
                } else {
                    Ok(())
                }
            })
            .interact_text()?;

        let password: String = Password::with_theme(&theme)
            .with_prompt("Password")
            .with_confirmation("Confirm password", "Passwords don't match")
            .validate_with(|input: &String| -> Result<(), &str> {
                if input.len() < 8 {
                    Err("Password must be at least 8 characters")
                } else {
                    Ok(())
                }
            })
            .interact()?;

        let request = StoreUserRequest::new(username, email, password)
            .context("Failed to create user request")?;

        match self.user_service.register(request).await {
            Ok(user) => {
                println!("{} User registered successfully!", CHECKMARK);
                println!("Username: {}", style(&user.username).green());
                println!("Email: {}", style(&user.email).green());
                info!("User {} registered successfully", user.username);
            }
            Err(UserServiceError::UsernameExists { username }) => {
                println!("{} Username '{}' already exists", CROSS, style(username).red());
            }
            Err(UserServiceError::EmailExists { email }) => {
                println!("{} Email '{}' already exists", CROSS, style(email).red());
            }
            Err(e) => {
                println!("{} Registration failed: {}", CROSS, style(&e).red());
                error!("Registration failed: {}", e);
            }
        }

        Ok(())
    }

    async fn handle_login(&self) -> Result<()> {
        println!("{} {}", ROCKET, style("User Login").bold().cyan());

        let theme = ColorfulTheme::default();

        let identifier: String = Input::with_theme(&theme)
            .with_prompt("Username or Email")
            .interact_text()?;

        let password: String = Password::with_theme(&theme)
            .with_prompt("Password")
            .interact()?;

        match self.auth_service.login(&identifier, &password).await {
            Ok(response) => {
                println!("{} Login successful!", CHECKMARK);
                println!("Welcome back, {}!", style(&response.user.username).green());
                println!("Session expires: {}", style(format_date(&response.expires_at)).yellow());
                info!("User {} logged in successfully", response.user.username);
            }
            Err(AuthServiceError::AuthenticationFailed) => {
                println!("{} Invalid username/email or password", CROSS);
                warn!("Login failed for user: {}", identifier);
            }
            Err(e) => {
                println!("{} Login failed: {}", CROSS, style(&e).red());
                error!("Login failed: {}", e);
            }
        }

        Ok(())
    }

    async fn handle_logout(&self) -> Result<()> {
        match self.auth_service.logout().await {
            Ok(_) => {
                println!("{} Logged out successfully", CHECKMARK);
                info!("User logged out successfully");
            }
            Err(e) => {
                println!("{} Logout failed: {}", CROSS, style(&e).red());
                error!("Logout failed: {}", e);
            }
        }

        Ok(())
    }

    async fn handle_auth_status(&self) -> Result<()> {
        match self.auth_service.get_current_session().await {
            Ok(Some(user)) => {
                println!("{} {}", INFO, style("Authentication Status").bold().cyan());
                println!("Status: {}", style("Authenticated").green());
                println!("Username: {}", style(&user.username).green());
                println!("Email: {}", style(&user.email).green());
                println!("User ID: {}", style(&user.id).dim());
            }
            Ok(None) => {
                println!("{} {}", WARNING, style("Not authenticated").yellow());
                println!("Use {} to login", style("todo-cli auth login").cyan());
            }
            Err(e) => {
                println!("{} Failed to check authentication status: {}", CROSS, style(&e).red());
                error!("Failed to check auth status: {}", e);
            }
        }

        Ok(())
    }

    // Task Commands
    async fn handle_task_command(&self, command: TaskCommands) -> Result<()> {
        // Check if user is authenticated
        let user = match self.auth_service.get_current_user().await {
            Ok(user) => user,
            Err(AuthServiceError::SessionNotFound) => {
                println!("{} Please login first: {}", WARNING, style("todo-cli auth login").cyan());
                return Ok(());
            }
            Err(e) => {
                println!("{} Authentication error: {}", CROSS, style(e).red());
                return Ok(());
            }
        };

        match command {
            TaskCommands::Add { title, description, priority, due } => {
                self.handle_add_task(&user, title, description, priority, due).await
            }
            TaskCommands::List { status, priority, search, completed, pending } => {
                self.handle_list_tasks(&user, status, priority, search, completed, pending).await
            }
            TaskCommands::Update { id, title, description, priority, due } => {
                self.handle_update_task(&user, id, title, description, priority, due).await
            }
            TaskCommands::Complete { id } => {
                self.handle_complete_task(&user, id, true).await
            }
            TaskCommands::Uncomplete { id } => {
                self.handle_complete_task(&user, id, false).await
            }
            TaskCommands::Delete { id, force } => {
                self.handle_delete_task(&user, id, force).await
            }
            TaskCommands::Show { id } => {
                self.handle_show_task(&user, id).await
            }
        }
    }

    async fn handle_add_task(&self, user: &UserResponse, title: String, description: Option<String>, priority: TaskPriority, due: Option<String>) -> Result<()> {
        let parsed_due = if let Some(due_str) = due {
            Some(chrono::NaiveDate::parse_from_str(&due_str, "%Y-%m-%d")
                .context("Invalid date format. Use YYYY-MM-DD")?
                .and_hms_opt(23, 59, 59)
                .unwrap()
                .and_utc())
        } else {
            None
        };

        let model_priority = match priority {
            TaskPriority::Low => ModelTaskPriority::Low,
            TaskPriority::Medium => ModelTaskPriority::Medium,
            TaskPriority::High => ModelTaskPriority::High,
        };

        let request = StoreTaskRequest {
            title,
            description,
            status: ModelTaskStatus::Pending,
            priority: model_priority,
            due_date: parsed_due,
        };

        match self.task_service.create_task(&user.id, request).await {
            Ok(task) => {
                println!("{} Task created successfully!", CHECKMARK);
                println!("ID: {}", style(&task.id).cyan());
                println!("Title: {}", style(&task.title).green());
                if let Some(desc) = &task.description {
                    println!("Description: {}", style(desc).dim());
                }
                println!("Priority: {}", style(format!("{:?}", task.priority)).yellow());
                info!("Task created: {}", task.title);
            }
            Err(e) => {
                println!("{} Failed to create task: {}", CROSS, style(&e).red());
                error!("Failed to create task: {}", e);
            }
        }

        Ok(())
    }

    async fn handle_list_tasks(&self, user: &UserResponse, status: Option<TaskStatus>, priority: Option<TaskPriority>, search: Option<String>, completed: bool, pending: bool) -> Result<()> {
        let mut filter = TaskFilter::default();

        // Apply status filters
        if completed {
            filter.status = Some(ModelTaskStatus::Completed);
        } else if pending {
            filter.status = Some(ModelTaskStatus::Pending);
        } else if let Some(status) = status {
            filter.status = Some(match status {
                TaskStatus::Pending => ModelTaskStatus::Pending,
                TaskStatus::InProgress => ModelTaskStatus::InProgress,
                TaskStatus::Completed => ModelTaskStatus::Completed,
            });
        }

        // Apply priority filter
        if let Some(priority) = priority {
            filter.priority = Some(match priority {
                TaskPriority::Low => ModelTaskPriority::Low,
                TaskPriority::Medium => ModelTaskPriority::Medium,
                TaskPriority::High => ModelTaskPriority::High,
            });
        }

        // Apply search filter
        if let Some(search) = search {
            filter.search_term = Some(search);
        }

        match self.task_service.get_tasks(&user.id, filter).await {
            Ok(tasks) => {
                if tasks.is_empty() {
                    println!("{} No tasks found", INFO);
                } else {
                    println!("{} {}", INFO, style(format!("Found {} tasks", tasks.len())).bold());
                    let table = format_task_table(&tasks);
                    println!("{}", table);
                }
            }
            Err(e) => {
                println!("{} Failed to list tasks: {}", CROSS, style(&e).red());
                error!("Failed to list tasks: {}", e);
            }
        }

        Ok(())
    }

    async fn handle_update_task(&self, user: &UserResponse, id: String, title: Option<String>, description: Option<String>, priority: Option<TaskPriority>, due: Option<String>) -> Result<()> {
        let task_id = Uuid::parse_str(&id).context("Invalid task ID format")?;

        let parsed_due = if let Some(due_str) = due {
            Some(chrono::NaiveDate::parse_from_str(&due_str, "%Y-%m-%d")
                .context("Invalid date format. Use YYYY-MM-DD")?
                .and_hms_opt(23, 59, 59)
                .unwrap()
                .and_utc())
        } else {
            None
        };

        let model_priority = priority.map(|p| match p {
            TaskPriority::Low => ModelTaskPriority::Low,
            TaskPriority::Medium => ModelTaskPriority::Medium,
            TaskPriority::High => ModelTaskPriority::High,
        });

        let updates = UpdateTaskRequest {
            title,
            description,
            priority: model_priority,
            due_date: parsed_due,
            ..Default::default()
        };

        match self.task_service.update_task(&user.id, &task_id, updates).await {
            Ok(task) => {
                println!("{} Task updated successfully!", CHECKMARK);
                println!("{}", format_task_detail(&task));
                info!("Task updated: {}", task.title);
            }
            Err(e) => {
                println!("{} Failed to update task: {}", CROSS, style(&e).red());
                error!("Failed to update task: {}", e);
            }
        }

        Ok(())
    }

    async fn handle_complete_task(&self, user: &UserResponse, id: String, complete: bool) -> Result<()> {
        let task_id = Uuid::parse_str(&id).context("Invalid task ID format")?;

        let result = if complete {
            self.task_service.complete_task(&user.id, &task_id).await
        } else {
            let updates = UpdateTaskRequest {
                status: Some(ModelTaskStatus::Pending),
                ..Default::default()
            };
            self.task_service.update_task(&user.id, &task_id, updates).await
        };

        match result {
            Ok(task) => {
                let action = if complete { "completed" } else { "marked as pending" };
                println!("{} Task {} successfully!", CHECKMARK, action);
                println!("{}", format_task_detail(&task));
                info!("Task {}: {}", action, task.title);
            }
            Err(e) => {
                let action = if complete { "complete" } else { "uncomplete" };
                println!("{} Failed to {} task: {}", CROSS, action, style(&e).red());
                error!("Failed to {} task: {}", action, e);
            }
        }

        Ok(())
    }

    async fn handle_delete_task(&self, user: &UserResponse, id: String, force: bool) -> Result<()> {
        let task_id = Uuid::parse_str(&id).context("Invalid task ID format")?;

        // Confirm deletion unless force flag is used
        if !force {
            let theme = ColorfulTheme::default();
            let confirm = Confirm::with_theme(&theme)
                .with_prompt("Are you sure you want to delete this task?")
                .default(false)
                .interact()?;

            if !confirm {
                println!("Task deletion cancelled");
                return Ok(());
            }
        }

        match self.task_service.delete_task(&user.id, &task_id).await {
            Ok(_) => {
                println!("{} Task deleted successfully!", CHECKMARK);
                info!("Task deleted: {}", task_id);
            }
            Err(e) => {
                println!("{} Failed to delete task: {}", CROSS, style(&e).red());
                error!("Failed to delete task: {}", e);
            }
        }

        Ok(())
    }

    async fn handle_show_task(&self, user: &UserResponse, id: String) -> Result<()> {
        let task_id = Uuid::parse_str(&id).context("Invalid task ID format")?;

        match self.task_service.get_task(&user.id, &task_id).await {
            Ok(task) => {
                println!("{} {}", INFO, style("Task Details").bold().cyan());
                println!("{}", format_task_detail(&task));
            }
            Err(e) => {
                println!("{} Failed to get task: {}", CROSS, style(&e).red());
                error!("Failed to get task: {}", e);
            }
        }

        Ok(())
    }

    // Config Commands (placeholder implementation)
    async fn handle_config_command(&self, command: ConfigCommands) -> Result<()> {
        match command {
            ConfigCommands::Show => {
                println!("{} Configuration management is not yet implemented", WARNING);
            }
            ConfigCommands::Set { key: _, value: _ } => {
                println!("{} Configuration management is not yet implemented", WARNING);
            }
            ConfigCommands::Get { key: _ } => {
                println!("{} Configuration management is not yet implemented", WARNING);
            }
            ConfigCommands::Reset => {
                println!("{} Configuration management is not yet implemented", WARNING);
            }
        }
        Ok(())
    }

    // Export Commands (placeholder implementation)
    async fn handle_export_command(&self, _format: ExportFormat, _output: Option<String>) -> Result<()> {
        println!("{} Export functionality is not yet implemented", WARNING);
        Ok(())
    }

    // Import Commands (placeholder implementation)
    async fn handle_import_command(&self, _file: String, _merge: bool) -> Result<()> {
        println!("{} Import functionality is not yet implemented", WARNING);
        Ok(())
    }

    // Search Commands (placeholder implementation)
    async fn handle_search_command(&self, _query: String, _in_description: bool) -> Result<()> {
        println!("{} Search functionality is not yet implemented", WARNING);
        Ok(())
    }

    // Stats Commands (placeholder implementation)
    async fn handle_stats_command(&self, _period: StatsPeriod) -> Result<()> {
        println!("{} Statistics functionality is not yet implemented", WARNING);
        Ok(())
    }
}