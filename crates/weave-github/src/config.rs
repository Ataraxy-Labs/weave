use std::sync::Arc;

use crate::error::ConfigError;

/// Server configuration loaded from environment variables.
#[derive(Clone)]
pub struct Config {
    /// GitHub App ID.
    pub app_id: u64,
    /// GitHub App private key (PEM format).
    pub private_key: String,
    /// Webhook secret for HMAC-SHA256 verification.
    pub webhook_secret: String,
    /// Port to listen on.
    pub port: u16,
}

impl Config {
    /// Load configuration from environment variables.
    ///
    /// Required: GITHUB_APP_ID, GITHUB_PRIVATE_KEY, GITHUB_WEBHOOK_SECRET
    /// Optional: PORT (default 8080)
    pub fn from_env() -> Result<Arc<Self>, ConfigError> {
        let required =
            |name: &'static str| std::env::var(name).map_err(|_| ConfigError::Missing { name });

        let raw_app_id = required("GITHUB_APP_ID")?;
        let app_id: u64 = raw_app_id.parse().map_err(|_| ConfigError::Malformed {
            name: "GITHUB_APP_ID",
            expected: "a number",
            found: raw_app_id.clone(),
        })?;

        let private_key = required("GITHUB_PRIVATE_KEY")?;
        let webhook_secret = required("GITHUB_WEBHOOK_SECRET")?;

        let raw_port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
        let port: u16 = raw_port.parse().map_err(|_| ConfigError::Malformed {
            name: "PORT",
            expected: "a port number",
            found: raw_port.clone(),
        })?;

        Ok(Arc::new(Config {
            app_id,
            private_key,
            webhook_secret,
            port,
        }))
    }
}
