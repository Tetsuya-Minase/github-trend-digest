/// Use cases (application services) for the application layer.
///
/// These use cases orchestrate domain objects and repository operations
/// to implement the application's business logic. They follow the principles
/// of Clean Architecture and hexagonal architecture.
use std::sync::Arc;

use tracing::{info, warn};

use crate::domain::{
    AiSummarizer, AppResult, GithubApiClient, NotificationSender, Repository, TrendingRepository,
    TrendingSummary,
};

/// Use case for fetching and summarizing trending repositories.
///
/// This use case orchestrates the following operations:
/// 1. Fetch trending repositories from GitHub
/// 2. Enrich repository details via GitHub API (if needed)
/// 3. Generate AI summaries for each repository
/// 4. Send notifications with the summaries
///
/// # Design Notes
/// - Uses Arc for shared ownership of dependencies (thread-safe)
/// - Dependencies are injected through the constructor (Dependency Injection)
/// - Follows Single Responsibility Principle: only handles this specific workflow
///
/// # Time Complexity
/// O(n) where n is the number of trending repositories
/// - Fetching: O(1) API call
/// - Processing: O(n) for n repositories
/// - Each repository requires 3-4 AI API calls: O(1) per call
/// - Total: O(n) with potential for parallelization
///
/// # Space Complexity
/// O(n) for storing n repositories and their summaries in memory
pub struct FetchAndSummarizeTrendingUseCase {
    trending_repository: Arc<dyn TrendingRepository>,
    // TODO: Use this field in future iterations. Track usage in issue #<issue-number> or remove if not needed.
    #[allow(dead_code)]
    github_api_client: Arc<dyn GithubApiClient>,
    ai_summarizer: Arc<dyn AiSummarizer>,
    notification_sender: Arc<dyn NotificationSender>,
}

impl FetchAndSummarizeTrendingUseCase {
    /// Creates a new instance of the use case with injected dependencies.
    ///
    /// # Arguments
    /// - `trending_repository`: Service for fetching trending repositories
    /// - `github_api_client`: Client for GitHub API operations
    /// - `ai_summarizer`: Service for generating AI summaries
    /// - `notification_sender`: Service for sending notifications
    ///
    /// # Examples
    /// ```ignore
    /// let use_case = FetchAndSummarizeTrendingUseCase::new(
    ///     Arc::new(trending_repo),
    ///     Arc::new(github_client),
    ///     Arc::new(ai_summarizer),
    ///     Arc::new(notification_sender),
    /// );
    /// ```
    pub fn new(
        trending_repository: Arc<dyn TrendingRepository>,
        github_api_client: Arc<dyn GithubApiClient>,
        ai_summarizer: Arc<dyn AiSummarizer>,
        notification_sender: Arc<dyn NotificationSender>,
    ) -> Self {
        Self {
            trending_repository,
            github_api_client,
            ai_summarizer,
            notification_sender,
        }
    }

    /// Executes the use case to fetch and summarize trending repositories.
    ///
    /// # Arguments
    /// - `language`: Optional language filter (e.g., "rust", "python")
    ///
    /// # Returns
    /// - `Ok(Vec<TrendingSummary>)`: List of trending summaries
    /// - `Err(AppError)`: Error if any step fails
    ///
    /// # Process Flow
    /// 1. Fetch trending repositories
    /// 2. For each repository:
    ///    a. Generate summary
    ///    b. Extract use cases
    ///    c. Identify target languages
    ///    d. Create TrendingSummary
    /// 3. Send notifications
    ///
    /// # Examples
    /// ```ignore
    /// let summaries = use_case.execute(Some("rust".to_string())).await?;
    /// ```
    pub async fn execute(&self, language: Option<String>) -> AppResult<Vec<TrendingSummary>> {
        info!("Starting fetch and summarize trending use case");
        info!("Language filter: {:?}", language);

        // Step 1: Fetch trending repositories
        info!("Fetching trending repositories");
        let repositories = self
            .trending_repository
            .fetch_trending(language.clone())
            .await?;
        info!("Fetched {} repositories", repositories.len());

        if repositories.is_empty() {
            warn!("No trending repositories found");
            return Ok(vec![]);
        }

        // Step 2: Generate summaries for each repository
        info!("Generating summaries for repositories");
        let mut summaries = Vec::with_capacity(repositories.len());

        for repository in repositories {
            info!("Processing repository: {}", repository.full_name());

            match self.create_summary(&repository).await {
                Ok(summary) => {
                    info!(
                        "Successfully created summary for {}",
                        repository.full_name()
                    );
                    summaries.push(summary);
                }
                Err(e) => {
                    warn!(
                        "Failed to create summary for {}: {}",
                        repository.full_name(),
                        e
                    );
                    // Continue processing other repositories even if one fails
                    continue;
                }
            }
        }

        info!("Generated {} summaries", summaries.len());

        // Step 3: Send notifications
        if !summaries.is_empty() {
            info!("Sending notifications");
            self.notification_sender.send_batch(&summaries).await?;
            info!("Notifications sent successfully");
        }

        Ok(summaries)
    }

    /// Creates a summary for a single repository.
    ///
    /// This helper method orchestrates AI operations for a single repository.
    ///
    /// # Arguments
    /// - `repository`: Repository to summarize
    ///
    /// # Returns
    /// - `Ok(TrendingSummary)`: Created summary
    /// - `Err(AppError)`: Error if summarization fails
    async fn create_summary(&self, repository: &Repository) -> AppResult<TrendingSummary> {
        // Generate AI summary
        let ai_summary = self.ai_summarizer.summarize(repository).await?;

        // Extract use cases
        let use_cases = self.ai_summarizer.extract_use_cases(repository).await?;

        // Identify target languages
        let target_languages = self
            .ai_summarizer
            .identify_target_languages(repository)
            .await?;

        // Create TrendingSummary
        TrendingSummary::new(repository.clone(), ai_summary, use_cases, target_languages)
            .map_err(crate::domain::AppError::ValidationError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;
    use mockall::predicate::*;

    // Import mock trait implementations
    use crate::domain::repositories::*;

    mock! {
        TrendingRepo {}

        #[async_trait::async_trait]
        impl TrendingRepository for TrendingRepo {
            async fn fetch_trending(&self, language: Option<String>) -> AppResult<Vec<Repository>>;
        }
    }

    mock! {
        GithubClient {}

        #[async_trait::async_trait]
        impl GithubApiClient for GithubClient {
            async fn get_repository_details(&self, owner: &str, name: &str) -> AppResult<Repository>;
            async fn verify_credentials(&self) -> AppResult<()>;
        }
    }

    mock! {
        AiSummarizerImpl {}

        #[async_trait::async_trait]
        impl AiSummarizer for AiSummarizerImpl {
            async fn summarize(&self, repository: &Repository) -> AppResult<String>;
            async fn extract_use_cases(&self, repository: &Repository) -> AppResult<Vec<String>>;
            async fn identify_target_languages(&self, repository: &Repository) -> AppResult<Vec<String>>;
        }
    }

    mock! {
        NotificationSenderImpl {}

        #[async_trait::async_trait]
        impl NotificationSender for NotificationSenderImpl {
            async fn send(&self, summary: &TrendingSummary) -> AppResult<()>;
            async fn send_batch(&self, summaries: &[TrendingSummary]) -> AppResult<()>;
        }
    }

    fn create_test_repository() -> Repository {
        Repository::new(
            "rust".to_string(),
            "rust-lang".to_string(),
            "https://github.com/rust-lang/rust".to_string(),
            Some("A language empowering everyone".to_string()),
            80000,
            Some("Rust".to_string()),
            vec!["compiler".to_string()],
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_execute_success() {
        let _test_repo = create_test_repository();

        // Setup mocks
        let mut trending_repo = MockTrendingRepo::new();
        trending_repo
            .expect_fetch_trending()
            .with(eq(Some("rust".to_string())))
            .times(1)
            .returning(move |_| Ok(vec![create_test_repository()]));

        let github_client = MockGithubClient::new();

        let mut ai_summarizer = MockAiSummarizerImpl::new();
        ai_summarizer
            .expect_summarize()
            .times(1)
            .returning(|_| Ok("A systems programming language".to_string()));
        ai_summarizer
            .expect_extract_use_cases()
            .times(1)
            .returning(|_| Ok(vec!["System programming".to_string()]));
        ai_summarizer
            .expect_identify_target_languages()
            .times(1)
            .returning(|_| Ok(vec!["Rust".to_string()]));

        let mut notification_sender = MockNotificationSenderImpl::new();
        notification_sender
            .expect_send_batch()
            .times(1)
            .returning(|_| Ok(()));

        // Create use case
        let use_case = FetchAndSummarizeTrendingUseCase::new(
            Arc::new(trending_repo),
            Arc::new(github_client),
            Arc::new(ai_summarizer),
            Arc::new(notification_sender),
        );

        // Execute
        let result = use_case.execute(Some("rust".to_string())).await;

        // Assert
        assert!(result.is_ok());
        let summaries = result.unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].ai_summary(), "A systems programming language");
    }

    #[tokio::test]
    async fn test_execute_no_repositories() {
        // Setup mocks
        let mut trending_repo = MockTrendingRepo::new();
        trending_repo
            .expect_fetch_trending()
            .times(1)
            .returning(|_| Ok(vec![]));

        let github_client = MockGithubClient::new();
        let ai_summarizer = MockAiSummarizerImpl::new();
        let notification_sender = MockNotificationSenderImpl::new();

        // Create use case
        let use_case = FetchAndSummarizeTrendingUseCase::new(
            Arc::new(trending_repo),
            Arc::new(github_client),
            Arc::new(ai_summarizer),
            Arc::new(notification_sender),
        );

        // Execute
        let result = use_case.execute(None).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_execute_fetch_error() {
        use crate::domain::AppError;

        // Setup mocks
        let mut trending_repo = MockTrendingRepo::new();
        trending_repo
            .expect_fetch_trending()
            .times(1)
            .returning(|_| Err(AppError::ScrapingError("Failed to fetch".to_string())));

        let github_client = MockGithubClient::new();
        let ai_summarizer = MockAiSummarizerImpl::new();
        let notification_sender = MockNotificationSenderImpl::new();

        // Create use case
        let use_case = FetchAndSummarizeTrendingUseCase::new(
            Arc::new(trending_repo),
            Arc::new(github_client),
            Arc::new(ai_summarizer),
            Arc::new(notification_sender),
        );

        // Execute
        let result = use_case.execute(None).await;

        // Assert
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::ScrapingError(_) => {}
            _ => panic!("Expected ScrapingError"),
        }
    }
}
