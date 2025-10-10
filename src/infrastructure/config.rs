/// Configuration management for the application.
///
/// This module handles loading configuration from environment variables
/// and configuration files, with proper validation and error handling.
use serde::Deserialize;

use crate::domain::{AppError, AppResult};

/// Application configuration structure.
///
/// This struct holds all configuration values needed to run the application.
/// Values are loaded from environment variables using dotenvy.
///
/// # Environment Variables
/// - `DISCORD_WEBHOOK_URL`: Discord webhook URL for notifications (required)
/// - `GEMINI_API_KEY`: Gemini API key for AI summarization (required)
/// - `GITHUB_TOKEN`: GitHub personal access token (optional, for higher rate limits)
/// - `TARGET_LANGUAGE`: Default language filter for trending repos (optional)
/// - `LOG_LEVEL`: Logging level (default: "info")
///
/// # Design Notes
/// - Uses builder pattern for flexible construction
/// - Implements validation in the constructor
/// - Separates required and optional fields clearly
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    discord_webhook_url: String,
    gemini_api_key: String,
    github_token: Option<String>,
    target_language: Option<String>,
    log_level: String,
}

impl AppConfig {
    /// Loads configuration from environment variables.
    ///
    /// # Returns
    /// - `Ok(AppConfig)`: Successfully loaded and validated configuration
    /// - `Err(AppError)`: Configuration error if required values are missing or invalid
    ///
    /// # Examples
    /// ```ignore
    /// let config = AppConfig::from_env()?;
    /// println!("Log level: {}", config.log_level());
    /// ```
    pub fn from_env() -> AppResult<Self> {
        // Load .env file if present (ignore error if not found)
        let _ = dotenvy::dotenv();

        // Load required fields
        let discord_webhook_url = std::env::var("DISCORD_WEBHOOK_URL").map_err(|_| {
            AppError::ConfigError(
                "DISCORD_WEBHOOK_URL environment variable is required".to_string(),
            )
        })?;

        let gemini_api_key = std::env::var("GEMINI_API_KEY").map_err(|_| {
            AppError::ConfigError("GEMINI_API_KEY environment variable is required".to_string())
        })?;

        // Load optional fields
        let github_token = std::env::var("GITHUB_TOKEN").ok();
        let target_language = std::env::var("TARGET_LANGUAGE").ok();
        let log_level = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());

        let config = Self {
            discord_webhook_url,
            gemini_api_key,
            github_token,
            target_language,
            log_level,
        };

        config.validate()?;

        Ok(config)
    }

    /// Validates the configuration values.
    ///
    /// # Returns
    /// - `Ok(())`: Configuration is valid
    /// - `Err(AppError)`: Configuration contains invalid values
    fn validate(&self) -> AppResult<()> {
        // Validate Discord webhook URL
        if !self
            .discord_webhook_url
            .starts_with("https://discord.com/api/webhooks/")
            && !self
                .discord_webhook_url
                .starts_with("https://discordapp.com/api/webhooks/")
        {
            return Err(AppError::ConfigError(
                "DISCORD_WEBHOOK_URL must be a valid Discord webhook URL".to_string(),
            ));
        }

        // Validate Gemini API key (basic check)
        if self.gemini_api_key.trim().is_empty() {
            return Err(AppError::ConfigError(
                "GEMINI_API_KEY cannot be empty".to_string(),
            ));
        }

        // Validate log level
        let valid_log_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_log_levels.contains(&self.log_level.to_lowercase().as_str()) {
            return Err(AppError::ConfigError(format!(
                "LOG_LEVEL must be one of: {}",
                valid_log_levels.join(", ")
            )));
        }

        Ok(())
    }

    /// Returns the Discord webhook URL.
    pub fn discord_webhook_url(&self) -> &str {
        &self.discord_webhook_url
    }

    /// Returns the Gemini API key.
    pub fn gemini_api_key(&self) -> &str {
        &self.gemini_api_key
    }

    /// Returns the GitHub token, if configured.
    pub fn github_token(&self) -> Option<&str> {
        self.github_token.as_deref()
    }

    /// Returns the target language filter, if configured.
    pub fn target_language(&self) -> Option<&str> {
        self.target_language.as_deref()
    }

    /// Returns the log level.
    pub fn log_level(&self) -> &str {
        &self.log_level
    }
}

/// Builder for creating AppConfig for testing purposes.
///
/// This builder allows creating configuration with custom values,
/// useful for testing without relying on environment variables.
#[cfg(test)]
pub struct AppConfigBuilder {
    discord_webhook_url: Option<String>,
    gemini_api_key: Option<String>,
    github_token: Option<String>,
    target_language: Option<String>,
    log_level: Option<String>,
}

#[cfg(test)]
impl Default for AppConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl AppConfigBuilder {
    /// Creates a new builder with default test values.
    pub fn new() -> Self {
        Self {
            discord_webhook_url: Some("https://discord.com/api/webhooks/test".to_string()),
            gemini_api_key: Some("test_api_key".to_string()),
            github_token: None,
            target_language: None,
            log_level: Some("info".to_string()),
        }
    }

    /// Sets the Discord webhook URL.
    pub fn discord_webhook_url(mut self, url: String) -> Self {
        self.discord_webhook_url = Some(url);
        self
    }

    /// Sets the Gemini API key.
    pub fn gemini_api_key(mut self, key: String) -> Self {
        self.gemini_api_key = Some(key);
        self
    }

    /// Sets the GitHub token.
    pub fn github_token(mut self, token: String) -> Self {
        self.github_token = Some(token);
        self
    }

    /// Sets the target language.
    pub fn target_language(mut self, language: String) -> Self {
        self.target_language = Some(language);
        self
    }

    /// Sets the log level.
    pub fn log_level(mut self, level: String) -> Self {
        self.log_level = Some(level);
        self
    }

    /// Builds the AppConfig.
    pub fn build(self) -> AppConfig {
        AppConfig {
            discord_webhook_url: self
                .discord_webhook_url
                .unwrap_or_else(|| "https://discord.com/api/webhooks/test".to_string()),
            gemini_api_key: self
                .gemini_api_key
                .unwrap_or_else(|| "test_api_key".to_string()),
            github_token: self.github_token,
            target_language: self.target_language,
            log_level: self.log_level.unwrap_or_else(|| "info".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = AppConfigBuilder::new()
            .discord_webhook_url("https://discord.com/api/webhooks/123456".to_string())
            .gemini_api_key("test_key".to_string())
            .github_token("ghp_test".to_string())
            .target_language("rust".to_string())
            .log_level("debug".to_string())
            .build();

        assert_eq!(
            config.discord_webhook_url(),
            "https://discord.com/api/webhooks/123456"
        );
        assert_eq!(config.gemini_api_key(), "test_key");
        assert_eq!(config.github_token(), Some("ghp_test"));
        assert_eq!(config.target_language(), Some("rust"));
        assert_eq!(config.log_level(), "debug");
    }

    #[test]
    fn test_validate_invalid_discord_url() {
        let config = AppConfig {
            discord_webhook_url: "https://invalid.com/webhook".to_string(),
            gemini_api_key: "test_key".to_string(),
            github_token: None,
            target_language: None,
            log_level: "info".to_string(),
        };

        let result = config.validate();
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::ConfigError(msg) => {
                assert!(msg.contains("Discord webhook URL"));
            }
            _ => panic!("Expected ConfigError"),
        }
    }

    #[test]
    fn test_validate_empty_gemini_key() {
        let config = AppConfig {
            discord_webhook_url: "https://discord.com/api/webhooks/test".to_string(),
            gemini_api_key: "".to_string(),
            github_token: None,
            target_language: None,
            log_level: "info".to_string(),
        };

        let result = config.validate();
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::ConfigError(msg) => {
                assert!(msg.contains("GEMINI_API_KEY"));
            }
            _ => panic!("Expected ConfigError"),
        }
    }

    #[test]
    fn test_validate_invalid_log_level() {
        let config = AppConfig {
            discord_webhook_url: "https://discord.com/api/webhooks/test".to_string(),
            gemini_api_key: "test_key".to_string(),
            github_token: None,
            target_language: None,
            log_level: "invalid".to_string(),
        };

        let result = config.validate();
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::ConfigError(msg) => {
                assert!(msg.contains("LOG_LEVEL"));
            }
            _ => panic!("Expected ConfigError"),
        }
    }

    #[test]
    fn test_validate_valid_config() {
        let config = AppConfigBuilder::new().build();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_from_env_missing_required_vars() {
        // Clear environment variables
        std::env::remove_var("DISCORD_WEBHOOK_URL");
        std::env::remove_var("GEMINI_API_KEY");

        let result = AppConfig::from_env();
        assert!(result.is_err());
    }

    #[test]
    fn test_discordapp_webhook_url_valid() {
        let config = AppConfig {
            discord_webhook_url: "https://discordapp.com/api/webhooks/123".to_string(),
            gemini_api_key: "test_key".to_string(),
            github_token: None,
            target_language: None,
            log_level: "info".to_string(),
        };

        assert!(config.validate().is_ok());
    }
}
