//! Use cases (application services) for the application layer.
//!
//! These use cases orchestrate domain objects and repository operations
//! to implement the application's business logic. They follow the principles
//! of Clean Architecture and hexagonal architecture.
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
    github_api_client: Arc<dyn GithubApiClient>,
    ai_summarizer: Arc<dyn AiSummarizer>,
    notification_sender: Arc<dyn NotificationSender>,
    pub max_concurrency: usize,
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
            max_concurrency: 3,
        }
    }

    pub fn with_max_concurrency(mut self, max_concurrency: usize) -> Self {
        self.max_concurrency = if max_concurrency == 0 {
            1
        } else {
            max_concurrency
        };
        self
    }

    async fn enrich_repository(&self, repository: &Repository) -> Repository {
        match self
            .github_api_client
            .get_repository_details(repository.owner(), repository.name())
            .await
        {
            Ok(enriched) => enriched,
            Err(e) => {
                warn!(
                    "Failed to enrich repository {}/{}: {}",
                    repository.owner(),
                    repository.name(),
                    e
                );
                repository.clone()
            }
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

        use futures::stream::{self, StreamExt};

        let summaries: Vec<_> = stream::iter(repositories)
            .map(|repository| async move {
                info!("Processing repository: {}", repository.full_name());
                let enriched_repository = self.enrich_repository(&repository).await;
                match self.create_summary(&enriched_repository).await {
                    Ok(summary) => {
                        info!(
                            "Successfully created summary for {}",
                            repository.full_name()
                        );
                        Some(summary)
                    }
                    Err(e) => {
                        warn!(
                            "Failed to create summary for {}: {}",
                            repository.full_name(),
                            e
                        );
                        None
                    }
                }
            })
            .buffered(self.max_concurrency)
            .filter_map(|x| async { x })
            .collect()
            .await;

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

        let mut github_client = MockGithubClient::new();
        github_client
            .expect_get_repository_details()
            .with(eq("rust-lang"), eq("rust"))
            .times(1)
            .returning(|_, _| Ok(create_test_repository()));

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
    async fn test_execute_enrich_success_t1() {
        let repo = Repository::new(
            "rust".to_string(),
            "rust-lang".to_string(),
            "https://github.com/rust-lang/rust".to_string(),
            None,
            100,
            None,
            vec![], // no topics initially
        )
        .unwrap();

        let enriched_repo = Repository::new(
            "rust".to_string(),
            "rust-lang".to_string(),
            "https://github.com/rust-lang/rust".to_string(),
            None,
            100,
            None,
            vec!["systems".to_string(), "compiler".to_string()],
        )
        .unwrap();

        let mut trending_repo = MockTrendingRepo::new();
        trending_repo
            .expect_fetch_trending()
            .times(1)
            .returning(move |_| Ok(vec![repo.clone()]));

        let mut github_client = MockGithubClient::new();
        github_client
            .expect_get_repository_details()
            .with(eq("rust-lang"), eq("rust"))
            .times(1)
            .returning(move |_, _| Ok(enriched_repo.clone()));

        let mut ai_summarizer = MockAiSummarizerImpl::new();
        ai_summarizer
            .expect_summarize()
            .times(1)
            .returning(|_| Ok("Summary".to_string()));
        ai_summarizer
            .expect_extract_use_cases()
            .times(1)
            .returning(|_| Ok(vec![]));
        ai_summarizer
            .expect_identify_target_languages()
            .times(1)
            .returning(|_| Ok(vec![]));

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

        let result = use_case.execute(None).await;
        assert!(result.is_ok());
        let summaries = result.unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(
            summaries[0].repository().topics(),
            vec!["systems".to_string(), "compiler".to_string()]
        );
    }

    #[tokio::test]
    async fn test_execute_enrich_fallback_t2() {
        let repo = Repository::new(
            "rust".to_string(),
            "rust-lang".to_string(),
            "https://github.com/rust-lang/rust".to_string(),
            None,
            100,
            None,
            vec![], // no topics initially
        )
        .unwrap();

        let mut trending_repo = MockTrendingRepo::new();
        trending_repo
            .expect_fetch_trending()
            .times(1)
            .returning(move |_| Ok(vec![repo.clone()]));

        let mut github_client = MockGithubClient::new();
        github_client
            .expect_get_repository_details()
            .with(eq("rust-lang"), eq("rust"))
            .times(1)
            .returning(|_, _| {
                Err(crate::domain::AppError::GithubApiError(
                    "API Failed".to_string(),
                ))
            });

        let mut ai_summarizer = MockAiSummarizerImpl::new();
        ai_summarizer
            .expect_summarize()
            .times(1)
            .returning(|_| Ok("Summary".to_string()));
        ai_summarizer
            .expect_extract_use_cases()
            .times(1)
            .returning(|_| Ok(vec![]));
        ai_summarizer
            .expect_identify_target_languages()
            .times(1)
            .returning(|_| Ok(vec![]));

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

        let result = use_case.execute(None).await;
        assert!(result.is_ok());
        let summaries = result.unwrap();
        assert_eq!(summaries.len(), 1);
        assert!(summaries[0].repository().topics().is_empty());
    }

    #[tokio::test]
    async fn test_execute_partial_success_t4() {
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
            Repository::new(
                "repo3".to_string(),
                "owner".to_string(),
                "http://example.com".to_string(),
                None,
                1,
                None,
                vec![],
            )
            .unwrap(),
            Repository::new(
                "repo4".to_string(),
                "owner".to_string(),
                "http://example.com".to_string(),
                None,
                1,
                None,
                vec![],
            )
            .unwrap(),
            Repository::new(
                "repo5".to_string(),
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
            .returning(|_, _| {
                Err(crate::domain::AppError::GithubApiError(
                    "API Failed".to_string(),
                ))
            });

        let mut ai_summarizer = MockAiSummarizerImpl::new();
        ai_summarizer.expect_summarize().returning(|repo| {
            if repo.name() == "repo3" {
                Err(crate::domain::AppError::AiServiceError(
                    "AI failed".to_string(),
                ))
            } else {
                Ok("Summary".to_string())
            }
        });
        ai_summarizer
            .expect_extract_use_cases()
            .returning(|_| Ok(vec![]));
        ai_summarizer
            .expect_identify_target_languages()
            .returning(|_| Ok(vec![]));

        let mut notification_sender = MockNotificationSenderImpl::new();
        notification_sender
            .expect_send_batch()
            .times(1)
            .returning(|summaries| {
                assert_eq!(summaries.len(), 4);
                Ok(())
            });

        let use_case = FetchAndSummarizeTrendingUseCase::new(
            Arc::new(trending_repo),
            Arc::new(github_client),
            Arc::new(ai_summarizer),
            Arc::new(notification_sender),
        )
        .with_max_concurrency(2);

        let result = use_case.execute(None).await;
        assert!(result.is_ok());
        let summaries = result.unwrap();
        assert_eq!(summaries.len(), 4);
    }

    #[tokio::test]
    async fn test_execute_order_preserved_t5() {
        let mut trending_repo = MockTrendingRepo::new();
        let repos = vec![
            Repository::new(
                "repo-a".to_string(),
                "owner".to_string(),
                "http://example.com".to_string(),
                None,
                1,
                None,
                vec![],
            )
            .unwrap(),
            Repository::new(
                "repo-b".to_string(),
                "owner".to_string(),
                "http://example.com".to_string(),
                None,
                1,
                None,
                vec![],
            )
            .unwrap(),
            Repository::new(
                "repo-c".to_string(),
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
            .returning(|_| Ok("Summary".to_string()));
        ai_summarizer
            .expect_extract_use_cases()
            .returning(|_| Ok(vec![]));
        ai_summarizer
            .expect_identify_target_languages()
            .returning(|_| Ok(vec![]));

        let mut notification_sender = MockNotificationSenderImpl::new();
        notification_sender
            .expect_send_batch()
            .times(1)
            .returning(|summaries| {
                assert_eq!(summaries.len(), 3);
                Ok(())
            });

        let use_case = FetchAndSummarizeTrendingUseCase::new(
            Arc::new(trending_repo),
            Arc::new(github_client),
            Arc::new(ai_summarizer),
            Arc::new(notification_sender),
        )
        .with_max_concurrency(3);

        let result = use_case.execute(None).await;
        assert!(result.is_ok());
        let summaries = result.unwrap();
        assert_eq!(summaries.len(), 3);
        assert_eq!(summaries[0].repository().name(), "repo-a");
        assert_eq!(summaries[1].repository().name(), "repo-b");
        assert_eq!(summaries[2].repository().name(), "repo-c");
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

    struct DelayedAiSummarizer {
        active_tasks: Arc<std::sync::atomic::AtomicUsize>,
        max_observed: Arc<std::sync::atomic::AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl AiSummarizer for DelayedAiSummarizer {
        async fn summarize(&self, _repository: &Repository) -> AppResult<String> {
            let current = self
                .active_tasks
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                + 1;
            self.max_observed
                .fetch_max(current, std::sync::atomic::Ordering::SeqCst);
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            self.active_tasks
                .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            Ok("Summary".to_string())
        }

        async fn extract_use_cases(&self, _repository: &Repository) -> AppResult<Vec<String>> {
            Ok(vec![])
        }

        async fn identify_target_languages(
            &self,
            _repository: &Repository,
        ) -> AppResult<Vec<String>> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn test_execute_concurrency_limit_t3() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let mut trending_repo = MockTrendingRepo::new();
        let repos = (0..5)
            .map(|i| {
                Repository::new(
                    format!("repo{}", i),
                    "owner".to_string(),
                    "http://example.com".to_string(),
                    None,
                    1,
                    None,
                    vec![],
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let expected_repos = repos.clone();
        trending_repo
            .expect_fetch_trending()
            .times(1)
            .returning(move |_| Ok(expected_repos.clone()));

        let mut github_client = MockGithubClient::new();
        github_client
            .expect_get_repository_details()
            .returning(|_, _| Err(crate::domain::AppError::GithubApiError("API".to_string())));

        let active_tasks = Arc::new(AtomicUsize::new(0));
        let max_observed = Arc::new(AtomicUsize::new(0));

        let ai_summarizer = DelayedAiSummarizer {
            active_tasks: active_tasks.clone(),
            max_observed: max_observed.clone(),
        };

        let mut notification_sender = MockNotificationSenderImpl::new();
        notification_sender
            .expect_send_batch()
            .times(1)
            .returning(|summaries| {
                assert_eq!(summaries.len(), 5);
                Ok(())
            });

        let use_case = FetchAndSummarizeTrendingUseCase::new(
            Arc::new(trending_repo),
            Arc::new(github_client),
            Arc::new(ai_summarizer),
            Arc::new(notification_sender),
        )
        .with_max_concurrency(2);

        let result = use_case.execute(None).await;
        assert!(result.is_ok());

        let observed = max_observed.load(Ordering::SeqCst);
        assert!(
            observed > 0 && observed <= 2,
            "Observed concurrency: {}",
            observed
        );
    }
}
