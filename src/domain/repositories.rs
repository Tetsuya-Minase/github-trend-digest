/// Repository trait definitions for the domain layer.
///
/// These traits define the contracts that infrastructure implementations must satisfy.
/// They follow the Dependency Inversion Principle (DIP) from SOLID,
/// allowing the domain layer to remain independent of infrastructure details.
use async_trait::async_trait;

use super::entities::{Repository, TrendingSummary};
use super::errors::AppResult;

/// Trait for fetching trending repositories.
///
/// This trait abstracts the mechanism for retrieving trending repositories,
/// allowing different implementations (e.g., web scraping, API calls).
///
/// # Design Notes
/// - Uses async_trait for async method support in traits
/// - Returns AppResult for consistent error handling
/// - Language filter is optional to support both language-specific and global trending
#[async_trait]
pub trait TrendingRepository: Send + Sync {
    /// Fetches the list of trending repositories.
    ///
    /// # Arguments
    /// - `language`: Optional language filter (e.g., "rust", "python")
    ///
    /// # Returns
    /// - `Ok(Vec<Repository>)`: List of trending repositories
    /// - `Err(AppError)`: Error if fetching fails
    ///
    /// # Examples
    /// ```ignore
    /// let repos = trending_repo.fetch_trending(Some("rust".to_string())).await?;
    /// ```
    async fn fetch_trending(&self, language: Option<String>) -> AppResult<Vec<Repository>>;
}

/// Trait for interacting with the GitHub API.
///
/// This trait provides methods for retrieving detailed information about repositories
/// from the GitHub API.
///
/// # Design Notes
/// - Separates concerns from TrendingRepository (SRP from SOLID)
/// - Allows mocking for testing without actual API calls
#[async_trait]
pub trait GithubApiClient: Send + Sync {
    /// Retrieves detailed information about a specific repository.
    ///
    /// # Arguments
    /// - `owner`: Repository owner username or organization name
    /// - `name`: Repository name
    ///
    /// # Returns
    /// - `Ok(Repository)`: Repository details
    /// - `Err(AppError)`: Error if the API call fails or repository not found
    ///
    /// # Examples
    /// ```ignore
    /// let repo = github_client.get_repository_details("rust-lang", "rust").await?;
    /// ```
    async fn get_repository_details(&self, owner: &str, name: &str) -> AppResult<Repository>;

    /// Checks if the API client has valid credentials.
    ///
    /// # Returns
    /// - `Ok(())`: Credentials are valid
    /// - `Err(AppError)`: Invalid or missing credentials
    async fn verify_credentials(&self) -> AppResult<()>;
}

/// Trait for AI-powered summarization of repositories.
///
/// This trait abstracts the AI service used for generating summaries,
/// allowing different implementations (e.g., Gemini, GPT, Claude).
///
/// # Design Notes
/// - Interface Segregation Principle (ISP): Focused on a single responsibility
/// - Allows easy switching between different AI providers
#[async_trait]
pub trait AiSummarizer: Send + Sync {
    /// Generates a summary for the given repository.
    ///
    /// # Arguments
    /// - `repository`: Repository to summarize
    ///
    /// # Returns
    /// - `Ok(String)`: AI-generated summary
    /// - `Err(AppError)`: Error if summarization fails
    ///
    /// # Examples
    /// ```ignore
    /// let summary = ai_summarizer.summarize(&repository).await?;
    /// ```
    async fn summarize(&self, repository: &Repository) -> AppResult<String>;

    /// Extracts potential use cases from the repository.
    ///
    /// # Arguments
    /// - `repository`: Repository to analyze
    ///
    /// # Returns
    /// - `Ok(Vec<String>)`: List of identified use cases
    /// - `Err(AppError)`: Error if analysis fails
    async fn extract_use_cases(&self, repository: &Repository) -> AppResult<Vec<String>>;

    /// Identifies target programming languages for the repository.
    ///
    /// # Arguments
    /// - `repository`: Repository to analyze
    ///
    /// # Returns
    /// - `Ok(Vec<String>)`: List of target languages
    /// - `Err(AppError)`: Error if analysis fails
    async fn identify_target_languages(&self, repository: &Repository) -> AppResult<Vec<String>>;
}

/// Trait for sending notifications about trending repositories.
///
/// This trait abstracts the notification mechanism,
/// allowing different implementations (e.g., Discord, Slack, Email).
///
/// # Design Notes
/// - Open/Closed Principle (OCP): Open for extension with new notification channels
/// - Single responsibility: Only handles notification sending
#[async_trait]
pub trait NotificationSender: Send + Sync {
    /// Sends a notification with the trending summary.
    ///
    /// # Arguments
    /// - `summary`: Trending summary to send
    ///
    /// # Returns
    /// - `Ok(())`: Notification sent successfully
    /// - `Err(AppError)`: Error if sending fails
    ///
    /// # Examples
    /// ```ignore
    /// notification_sender.send(&summary).await?;
    /// ```
    async fn send(&self, summary: &TrendingSummary) -> AppResult<()>;

    /// Sends multiple notifications in batch.
    ///
    /// # Arguments
    /// - `summaries`: List of summaries to send
    ///
    /// # Returns
    /// - `Ok(())`: All notifications sent successfully
    /// - `Err(AppError)`: Error if any notification fails
    ///
    /// # Notes
    /// Default implementation sends notifications sequentially.
    /// Implementers can override for parallel sending if supported.
    async fn send_batch(&self, summaries: &[TrendingSummary]) -> AppResult<()> {
        for summary in summaries {
            self.send(summary).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;

    // Mock implementations for testing

    mock! {
        pub TrendingRepo {}

        #[async_trait]
        impl TrendingRepository for TrendingRepo {
            async fn fetch_trending(&self, language: Option<String>) -> AppResult<Vec<Repository>>;
        }
    }

    mock! {
        pub GithubClient {}

        #[async_trait]
        impl GithubApiClient for GithubClient {
            async fn get_repository_details(&self, owner: &str, name: &str) -> AppResult<Repository>;
            async fn verify_credentials(&self) -> AppResult<()>;
        }
    }

    mock! {
        pub AiSummarizerImpl {}

        #[async_trait]
        impl AiSummarizer for AiSummarizerImpl {
            async fn summarize(&self, repository: &Repository) -> AppResult<String>;
            async fn extract_use_cases(&self, repository: &Repository) -> AppResult<Vec<String>>;
            async fn identify_target_languages(&self, repository: &Repository) -> AppResult<Vec<String>>;
        }
    }

    mock! {
        pub NotificationSenderImpl {}

        #[async_trait]
        impl NotificationSender for NotificationSenderImpl {
            async fn send(&self, summary: &TrendingSummary) -> AppResult<()>;
            async fn send_batch(&self, summaries: &[TrendingSummary]) -> AppResult<()>;
        }
    }

    #[tokio::test]
    async fn test_mock_trending_repository() {
        let mut mock = MockTrendingRepo::new();
        mock.expect_fetch_trending()
            .with(mockall::predicate::eq(Some("rust".to_string())))
            .times(1)
            .returning(|_| Ok(vec![]));

        let result = mock.fetch_trending(Some("rust".to_string())).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mock_github_api_client() {
        let mut mock = MockGithubClient::new();
        mock.expect_verify_credentials()
            .times(1)
            .returning(|| Ok(()));

        let result = mock.verify_credentials().await;
        assert!(result.is_ok());
    }
}
