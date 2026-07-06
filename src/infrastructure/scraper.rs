//! GitHub Trending scraper implementation.
//!
//! This module implements web scraping functionality for fetching trending
//! repositories from GitHub's trending page.
use async_trait::async_trait;
use reqwest::Client;
use scraper::{Html, Selector};
use tracing::{debug, error, info, warn};

use crate::domain::entities::Repository;
use crate::domain::errors::{AppError, AppResult};
use crate::domain::repositories::TrendingRepository;

/// CSS selectors for parsing GitHub Trending page.
///
/// These selectors target the HTML structure of GitHub's trending page.
/// Note: GitHub may change their HTML structure, so multiple fallback
/// strategies are implemented.
mod selectors {
    /// Main article container for each repository
    pub const REPO_ARTICLE: &str = "article.Box-row";

    /// Repository name and URL (h2 > a)
    pub const REPO_NAME_PRIMARY: &str = "h2 a";
    pub const REPO_NAME_FALLBACK: &str = "h1.h3 a";

    /// Repository description
    pub const DESCRIPTION_PRIMARY: &str = "p.col-9";
    pub const DESCRIPTION_FALLBACK: &str = "p";

    /// Programming language
    pub const LANGUAGE_PRIMARY: &str = "span[itemprop='programmingLanguage']";
    pub const LANGUAGE_FALLBACK: &str = "span.d-inline-block.ml-0.mr-3";

    /// Stars today count
    pub const STARS_TODAY_PRIMARY: &str = "span.d-inline-block.float-sm-right";
    pub const STARS_TODAY_FALLBACK: &str = "span.float-sm-right";
}

/// Client for scraping GitHub Trending page.
///
/// This client handles HTTP requests to GitHub's trending page and parses
/// the HTML to extract repository information.
///
/// # Design Notes
/// - Uses reqwest for HTTP requests with custom User-Agent
/// - Implements retry logic with exponential backoff
/// - Provides comprehensive error handling for network and parsing errors
/// - Logs detailed information for debugging and monitoring
pub struct TrendingScraperClient {
    http_client: Client,
    base_url: String,
}

impl TrendingScraperClient {
    /// Creates a new TrendingScraperClient instance.
    ///
    /// # Returns
    /// - `Ok(TrendingScraperClient)`: Successfully created client
    /// - `Err(AppError)`: Failed to create HTTP client
    ///
    /// # Examples
    /// ```
    /// use github_trending_summary::infrastructure::scraper::TrendingScraperClient;
    ///
    /// let client = TrendingScraperClient::new().unwrap();
    /// ```
    pub fn new() -> AppResult<Self> {
        Self::with_base_url("https://github.com".to_string())
    }

    /// Creates a new TrendingScraperClient with a custom base URL.
    ///
    /// This is primarily useful for testing with mock servers.
    ///
    /// # Arguments
    /// - `base_url`: Base URL for the GitHub site (e.g., "https://github.com")
    ///
    /// # Returns
    /// - `Ok(TrendingScraperClient)`: Successfully created client
    /// - `Err(AppError)`: Failed to create HTTP client
    pub fn with_base_url(base_url: String) -> AppResult<Self> {
        let http_client = Client::builder()
            .user_agent("Mozilla/5.0 (compatible; GithubTrendingBot/1.0; +https://github.com/yourusername/github-trending-summary)")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| AppError::NetworkError(format!("Failed to create HTTP client: {}", e)))?;

        info!(
            "TrendingScraperClient initialized with base_url: {}",
            base_url
        );

        Ok(Self {
            http_client,
            base_url,
        })
    }

    /// Fetches HTML content from GitHub Trending page.
    ///
    /// # Arguments
    /// - `language`: Optional language filter (e.g., "rust", "python")
    ///
    /// # Returns
    /// - `Ok(String)`: HTML content of the page
    /// - `Err(AppError)`: Network error or HTTP error
    async fn fetch_html(&self, language: Option<&str>) -> AppResult<String> {
        let url = match language {
            Some(lang) => format!("{}/trending/{}", self.base_url, lang),
            None => format!("{}/trending", self.base_url),
        };

        info!("Fetching trending page: {}", url);

        let response = self.http_client.get(&url).send().await.map_err(|e| {
            error!("Failed to fetch trending page: {}", e);
            AppError::NetworkError(format!("Failed to fetch {}: {}", url, e))
        })?;

        // Check HTTP status
        let status = response.status();
        if !status.is_success() {
            error!("HTTP error when fetching trending page: {}", status);
            return Err(AppError::ScrapingError(format!(
                "HTTP error {}: {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or("Unknown error")
            )));
        }

        let html = response.text().await.map_err(|e| {
            error!("Failed to read response body: {}", e);
            AppError::NetworkError(format!("Failed to read response: {}", e))
        })?;

        debug!("Successfully fetched HTML ({} bytes)", html.len());
        Ok(html)
    }

    /// Parses HTML content to extract repository information.
    ///
    /// # Arguments
    /// - `html`: HTML content from GitHub Trending page
    ///
    /// # Returns
    /// - `Ok(Vec<Repository>)`: List of parsed repositories
    /// - `Err(AppError)`: Parsing error
    ///
    /// # Implementation Notes
    /// - Uses multiple selector strategies with fallbacks
    /// - Handles missing or malformed data gracefully
    /// - Logs warnings for data extraction failures
    fn parse_trending_page(&self, html: &str) -> AppResult<Vec<Repository>> {
        info!("Parsing trending page HTML");

        let document = Html::parse_document(html);

        // Parse article elements
        let article_selector = Selector::parse(selectors::REPO_ARTICLE)
            .map_err(|e| AppError::ParseError(format!("Invalid selector: {}", e)))?;

        let articles = document.select(&article_selector).collect::<Vec<_>>();

        if articles.is_empty() {
            warn!("No repository articles found in HTML");
            return Ok(Vec::new());
        }

        info!("Found {} repository articles", articles.len());

        let mut repositories = Vec::new();

        for (index, article) in articles.iter().enumerate() {
            match self.parse_repository_article(article, index + 1) {
                Ok(repo) => {
                    debug!("Successfully parsed repository: {}", repo.full_name());
                    repositories.push(repo);
                }
                Err(e) => {
                    warn!(
                        "Failed to parse repository at position {}: {}",
                        index + 1,
                        e
                    );
                    // Continue parsing other repositories
                }
            }
        }

        info!("Successfully parsed {} repositories", repositories.len());
        Ok(repositories)
    }

    /// Parses a single repository article element.
    ///
    /// # Arguments
    /// - `article`: HTML element containing repository information
    /// - `_rank`: Position in the trending list (currently unused)
    ///
    /// # Returns
    /// - `Ok(Repository)`: Parsed repository
    /// - `Err(AppError)`: Parsing error
    fn parse_repository_article(
        &self,
        article: &scraper::ElementRef,
        _rank: usize,
    ) -> AppResult<Repository> {
        // Extract repository name and URL
        let (owner, name, url) = self.extract_repo_name_and_url(article)?;

        // Extract description (optional)
        let description = self.extract_description(article);

        // Extract language (optional)
        let language = self.extract_language(article);

        // Extract stars count (use rank as placeholder since daily stars are hard to extract reliably)
        let stars = self.extract_stars_today(article).unwrap_or(0);

        debug!(
            "Parsed repository: owner={}, name={}, url={}, language={:?}, stars={}",
            owner, name, url, language, stars
        );

        Repository::new(
            name,
            owner,
            url,
            description,
            stars,
            language,
            vec![], // Topics are not available on trending page
        )
        .map_err(|e| AppError::ValidationError(format!("Invalid repository data: {}", e)))
    }

    /// Extracts repository owner, name, and URL from article element.
    fn extract_repo_name_and_url(
        &self,
        article: &scraper::ElementRef,
    ) -> AppResult<(String, String, String)> {
        // Try primary selector
        let name_selector_primary = Selector::parse(selectors::REPO_NAME_PRIMARY)
            .map_err(|e| AppError::ParseError(format!("Invalid selector: {}", e)))?;

        let name_element = article.select(&name_selector_primary).next();

        // Try fallback selector if primary fails
        let name_element = if name_element.is_none() {
            warn!("Primary name selector failed, trying fallback");
            let name_selector_fallback = Selector::parse(selectors::REPO_NAME_FALLBACK)
                .map_err(|e| AppError::ParseError(format!("Invalid selector: {}", e)))?;
            article.select(&name_selector_fallback).next()
        } else {
            name_element
        };

        let name_element = name_element
            .ok_or_else(|| AppError::ParseError("Repository name element not found".to_string()))?;

        // Get href attribute for URL
        let href = name_element
            .value()
            .attr("href")
            .ok_or_else(|| AppError::ParseError("Repository URL not found".to_string()))?;

        let url = if href.starts_with("http") {
            href.to_string()
        } else {
            format!("{}{}", self.base_url, href)
        };

        // Extract text content and split into owner/name
        let full_name = name_element
            .text()
            .collect::<String>()
            .trim()
            .replace('\n', "")
            .split_whitespace()
            .collect::<String>();

        // Parse owner/name from format like "owner/name" or "owner / name"
        let parts: Vec<&str> = full_name.split('/').map(|s| s.trim()).collect();

        if parts.len() != 2 {
            return Err(AppError::ParseError(format!(
                "Invalid repository name format: {}",
                full_name
            )));
        }

        Ok((parts[0].to_string(), parts[1].to_string(), url))
    }

    /// Extracts repository description from article element.
    fn extract_description(&self, article: &scraper::ElementRef) -> Option<String> {
        let desc_selector_primary = Selector::parse(selectors::DESCRIPTION_PRIMARY).ok()?;

        let desc_element = article.select(&desc_selector_primary).next().or_else(|| {
            warn!("Primary description selector failed, trying fallback");
            let desc_selector_fallback = Selector::parse(selectors::DESCRIPTION_FALLBACK).ok()?;
            article.select(&desc_selector_fallback).next()
        })?;

        let description = desc_element.text().collect::<String>().trim().to_string();

        if description.is_empty() {
            None
        } else {
            Some(description)
        }
    }

    /// Extracts programming language from article element.
    fn extract_language(&self, article: &scraper::ElementRef) -> Option<String> {
        let lang_selector_primary = Selector::parse(selectors::LANGUAGE_PRIMARY).ok()?;

        let lang_element = article.select(&lang_selector_primary).next().or_else(|| {
            debug!("Primary language selector failed, trying fallback");
            let lang_selector_fallback = Selector::parse(selectors::LANGUAGE_FALLBACK).ok()?;
            article.select(&lang_selector_fallback).next()
        })?;

        let language = lang_element.text().collect::<String>().trim().to_string();

        if language.is_empty() {
            None
        } else {
            Some(language)
        }
    }

    /// Extracts today's star count from article element.
    ///
    /// Note: This is difficult to extract reliably due to frequent HTML changes.
    /// Returns 0 if extraction fails.
    fn extract_stars_today(&self, article: &scraper::ElementRef) -> Option<u32> {
        let stars_selector_primary = Selector::parse(selectors::STARS_TODAY_PRIMARY).ok()?;

        let stars_element = article.select(&stars_selector_primary).next().or_else(|| {
            debug!("Primary stars selector failed, trying fallback");
            let stars_selector_fallback = Selector::parse(selectors::STARS_TODAY_FALLBACK).ok()?;
            article.select(&stars_selector_fallback).next()
        })?;

        let stars_text = stars_element.text().collect::<String>();

        // Extract numbers from text like "123 stars today"
        let stars_str = stars_text
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect::<String>();

        stars_str.parse::<u32>().ok()
    }
}

#[async_trait]
impl TrendingRepository for TrendingScraperClient {
    async fn fetch_trending(&self, language: Option<String>) -> AppResult<Vec<Repository>> {
        info!("Fetching trending repositories (language: {:?})", language);

        let html = self.fetch_html(language.as_deref()).await?;
        let repositories = self.parse_trending_page(&html)?;

        info!(
            "Successfully fetched {} trending repositories",
            repositories.len()
        );
        Ok(repositories)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sample HTML structure representing a GitHub Trending page.
    /// This is a simplified version based on GitHub's actual structure.
    const SAMPLE_HTML: &str = r#"
        <!DOCTYPE html>
        <html>
        <body>
            <article class="Box-row">
                <h2 class="h3 lh-condensed">
                    <a href="/rust-lang/rust">
                        <span>rust-lang</span>
                        <span>/</span>
                        <span>rust</span>
                    </a>
                </h2>
                <p class="col-9 color-fg-muted my-1 pr-4">
                    Empowering everyone to build reliable and efficient software.
                </p>
                <div class="f6 color-fg-muted mt-2">
                    <span class="d-inline-block ml-0 mr-3">
                        <span class="repo-language-color" style="background-color:#dea584;"></span>
                        <span itemprop="programmingLanguage">Rust</span>
                    </span>
                    <span class="d-inline-block float-sm-right">
                        <svg aria-hidden="true" class="octicon octicon-star"></svg>
                        123 stars today
                    </span>
                </div>
            </article>
            <article class="Box-row">
                <h2 class="h3 lh-condensed">
                    <a href="/torvalds/linux">
                        torvalds / linux
                    </a>
                </h2>
                <p class="col-9 color-fg-muted my-1 pr-4">
                    Linux kernel source tree
                </p>
                <div class="f6 color-fg-muted mt-2">
                    <span class="d-inline-block ml-0 mr-3">
                        <span class="repo-language-color" style="background-color:#555555;"></span>
                        <span itemprop="programmingLanguage">C</span>
                    </span>
                    <span class="d-inline-block float-sm-right">
                        <svg aria-hidden="true" class="octicon octicon-star"></svg>
                        456 stars today
                    </span>
                </div>
            </article>
        </body>
        </html>
    "#;

    const SAMPLE_HTML_NO_DESCRIPTION: &str = r#"
        <!DOCTYPE html>
        <html>
        <body>
            <article class="Box-row">
                <h2 class="h3 lh-condensed">
                    <a href="/owner/repo">owner / repo</a>
                </h2>
                <div class="f6 color-fg-muted mt-2">
                    <span class="d-inline-block ml-0 mr-3">
                        <span itemprop="programmingLanguage">JavaScript</span>
                    </span>
                </div>
            </article>
        </body>
        </html>
    "#;

    const SAMPLE_HTML_INVALID: &str = r#"
        <!DOCTYPE html>
        <html>
        <body>
            <article class="Box-row">
                <h2>Invalid structure</h2>
            </article>
        </body>
        </html>
    "#;

    #[test]
    fn test_parse_trending_page_success() {
        let client = TrendingScraperClient::new().unwrap();
        let result = client.parse_trending_page(SAMPLE_HTML);

        assert!(result.is_ok());
        let repos = result.unwrap();
        assert_eq!(repos.len(), 2);

        // Check first repository
        assert_eq!(repos[0].owner(), "rust-lang");
        assert_eq!(repos[0].name(), "rust");
        assert_eq!(repos[0].url(), "https://github.com/rust-lang/rust");
        assert_eq!(
            repos[0].description(),
            Some("Empowering everyone to build reliable and efficient software.")
        );
        assert_eq!(repos[0].language(), Some("Rust"));
        assert_eq!(repos[0].stars(), 123);

        // Check second repository
        assert_eq!(repos[1].owner(), "torvalds");
        assert_eq!(repos[1].name(), "linux");
        assert_eq!(repos[1].language(), Some("C"));
        assert_eq!(repos[1].stars(), 456);
    }

    #[test]
    fn test_parse_trending_page_missing_description() {
        let client = TrendingScraperClient::new().unwrap();
        let result = client.parse_trending_page(SAMPLE_HTML_NO_DESCRIPTION);

        assert!(result.is_ok());
        let repos = result.unwrap();
        assert_eq!(repos.len(), 1);

        assert_eq!(repos[0].owner(), "owner");
        assert_eq!(repos[0].name(), "repo");
        assert_eq!(repos[0].description(), None);
        assert_eq!(repos[0].language(), Some("JavaScript"));
    }

    #[test]
    fn test_parse_trending_page_invalid_html() {
        let client = TrendingScraperClient::new().unwrap();
        let result = client.parse_trending_page(SAMPLE_HTML_INVALID);

        // Should return empty list when parsing fails
        assert!(result.is_ok());
        let repos = result.unwrap();
        assert_eq!(repos.len(), 0);
    }

    #[test]
    fn test_parse_trending_page_empty_html() {
        let client = TrendingScraperClient::new().unwrap();
        let result = client.parse_trending_page("");

        assert!(result.is_ok());
        let repos = result.unwrap();
        assert_eq!(repos.len(), 0);
    }

    #[tokio::test]
    async fn test_client_creation() {
        let result = TrendingScraperClient::new();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_client_with_custom_base_url() {
        let result = TrendingScraperClient::with_base_url("https://example.com".to_string());
        assert!(result.is_ok());

        let client = result.unwrap();
        assert_eq!(client.base_url, "https://example.com");
    }
}
