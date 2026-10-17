//! Use cases (application services) for the application layer.
//!
//! These use cases orchestrate domain objects and repository operations
//! to implement the application's business logic. They follow the principles
//! of Clean Architecture and hexagonal architecture.
use std::sync::Arc;

use futures::stream::{self, StreamExt};
use tracing::{info, warn};

use crate::domain::{
    AiSummarizer, AppResult, GithubApiClient, NotificationSender, Repository, TrendingRepository,
    TrendingSummary,
};

/// Use case for fetching and summarizing trending repositories.
///
/// This use case orchestrates the following operations:
/// 1. Fetch trending repositories from GitHub
/// 2. Enrich repository details via GitHub API
/// 3. Generate AI summaries for each repository
/// 4. Send notifications with the summaries
///
/// # Design Notes
/// - Uses Arc for shared ownership of dependencies (thread-safe)
/// - Dependencies are injected through the constructor (Dependency Injection)
/// - Follows Single Responsibility Principle: only handles this specific workflow
/// - Enrichment and summarization run concurrently (up to `max_concurrency`
///   repositories at a time) while preserving the trending order
///
/// # Time Complexity
/// O(n) where n is the number of trending repositories
/// - Fetching: O(1) API call
/// - Processing: O(n) for n repositories, up to `max_concurrency` in parallel
/// - Each repository requires 1 GitHub API call and 3 AI API calls: O(1) per call
///
/// # Space Complexity
/// O(n) for storing n repositories and their summaries in memory
pub struct FetchAndSummarizeTrendingUseCase {
    trending_repository: Arc<dyn TrendingRepository>,
    github_api_client: Arc<dyn GithubApiClient>,
    ai_summarizer: Arc<dyn AiSummarizer>,
    notification_sender: Arc<dyn NotificationSender>,
    max_concurrency: usize,
}

/// Default number of repositories processed concurrently.
const DEFAULT_MAX_CONCURRENCY: usize = 3;

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
            max_concurrency: DEFAULT_MAX_CONCURRENCY,
        }
    }

    /// Sets the maximum number of repositories processed concurrently.
    ///
    /// A value of `0` is clamped to `1`.
    ///
    /// # Arguments
    /// - `n`: Desired concurrency limit
    ///
    /// # Examples
    /// ```ignore
    /// let use_case = FetchAndSummarizeTrendingUseCase::new(/* ... */)
    ///     .with_max_concurrency(5);
    /// ```
    pub fn with_max_concurrency(mut self, n: usize) -> Self {
        self.max_concurrency = n.max(1);
        self
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
    /// 2. For each repository (concurrently, up to `max_concurrency`):
    ///    a. Enrich with GitHub API details (falls back to scraped data on failure)
    ///    b. Generate summary
    ///    c. Extract use cases
    ///    d. Identify target languages
    ///    e. Create TrendingSummary
    /// 3. Send notifications (in trending order)
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

        // Step 2: Enrich and summarize repositories concurrently.
        // `buffered` (not `buffer_unordered`) keeps results in trending order
        // while limiting concurrency to `max_concurrency`.
        info!("Generating summaries for repositories");
        let results: Vec<(String, AppResult<TrendingSummary>)> = stream::iter(repositories)
            .map(|repository| async move {
                let full_name = repository.full_name();
                info!("Processing repository: {}", full_name);

                let enriched = self.enrich_repository(repository).await;
                (full_name, self.create_summary(&enriched).await)
            })
            .buffered(self.max_concurrency)
            .collect()
            .await;

        let mut summaries = Vec::with_capacity(results.len());
        for (full_name, result) in results {
            match result {
                Ok(summary) => {
                    info!("Successfully created summary for {}", full_name);
                    summaries.push(summary);
                }
                Err(e) => {
                    // Continue with the remaining repositories even if one fails
                    warn!("Failed to create summary for {}: {}", full_name, e);
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

    /// Enriches a scraped repository with details from the GitHub API.
    ///
    /// On success, returns the repository fetched from the API (with topics,
    /// accurate star count, and description). On failure, logs a warning and
    /// falls back to the scraped repository so processing can continue
    /// (unauthenticated GitHub API calls are rate-limited to 60 req/h,
    /// so enrichment failures are expected).
    ///
    /// # Arguments
    /// - `repository`: Repository obtained from scraping
    ///
    /// # Returns
    /// The enriched repository, or the original one if enrichment fails
    async fn enrich_repository(&self, repository: Repository) -> Repository {
        match self
            .github_api_client
            .get_repository_details(repository.owner(), repository.name())
            .await
        {
            Ok(enriched) => enriched,
            Err(e) => {
                warn!(
                    "Failed to enrich repository {}: {}. Falling back to scraped data",
                    repository.full_name(),
                    e
                );
                repository
            }
        }
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

    /// Test stub that echoes back a repository built from the requested
    /// owner/name, so enrichment always succeeds.
    struct EchoGithubClient;

    #[async_trait::async_trait]
    impl GithubApiClient for EchoGithubClient {
        async fn get_repository_details(&self, owner: &str, name: &str) -> AppResult<Repository> {
            Ok(Repository::new(
                name.to_string(),
                owner.to_string(),
                format!("https://github.com/{owner}/{name}"),
                None,
                100,
                None,
                vec![],
            )
            .unwrap())
        }

        async fn verify_credentials(&self) -> AppResult<()> {
            Ok(())
        }
    }

    fn create_test_repository_named(name: &str) -> Repository {
        Repository::new(
            name.to_string(),
            "owner".to_string(),
            format!("https://github.com/owner/{name}"),
            None,
            10,
            None,
            vec![],
        )
        .unwrap()
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

        let mut github_client = MockGithubClient::new();
        github_client
            .expect_get_repository_details()
            .times(1)
            .returning(|owner, name| {
                Ok(Repository::new(
                    name.to_string(),
                    owner.to_string(),
                    format!("https://github.com/{owner}/{name}"),
                    Some("A language empowering everyone".to_string()),
                    80000,
                    Some("Rust".to_string()),
                    vec!["compiler".to_string()],
                )
                .unwrap())
            });

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
    async fn test_execute_enriches_repositories() {
        // Given: the GitHub API returns an enriched repository with topics
        let mut trending_repo = MockTrendingRepo::new();
        trending_repo
            .expect_fetch_trending()
            .times(1)
            .returning(|_| Ok(vec![create_test_repository()]));

        let mut github_client = MockGithubClient::new();
        github_client
            .expect_get_repository_details()
            .with(eq("rust-lang"), eq("rust"))
            .times(1)
            .returning(|owner, name| {
                Ok(Repository::new(
                    name.to_string(),
                    owner.to_string(),
                    format!("https://github.com/{owner}/{name}"),
                    Some("Enriched description".to_string()),
                    90000,
                    Some("Rust".to_string()),
                    vec!["compiler".to_string(), "language".to_string()],
                )
                .unwrap())
            });

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

        let use_case = FetchAndSummarizeTrendingUseCase::new(
            Arc::new(trending_repo),
            Arc::new(github_client),
            Arc::new(ai_summarizer),
            Arc::new(notification_sender),
        );

        // When
        let result = use_case.execute(Some("rust".to_string())).await;

        // Then: the summary reflects the enriched repository information
        let summaries = result.unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(
            summaries[0].repository().topics(),
            &["compiler".to_string(), "language".to_string()]
        );
        assert_eq!(summaries[0].repository().stars(), 90000);
        assert_eq!(
            summaries[0].repository().description(),
            Some("Enriched description")
        );
    }

    #[tokio::test]
    async fn test_execute_enrich_failure_falls_back_to_scraped() {
        use crate::domain::AppError;

        // Given: the GitHub API fails to return repository details
        let mut trending_repo = MockTrendingRepo::new();
        trending_repo
            .expect_fetch_trending()
            .times(1)
            .returning(|_| Ok(vec![create_test_repository()]));

        let mut github_client = MockGithubClient::new();
        github_client
            .expect_get_repository_details()
            .times(1)
            .returning(|_, _| Err(AppError::GithubApiError("Rate limit exceeded".to_string())));

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

        let use_case = FetchAndSummarizeTrendingUseCase::new(
            Arc::new(trending_repo),
            Arc::new(github_client),
            Arc::new(ai_summarizer),
            Arc::new(notification_sender),
        );

        // When
        let result = use_case.execute(Some("rust".to_string())).await;

        // Then: the summary is generated from the scraped repository data
        let summaries = result.unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].repository().stars(), 80000);
        assert_eq!(
            summaries[0].repository().topics(),
            &["compiler".to_string()]
        );
    }

    #[tokio::test]
    async fn test_execute_parallel_limits_concurrency() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        /// Test stub that tracks the maximum number of concurrent
        /// `summarize` calls observed during execution.
        struct ConcurrencyTrackingSummarizer {
            current: Arc<AtomicUsize>,
            max_observed: Arc<AtomicUsize>,
        }

        #[async_trait::async_trait]
        impl AiSummarizer for ConcurrencyTrackingSummarizer {
            async fn summarize(&self, _repository: &Repository) -> AppResult<String> {
                let current = self.current.fetch_add(1, Ordering::SeqCst) + 1;
                self.max_observed.fetch_max(current, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                self.current.fetch_sub(1, Ordering::SeqCst);
                Ok("summary".to_string())
            }

            async fn extract_use_cases(&self, _repository: &Repository) -> AppResult<Vec<String>> {
                Ok(vec!["use case".to_string()])
            }

            async fn identify_target_languages(
                &self,
                _repository: &Repository,
            ) -> AppResult<Vec<String>> {
                Ok(vec!["Rust".to_string()])
            }
        }

        // Given: 5 repositories and a summarizer that records concurrency
        let mut trending_repo = MockTrendingRepo::new();
        trending_repo
            .expect_fetch_trending()
            .times(1)
            .returning(|_| {
                Ok((1..=5)
                    .map(|i| create_test_repository_named(&format!("repo{i}")))
                    .collect())
            });

        let max_observed = Arc::new(AtomicUsize::new(0));
        let ai_summarizer = ConcurrencyTrackingSummarizer {
            current: Arc::new(AtomicUsize::new(0)),
            max_observed: Arc::clone(&max_observed),
        };

        let mut notification_sender = MockNotificationSenderImpl::new();
        notification_sender
            .expect_send_batch()
            .times(1)
            .returning(|_| Ok(()));

        let use_case = FetchAndSummarizeTrendingUseCase::new(
            Arc::new(trending_repo),
            Arc::new(EchoGithubClient),
            Arc::new(ai_summarizer),
            Arc::new(notification_sender),
        )
        .with_max_concurrency(2);

        // When
        let result = use_case.execute(None).await;

        // Then: all repositories are processed with at most 2 concurrent tasks
        assert_eq!(result.unwrap().len(), 5);
        assert!(
            max_observed.load(Ordering::SeqCst) <= 2,
            "observed concurrency {} exceeds limit 2",
            max_observed.load(Ordering::SeqCst)
        );
    }

    #[tokio::test]
    async fn test_execute_partial_success() {
        use crate::domain::AppError;

        /// Test stub that fails `summarize` only for the repository named
        /// `failing_name` and succeeds for all others.
        struct PartiallyFailingSummarizer {
            failing_name: String,
        }

        #[async_trait::async_trait]
        impl AiSummarizer for PartiallyFailingSummarizer {
            async fn summarize(&self, repository: &Repository) -> AppResult<String> {
                if repository.name() == self.failing_name {
                    Err(AppError::AiServiceError("Summarization failed".to_string()))
                } else {
                    Ok(format!("Summary of {}", repository.name()))
                }
            }

            async fn extract_use_cases(&self, _repository: &Repository) -> AppResult<Vec<String>> {
                Ok(vec!["use case".to_string()])
            }

            async fn identify_target_languages(
                &self,
                _repository: &Repository,
            ) -> AppResult<Vec<String>> {
                Ok(vec!["Rust".to_string()])
            }
        }

        // Given: 5 repositories where summarization fails for "repo3" only
        let mut trending_repo = MockTrendingRepo::new();
        trending_repo
            .expect_fetch_trending()
            .times(1)
            .returning(|_| {
                Ok((1..=5)
                    .map(|i| create_test_repository_named(&format!("repo{i}")))
                    .collect())
            });

        let ai_summarizer = PartiallyFailingSummarizer {
            failing_name: "repo3".to_string(),
        };

        let mut notification_sender = MockNotificationSenderImpl::new();
        notification_sender
            .expect_send_batch()
            .withf(|summaries: &[TrendingSummary]| {
                summaries.len() == 4
                    && summaries
                        .iter()
                        .all(|summary| summary.repository().name() != "repo3")
            })
            .times(1)
            .returning(|_| Ok(()));

        let use_case = FetchAndSummarizeTrendingUseCase::new(
            Arc::new(trending_repo),
            Arc::new(EchoGithubClient),
            Arc::new(ai_summarizer),
            Arc::new(notification_sender),
        );

        // When
        let result = use_case.execute(None).await;

        // Then: the 4 successful summaries are returned and notified
        let summaries = result.unwrap();
        assert_eq!(summaries.len(), 4);
        assert!(summaries
            .iter()
            .all(|summary| summary.repository().name() != "repo3"));
    }

    #[tokio::test]
    async fn test_execute_preserves_order() {
        /// Test stub whose `summarize` finishes faster for repositories that
        /// appear later in the input, so out-of-order collection would be
        /// detected by the order assertion below.
        struct ReverseDelaySummarizer;

        #[async_trait::async_trait]
        impl AiSummarizer for ReverseDelaySummarizer {
            async fn summarize(&self, repository: &Repository) -> AppResult<String> {
                let delay_ms = match repository.name() {
                    "a" => 60,
                    "b" => 30,
                    _ => 0,
                };
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                Ok(format!("Summary of {}", repository.name()))
            }

            async fn extract_use_cases(&self, _repository: &Repository) -> AppResult<Vec<String>> {
                Ok(vec!["use case".to_string()])
            }

            async fn identify_target_languages(
                &self,
                _repository: &Repository,
            ) -> AppResult<Vec<String>> {
                Ok(vec!["Rust".to_string()])
            }
        }

        // Given: 3 repositories "a", "b", "c" in trending order
        let mut trending_repo = MockTrendingRepo::new();
        trending_repo
            .expect_fetch_trending()
            .times(1)
            .returning(|_| {
                Ok(["a", "b", "c"]
                    .iter()
                    .map(|name| create_test_repository_named(name))
                    .collect())
            });

        let mut notification_sender = MockNotificationSenderImpl::new();
        notification_sender
            .expect_send_batch()
            .times(1)
            .returning(|_| Ok(()));

        let use_case = FetchAndSummarizeTrendingUseCase::new(
            Arc::new(trending_repo),
            Arc::new(EchoGithubClient),
            Arc::new(ReverseDelaySummarizer),
            Arc::new(notification_sender),
        )
        .with_max_concurrency(3);

        // When
        let result = use_case.execute(None).await;

        // Then: summaries keep the input (trending) order
        let summaries = result.unwrap();
        let names: Vec<&str> = summaries
            .iter()
            .map(|summary| summary.repository().name())
            .collect();
        assert_eq!(names, vec!["a", "b", "c"]);
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

    #[tokio::test]
    async fn test_with_max_concurrency_t6() {
        let trending_repo = MockTrendingRepo::new();
        let github_client = MockGithubClient::new();
        let ai_summarizer = MockAiSummarizerImpl::new();
        let notification_sender = MockNotificationSenderImpl::new();

        let use_case = FetchAndSummarizeTrendingUseCase::new(
            Arc::new(trending_repo),
            Arc::new(github_client),
            Arc::new(ai_summarizer),
            Arc::new(notification_sender),
        );

        let use_case = use_case.with_max_concurrency(0);
        assert_eq!(use_case.max_concurrency, 1);

        let use_case = use_case.with_max_concurrency(5);
        assert_eq!(use_case.max_concurrency, 5);
    }

    #[tokio::test]
    async fn test_execute_all_fail_t7() {
        let mut trending_repo = MockTrendingRepo::new();
        let repos = vec![
            Repository::new(
                "repo1".to_string(),
                "owner".to_string(),
                "http://example.com".to_string(),
                None,
                1,
                None,
                vec![],
            )
            .unwrap(),
            Repository::new(
                "repo2".to_string(),
                "owner".to_string(),
                "http://example.com".to_string(),
                None,
                1,
                None,
                vec![],
            )
            .unwrap(),
        ];
        let expected_repos = repos.clone();
        trending_repo
            .expect_fetch_trending()
            .times(1)
            .returning(move |_| Ok(expected_repos.clone()));

        let mut github_client = MockGithubClient::new();
        github_client
            .expect_get_repository_details()
            .returning(|_, _| Err(crate::domain::AppError::GithubApiError("API".to_string())));

        let mut ai_summarizer = MockAiSummarizerImpl::new();
        ai_summarizer
            .expect_summarize()
            .returning(|_| Err(crate::domain::AppError::AiServiceError("AI".to_string())));

        let mut notification_sender = MockNotificationSenderImpl::new();
        notification_sender.expect_send_batch().times(0);

        let use_case = FetchAndSummarizeTrendingUseCase::new(
            Arc::new(trending_repo),
            Arc::new(github_client),
            Arc::new(ai_summarizer),
            Arc::new(notification_sender),
        );

        let result = use_case.execute(None).await;
        assert!(result.is_ok());
        let summaries = result.unwrap();
        assert_eq!(summaries.len(), 0);
    }
}
