/// Test helpers and builders for creating test data.
///
/// This module provides builder patterns for creating domain entities
/// and other test fixtures with sensible defaults.

use github_trending_summary::domain::{Repository, TrendingSummary};

/// Builder for creating Repository instances in tests.
///
/// Uses the Builder pattern to provide a fluent API for constructing
/// test repositories with custom or default values.
///
/// # Design Notes
/// - Implements Default for convenient initialization
/// - All fields have sensible defaults for testing
/// - Allows selective override of fields
///
/// # Examples
/// ```
/// use github_trending_summary::tests::common::RepositoryBuilder;
///
/// let repo = RepositoryBuilder::new()
///     .name("test-repo".to_string())
///     .stars(100)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct RepositoryBuilder {
    name: String,
    owner: String,
    url: String,
    description: Option<String>,
    stars: u32,
    language: Option<String>,
    topics: Vec<String>,
}

impl Default for RepositoryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl RepositoryBuilder {
    /// Creates a new builder with default values.
    pub fn new() -> Self {
        Self {
            name: "test-repo".to_string(),
            owner: "test-owner".to_string(),
            url: "https://github.com/test-owner/test-repo".to_string(),
            description: Some("Test repository description".to_string()),
            stars: 100,
            language: Some("Rust".to_string()),
            topics: vec!["test".to_string()],
        }
    }

    /// Sets the repository name.
    pub fn name(mut self, name: String) -> Self {
        self.name = name;
        self
    }

    /// Sets the repository owner.
    pub fn owner(mut self, owner: String) -> Self {
        self.owner = owner;
        self
    }

    /// Sets the repository URL.
    pub fn url(mut self, url: String) -> Self {
        self.url = url;
        self
    }

    /// Sets the repository description.
    pub fn description(mut self, description: Option<String>) -> Self {
        self.description = description;
        self
    }

    /// Sets the star count.
    pub fn stars(mut self, stars: u32) -> Self {
        self.stars = stars;
        self
    }

    /// Sets the primary language.
    pub fn language(mut self, language: Option<String>) -> Self {
        self.language = language;
        self
    }

    /// Sets the topics.
    pub fn topics(mut self, topics: Vec<String>) -> Self {
        self.topics = topics;
        self
    }

    /// Builds the Repository instance.
    ///
    /// # Panics
    /// Panics if the repository cannot be created (e.g., validation fails).
    /// This is acceptable for tests as it makes failures explicit.
    pub fn build(self) -> Repository {
        Repository::new(
            self.name,
            self.owner,
            self.url,
            self.description,
            self.stars,
            self.language,
            self.topics,
        )
        .expect("Failed to build test repository")
    }
}

/// Builder for creating TrendingSummary instances in tests.
///
/// Provides a fluent API for constructing test summaries with custom or default values.
///
/// # Examples
/// ```
/// use github_trending_summary::tests::common::{RepositoryBuilder, TrendingSummaryBuilder};
///
/// let repo = RepositoryBuilder::new().build();
/// let summary = TrendingSummaryBuilder::new()
///     .repository(repo)
///     .ai_summary("Test summary".to_string())
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct TrendingSummaryBuilder {
    repository: Option<Repository>,
    ai_summary: String,
    use_cases: Vec<String>,
    target_languages: Vec<String>,
}

impl Default for TrendingSummaryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TrendingSummaryBuilder {
    /// Creates a new builder with default values.
    pub fn new() -> Self {
        Self {
            repository: None,
            ai_summary: "Test AI-generated summary".to_string(),
            use_cases: vec!["Test use case".to_string()],
            target_languages: vec!["Rust".to_string()],
        }
    }

    /// Sets the repository.
    pub fn repository(mut self, repository: Repository) -> Self {
        self.repository = Some(repository);
        self
    }

    /// Sets the AI summary.
    pub fn ai_summary(mut self, ai_summary: String) -> Self {
        self.ai_summary = ai_summary;
        self
    }

    /// Sets the use cases.
    pub fn use_cases(mut self, use_cases: Vec<String>) -> Self {
        self.use_cases = use_cases;
        self
    }

    /// Sets the target languages.
    pub fn target_languages(mut self, target_languages: Vec<String>) -> Self {
        self.target_languages = target_languages;
        self
    }

    /// Builds the TrendingSummary instance.
    ///
    /// # Panics
    /// Panics if the summary cannot be created or if no repository was set.
    pub fn build(self) -> TrendingSummary {
        let repository = self
            .repository
            .unwrap_or_else(|| RepositoryBuilder::new().build());

        TrendingSummary::new(
            repository,
            self.ai_summary,
            self.use_cases,
            self.target_languages,
        )
        .expect("Failed to build test trending summary")
    }
}

/// Creates a test repository with Rust-specific defaults.
pub fn create_rust_repository() -> Repository {
    RepositoryBuilder::new()
        .name("rust".to_string())
        .owner("rust-lang".to_string())
        .url("https://github.com/rust-lang/rust".to_string())
        .description(Some("A language empowering everyone to build reliable and efficient software.".to_string()))
        .stars(80000)
        .language(Some("Rust".to_string()))
        .topics(vec!["compiler".to_string(), "rust".to_string()])
        .build()
}

/// Creates a test repository with Python-specific defaults.
pub fn create_python_repository() -> Repository {
    RepositoryBuilder::new()
        .name("cpython".to_string())
        .owner("python".to_string())
        .url("https://github.com/python/cpython".to_string())
        .description(Some("The Python programming language".to_string()))
        .stars(50000)
        .language(Some("Python".to_string()))
        .topics(vec!["python".to_string(), "cpython".to_string()])
        .build()
}

/// Creates a test repository with JavaScript-specific defaults.
pub fn create_javascript_repository() -> Repository {
    RepositoryBuilder::new()
        .name("node".to_string())
        .owner("nodejs".to_string())
        .url("https://github.com/nodejs/node".to_string())
        .description(Some("Node.js JavaScript runtime".to_string()))
        .stars(95000)
        .language(Some("JavaScript".to_string()))
        .topics(vec!["javascript".to_string(), "nodejs".to_string()])
        .build()
}

/// Creates a test trending summary with default values.
pub fn create_test_summary() -> TrendingSummary {
    TrendingSummaryBuilder::new().build()
}

/// Creates multiple test repositories for batch testing.
///
/// # Arguments
/// - `count`: Number of repositories to create
///
/// # Returns
/// Vector of repositories with unique names
pub fn create_test_repositories(count: usize) -> Vec<Repository> {
    (0..count)
        .map(|i| {
            RepositoryBuilder::new()
                .name(format!("test-repo-{}", i))
                .owner(format!("test-owner-{}", i))
                .url(format!("https://github.com/test-owner-{}/test-repo-{}", i, i))
                .stars(100 * (i as u32 + 1))
                .build()
        })
        .collect()
}

/// Creates multiple test summaries for batch testing.
///
/// # Arguments
/// - `count`: Number of summaries to create
///
/// # Returns
/// Vector of trending summaries with unique repositories
pub fn create_test_summaries(count: usize) -> Vec<TrendingSummary> {
    create_test_repositories(count)
        .into_iter()
        .map(|repo| TrendingSummaryBuilder::new().repository(repo).build())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repository_builder_default() {
        let repo = RepositoryBuilder::new().build();
        assert_eq!(repo.name(), "test-repo");
        assert_eq!(repo.owner(), "test-owner");
        assert_eq!(repo.stars(), 100);
    }

    #[test]
    fn test_repository_builder_custom() {
        let repo = RepositoryBuilder::new()
            .name("custom-repo".to_string())
            .owner("custom-owner".to_string())
            .stars(500)
            .build();

        assert_eq!(repo.name(), "custom-repo");
        assert_eq!(repo.owner(), "custom-owner");
        assert_eq!(repo.stars(), 500);
    }

    #[test]
    fn test_trending_summary_builder_default() {
        let summary = TrendingSummaryBuilder::new().build();
        assert_eq!(summary.ai_summary(), "Test AI-generated summary");
        assert_eq!(summary.use_cases().len(), 1);
        assert_eq!(summary.target_languages().len(), 1);
    }

    #[test]
    fn test_trending_summary_builder_custom() {
        let repo = RepositoryBuilder::new().build();
        let summary = TrendingSummaryBuilder::new()
            .repository(repo.clone())
            .ai_summary("Custom summary".to_string())
            .use_cases(vec!["Use case 1".to_string(), "Use case 2".to_string()])
            .target_languages(vec!["Rust".to_string(), "Python".to_string()])
            .build();

        assert_eq!(summary.ai_summary(), "Custom summary");
        assert_eq!(summary.use_cases().len(), 2);
        assert_eq!(summary.target_languages().len(), 2);
        assert_eq!(summary.repository(), &repo);
    }

    #[test]
    fn test_create_rust_repository() {
        let repo = create_rust_repository();
        assert_eq!(repo.name(), "rust");
        assert_eq!(repo.owner(), "rust-lang");
        assert_eq!(repo.language(), Some("Rust"));
    }

    #[test]
    fn test_create_python_repository() {
        let repo = create_python_repository();
        assert_eq!(repo.name(), "cpython");
        assert_eq!(repo.owner(), "python");
        assert_eq!(repo.language(), Some("Python"));
    }

    #[test]
    fn test_create_javascript_repository() {
        let repo = create_javascript_repository();
        assert_eq!(repo.name(), "node");
        assert_eq!(repo.owner(), "nodejs");
        assert_eq!(repo.language(), Some("JavaScript"));
    }

    #[test]
    fn test_create_test_repositories() {
        let repos = create_test_repositories(5);
        assert_eq!(repos.len(), 5);

        for (i, repo) in repos.iter().enumerate() {
            assert_eq!(repo.name(), format!("test-repo-{}", i));
            assert_eq!(repo.owner(), format!("test-owner-{}", i));
            assert_eq!(repo.stars(), 100 * (i as u32 + 1));
        }
    }

    #[test]
    fn test_create_test_summaries() {
        let summaries = create_test_summaries(3);
        assert_eq!(summaries.len(), 3);

        for summary in &summaries {
            assert!(!summary.ai_summary().is_empty());
            assert!(!summary.use_cases().is_empty());
            assert!(!summary.target_languages().is_empty());
        }
    }
}
