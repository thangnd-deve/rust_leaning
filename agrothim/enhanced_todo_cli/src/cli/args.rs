use clap::{Parser, Subcommand, ValueEnum};
use std::fmt;

#[derive(Parser)]
#[command(name = "todo-cli")]
#[command(about = "A comprehensive TODO CLI application with user management")]
#[command(version = "0.1.0")]
#[command(author = "Thang Nguyen <thangnd.deve@gmail.com>")]
pub struct Args {
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Configuration file path
    #[arg(short, long, global = true)]
    pub config: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Authentication related commands
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },
    /// Task management commands
    Task {
        #[command(subcommand)]
        command: TaskCommands,
    },
    /// Configuration management
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    /// Export data
    Export {
        /// Output format
        #[arg(short, long, default_value = "json")]
        format: ExportFormat,
        /// Output file path
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Import data
    Import {
        /// Input file path
        #[arg(short, long)]
        file: String,
        /// Merge with existing data instead of replacing
        #[arg(short, long)]
        merge: bool,
    },
    /// Search tasks
    Search {
        /// Search query
        query: String,
        /// Search in description instead of title
        #[arg(long)]
        in_description: bool,
    },
    /// Show statistics
    Stats {
        /// Time period for statistics
        #[arg(short, long, default_value = "all")]
        period: StatsPeriod,
    },
}

#[derive(Subcommand)]
pub enum AuthCommands {
    /// Register a new user account
    Register,
    /// Login to an existing account
    Login,
    /// Logout from current session
    Logout,
    /// Show current authentication status
    Status,
}

#[derive(Subcommand)]
pub enum TaskCommands {
    /// Add a new task
    Add {
        /// Task title
        title: String,
        /// Task description
        #[arg(short, long)]
        description: Option<String>,
        /// Task priority
        #[arg(short, long, default_value = "medium")]
        priority: TaskPriority,
        /// Due date (YYYY-MM-DD format)
        #[arg(long)]
        due: Option<String>,
    },
    /// List tasks with optional filtering
    List {
        /// Filter by task status
        #[arg(short, long)]
        status: Option<TaskStatus>,
        /// Filter by priority
        #[arg(short, long)]
        priority: Option<TaskPriority>,
        /// Search keyword
        #[arg(long)]
        search: Option<String>,
        /// Show completed tasks only
        #[arg(short, long)]
        completed: bool,
        /// Show pending tasks only
        #[arg(long)]
        pending: bool,
    },
    /// Update an existing task
    Update {
        /// Task ID
        id: String,
        /// New title
        #[arg(short, long)]
        title: Option<String>,
        /// New description
        #[arg(short, long)]
        description: Option<String>,
        /// New priority
        #[arg(short, long)]
        priority: Option<TaskPriority>,
        /// New due date (YYYY-MM-DD format)
        #[arg(long)]
        due: Option<String>,
    },
    /// Mark task as completed
    Complete {
        /// Task ID
        id: String,
    },
    /// Mark task as pending (uncomplete)
    Uncomplete {
        /// Task ID
        id: String,
    },
    /// Delete a task
    Delete {
        /// Task ID
        id: String,
        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },
    /// Show detailed information about a task
    Show {
        /// Task ID
        id: String,
    },
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Show current configuration
    Show,
    /// Set a configuration value
    Set {
        /// Configuration key
        key: String,
        /// Configuration value
        value: String,
    },
    /// Get a configuration value
    Get {
        /// Configuration key
        key: String,
    },
    /// Reset configuration to defaults
    Reset,
}

#[derive(Clone, ValueEnum)]
pub enum TaskPriority {
    Low,
    Medium,
    High,
}

impl fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskPriority::Low => write!(f, "low"),
            TaskPriority::Medium => write!(f, "medium"),
            TaskPriority::High => write!(f, "high"),
        }
    }
}

#[derive(Clone, ValueEnum)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::InProgress => write!(f, "in_progress"),
            TaskStatus::Completed => write!(f, "completed"),
        }
    }
}

#[derive(Clone, ValueEnum)]
pub enum ExportFormat {
    Json,
    Csv,
}

impl fmt::Display for ExportFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportFormat::Json => write!(f, "json"),
            ExportFormat::Csv => write!(f, "csv"),
        }
    }
}

#[derive(Clone, ValueEnum)]
pub enum StatsPeriod {
    Day,
    Week,
    Month,
    Year,
    All,
}

impl fmt::Display for StatsPeriod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatsPeriod::Day => write!(f, "day"),
            StatsPeriod::Week => write!(f, "week"),
            StatsPeriod::Month => write!(f, "month"),
            StatsPeriod::Year => write!(f, "year"),
            StatsPeriod::All => write!(f, "all"),
        }
    }
}