use anyhow::Result;
use enhanced_todo_cli::{database::Database, utils::Config};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!("🦀 Enhanced Todo CLI starting...");

    let config = Config::from_env().map_err(|e| {
        tracing::error!("Failed to load configuration: {}", e);
        e
    })?;
    tracing::info!(
        "Configuration loaded for {} environment",
        config.environment
    );

    let database = Database::from_url(&config.database_url).await?;

    match database.health_check().await {
        Ok(true) => tracing::info!("Database health check passed"),
        Ok(false) => {
            tracing::error!("Database health check failed. Please check your database connection.");
            return Err(anyhow::anyhow!("Database health check failed"));
        }
        Err(e) => {
            tracing::error!("Failed to perform health check: {}", e);
            return Err(e);
        }
    }
    database.close().await?;
    tracing::info!("🦀 Enhanced Todo CLI stopped");
    Ok(())
}
