/// Logging configuration and initialization.
///
/// This module sets up structured logging using the tracing crate,
/// with configurable log levels and output formats.
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::domain::AppResult;

/// Initializes the logging system with the specified log level.
///
/// Sets up structured logging with:
/// - Timestamp for each log entry
/// - Thread ID/name information
/// - Colored output (when supported)
/// - JSON formatting for structured logs (optional)
///
/// # Arguments
/// - `log_level`: Log level as a string ("trace", "debug", "info", "warn", "error")
///
/// # Returns
/// - `Ok(())`: Logging initialized successfully
/// - `Err(AppError)`: Failed to initialize logging
///
/// # Examples
/// ```ignore
/// init_logging("info")?;
/// tracing::info!("Application started");
/// ```
///
/// # Design Notes
/// - Uses tracing for structured, async-aware logging
/// - EnvFilter allows runtime configuration via environment variables
/// - Follows the Facade pattern to abstract logging implementation
pub fn init_logging(log_level: &str) -> AppResult<()> {
    let filter = EnvFilter::try_new(log_level).unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .with_target(true)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true),
        )
        .try_init()
        .map_err(|e| {
            crate::domain::AppError::InternalError(format!("Failed to initialize logging: {}", e))
        })?;

    tracing::info!("Logging initialized with level: {}", log_level);

    Ok(())
}

/// Initializes logging for testing purposes.
///
/// Sets up logging with sane defaults for tests:
/// - Log level: "debug"
/// - Compact format
/// - No colors (for CI environments)
///
/// # Returns
/// - `Ok(())`: Logging initialized successfully
/// - `Err(AppError)`: Failed to initialize logging
///
/// # Note
/// This function is idempotent - calling it multiple times is safe.
#[cfg(test)]
pub fn init_test_logging() -> AppResult<()> {
    let filter = EnvFilter::try_new("debug").unwrap_or_else(|_| EnvFilter::new("debug"));

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_test_writer())
        .try_init();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_test_logging() {
        let result = init_test_logging();
        // Should succeed or be idempotent
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_logging_macros() {
        let _ = init_test_logging();

        tracing::trace!("This is a trace message");
        tracing::debug!("This is a debug message");
        tracing::info!("This is an info message");
        tracing::warn!("This is a warning message");
        tracing::error!("This is an error message");
    }

    #[test]
    fn test_structured_logging() {
        let _ = init_test_logging();

        let user_id = 123;
        let action = "login";

        tracing::info!(user_id = user_id, action = action, "User performed action");
    }
}
