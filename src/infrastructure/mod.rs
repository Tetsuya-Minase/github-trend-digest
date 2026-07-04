/// Infrastructure layer containing implementations of domain interfaces.
///
/// This layer contains concrete implementations of repositories, clients,
/// and services defined in the domain layer. It handles external communication
/// and infrastructure concerns.
pub mod config;
pub mod logging;
pub mod scraper;
pub mod discord;

// Re-export commonly used types
pub use config::AppConfig;
pub use logging::init_logging;
pub use scraper::TrendingScraperClient;
pub use discord::DiscordNotificationSender;
