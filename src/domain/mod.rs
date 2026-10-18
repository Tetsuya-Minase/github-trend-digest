//! Domain layer containing business logic and entities.
//!
//! This layer is the core of the application and is independent of
//! infrastructure concerns. It follows the principles of hexagonal architecture.
pub mod entities;
pub mod errors;
pub mod repositories;

// Re-export commonly used types for convenience
pub use entities::{Repository, RepositoryInsight, TrendingSummary};
pub use errors::{AppError, AppResult};
pub use repositories::{AiSummarizer, GithubApiClient, NotificationSender, TrendingRepository};
