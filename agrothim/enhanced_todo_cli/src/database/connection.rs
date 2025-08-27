use anyhow::Context;
use sqlx::PgPool;

#[derive(Debug, Clone)]
pub struct Database {
    pool: PgPool,
}

pub struct ConnectionConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
}

impl Database {
    pub async fn new(config: &ConnectionConfig) -> Result<Self, anyhow::Error> {
        tracing::info!("Connecting to database...");
        let pool = PgPool::connect_with(
            sqlx::postgres::PgConnectOptions::new()
                .host(&config.host)
                .port(config.port)
                .username(&config.username)
                .password(&config.password)
                .database(&config.database),
        )
        .await
        .context("Failed to connect to database")?;

        tracing::info!("Database connected successfully");
        Ok(Database { pool })
    }

    pub async fn from_url(database_url: &str) -> Result<Self, anyhow::Error> {
        tracing::info!("Connecting to database from URL: {}", 
            database_url.replace(char::is_alphanumeric, "*")); // Hide credentials
        let pool = PgPool::connect(database_url)
            .await
            .context("Failed to connect to database")?;

        tracing::info!("Database connected successfully");
        Ok(Database { pool })
    }

    pub async fn health_check(&self) -> Result<bool, anyhow::Error> {
        let health_check = sqlx::query!("SELECT 1 as health_check")
            .fetch_one(&self.pool)
            .await
            .context("Failed to perform health check")?;

        tracing::info!("Database health check passed");
        Ok(health_check.health_check.unwrap_or(0) == 1)
    }

    pub async fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn close(&self) -> Result<(), anyhow::Error> {
        self.pool
            .close()
            .await;
        tracing::info!("Database connection closed successfully");
        Ok(())
    }
}
