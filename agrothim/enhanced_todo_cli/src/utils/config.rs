use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub log_level: String,
    pub environment: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenv::dotenv().ok();
        let config = Config {
            database_url: env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL is not set"))?
                .to_string(),
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or("secret".to_string())
                .to_string(),
            log_level: env::var("LOG_LEVEL")
                .unwrap_or("info".to_string())
                .to_string(),
            environment: env::var("APP_ENV")
                .unwrap_or("development".to_string())
                .to_string(),
        };

        tracing::info!("Config: successfully loaded for {} environment", config.environment);
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), anyhow::Error> {
        if self.database_url.is_empty() {
            return Err(anyhow::anyhow!("DATABASE_URL is not set"));
        }

        if !self.database_url.starts_with("postgres://") {
            return Err(anyhow::anyhow!(
                "DATABASE_URL must start with 'postgres://'"
            ));
        }

        if self.is_production() && self.jwt_secret == "default-secret-change-me" {
            return Err(anyhow::anyhow!("JWT_SECRET is not set in production"));
        }

        Ok(())
    }

    pub fn is_production(&self) -> bool {
        self.environment == "production"
    }
}
