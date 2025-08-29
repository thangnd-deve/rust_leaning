mod models;
mod database;
mod services;
mod cli;
mod utils;
mod api;

use clap::Parser;
use anyhow::Result;
use tracing::{error, info};

use cli::{Args, CliApp};

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize the CLI application
    match CliApp::new().await {
        Ok(app) => {
            info!("🦀 Enhanced Todo CLI started");
            
            // Run the CLI command
            if let Err(e) = app.run(args).await {
                error!("Application error: {}", e);
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Err(e) => {
            error!("Failed to initialize application: {}", e);
            eprintln!("Failed to initialize application: {}", e);
            
            // Check if it's a database connection error
            if e.to_string().contains("DATABASE_URL") {
                eprintln!("\nPlease ensure:");
                eprintln!("1. DATABASE_URL environment variable is set");
                eprintln!("2. PostgreSQL database is running");
                eprintln!("3. Database migrations have been applied");
                eprintln!("\nExample:");
                eprintln!("  export DATABASE_URL=\"postgres://username:password@localhost/todo_cli\"");
                eprintln!("  sqlx migrate run");
            }
            
            std::process::exit(1);
        }
    }

    Ok(())
}
