//! Infrastructure layer containing implementations of domain interfaces.
//!
//! This layer contains concrete implementations of repositories, clients,
//! and services defined in the domain layer. It handles external communication
//! and infrastructure concerns.
pub mod config;
pub mod discord;
pub mod github;
pub mod logging;
pub mod scraper;

// Re-export commonly used types
pub use config::AppConfig;
pub use discord::DiscordNotificationSender;
pub use github::GithubApiClientImpl;
pub use logging::init_logging;
pub use scraper::TrendingScraperClient;
