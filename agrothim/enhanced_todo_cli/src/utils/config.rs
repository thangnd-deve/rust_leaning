use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub environment: String,
}

impl Config {
    #[allow(dead_code)]
    pub fn from_env() -> anyhow::Result<Self> {
        dotenv::dotenv().ok();
        let config = Config {
            database_url: env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL is not set"))?
                .to_string(),
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or("secret".to_string())
                .to_string(),
            environment: env::var("APP_ENV")
                .unwrap_or("development".to_string())
                .to_string(),
        };

        tracing::info!("Config: successfully loaded for {} environment", config.environment);
        config.validate()?;
        Ok(config)
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    pub fn is_production(&self) -> bool {
        self.environment == "production"
    }
}
