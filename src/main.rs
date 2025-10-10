/// Main entry point for the GitHub Trending Summary application.
use github_trending_summary::{init_logging, AppConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration from environment
    let config = AppConfig::from_env()?;

    // Initialize logging
    init_logging(config.log_level())?;

    tracing::info!("GitHub Trending Summary application started");
    tracing::info!("Target language: {:?}", config.target_language());

    // TODO: Initialize dependencies and run the application
    // This will be implemented in future iterations

    println!("Hello, GitHub Trending Summary!");

    Ok(())
}
