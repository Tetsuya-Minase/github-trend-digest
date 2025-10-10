/// Application-wide error types.
///
/// This module defines a comprehensive error hierarchy using thiserror
/// for better error handling and propagation throughout the application.
use thiserror::Error;

/// Main application error type encompassing all possible error conditions.
///
/// This enum represents all errors that can occur in the application,
/// organized by the domain or infrastructure layer where they originate.
#[derive(Debug, Error)]
pub enum AppError {
    /// Error occurred during web scraping operations.
    #[error("Scraping failed: {0}")]
    ScrapingError(String),

    /// Error occurred when calling the GitHub API.
    #[error("GitHub API error: {0}")]
    GithubApiError(String),

    /// Error occurred when calling an AI service.
    #[error("AI service error: {0}")]
    AiServiceError(String),

    /// Error occurred when sending notifications.
    #[error("Notification error: {0}")]
    NotificationError(String),

    /// Error occurred when loading or parsing configuration.
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Error occurred during network operations.
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Error occurred during data parsing or serialization.
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Error occurred due to invalid input or validation failure.
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// Error occurred during I/O operations.
    #[error("I/O error: {0}")]
    IoError(String),

    /// Unexpected internal error.
    #[error("Internal error: {0}")]
    InternalError(String),
}

// Conversion implementations for common error types

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::IoError(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::ParseError(err.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::NetworkError(err.to_string())
    }
}

impl From<config::ConfigError> for AppError {
    fn from(err: config::ConfigError) -> Self {
        AppError::ConfigError(err.to_string())
    }
}

/// Result type alias for operations that may fail with AppError.
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let error = AppError::ScrapingError("Failed to parse HTML".to_string());
        assert_eq!(error.to_string(), "Scraping failed: Failed to parse HTML");

        let error = AppError::GithubApiError("Rate limit exceeded".to_string());
        assert_eq!(error.to_string(), "GitHub API error: Rate limit exceeded");

        let error = AppError::AiServiceError("API key invalid".to_string());
        assert_eq!(error.to_string(), "AI service error: API key invalid");

        let error = AppError::NotificationError("Webhook URL invalid".to_string());
        assert_eq!(error.to_string(), "Notification error: Webhook URL invalid");

        let error = AppError::ConfigError("Missing required field".to_string());
        assert_eq!(
            error.to_string(),
            "Configuration error: Missing required field"
        );

        let error = AppError::ValidationError("Invalid URL format".to_string());
        assert_eq!(error.to_string(), "Validation error: Invalid URL format");
    }

    #[test]
    fn test_error_conversion_from_io_error() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let app_error: AppError = io_error.into();

        match app_error {
            AppError::IoError(msg) => assert!(msg.contains("File not found")),
            _ => panic!("Expected IoError variant"),
        }
    }

    #[test]
    fn test_error_conversion_from_serde_error() {
        let json = r#"{"invalid": json}"#;
        let serde_error: serde_json::Error =
            serde_json::from_str::<serde_json::Value>(json).unwrap_err();
        let app_error: AppError = serde_error.into();

        match app_error {
            AppError::ParseError(_) => {}
            _ => panic!("Expected ParseError variant"),
        }
    }

    #[test]
    fn test_error_conversion_from_config_error() {
        let config_error = config::ConfigError::NotFound("test".to_string());
        let app_error: AppError = config_error.into();

        match app_error {
            AppError::ConfigError(_) => {}
            _ => panic!("Expected ConfigError variant"),
        }
    }

    #[test]
    fn test_app_result_type() {
        let success: AppResult<i32> = Ok(42);
        assert!(success.is_ok());
        if let Ok(value) = success {
            assert_eq!(value, 42);
        }

        let failure: AppResult<i32> = Err(AppError::InternalError("Test error".to_string()));
        assert!(failure.is_err());
    }
}
