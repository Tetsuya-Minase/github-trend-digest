/// Integration tests for GitHub Trending scraper with mock HTTP server.
///
/// These tests verify the scraper's behavior with realistic HTTP scenarios
/// using httpmock to simulate the GitHub Trending page.
use github_trending_summary::domain::repositories::TrendingRepository;
use github_trending_summary::infrastructure::scraper::TrendingScraperClient;
use httpmock::prelude::*;

/// Sample HTML representing a successful GitHub Trending page response.
const MOCK_TRENDING_HTML: &str = r#"
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
                <span itemprop="programmingLanguage">Rust</span>
            </span>
            <span class="d-inline-block float-sm-right">
                <svg aria-hidden="true" class="octicon octicon-star"></svg>
                250 stars today
            </span>
        </div>
    </article>
    <article class="Box-row">
        <h2 class="h3 lh-condensed">
            <a href="/serde-rs/serde">serde-rs / serde</a>
        </h2>
        <p class="col-9 color-fg-muted my-1 pr-4">
            Serialization framework for Rust
        </p>
        <div class="f6 color-fg-muted mt-2">
            <span class="d-inline-block ml-0 mr-3">
                <span itemprop="programmingLanguage">Rust</span>
            </span>
            <span class="d-inline-block float-sm-right">
                <svg aria-hidden="true" class="octicon octicon-star"></svg>
                180 stars today
            </span>
        </div>
    </article>
</body>
</html>
"#;

#[tokio::test]
async fn test_fetch_trending_with_mock_server() {
    // Start mock server
    let server = MockServer::start();

    // Configure mock response
    let mock = server.mock(|when, then| {
        when.method(GET).path("/trending");
        then.status(200)
            .header("content-type", "text/html")
            .body(MOCK_TRENDING_HTML);
    });

    // Create client with mock server URL
    let client = TrendingScraperClient::with_base_url(server.base_url()).unwrap();

    // Fetch trending repositories
    let result = client.fetch_trending(None).await;

    // Verify mock was called
    mock.assert();

    // Assertions
    assert!(
        result.is_ok(),
        "Failed to fetch trending: {:?}",
        result.err()
    );
    let repos = result.unwrap();
    assert_eq!(repos.len(), 2);

    // Verify first repository
    assert_eq!(repos[0].owner(), "rust-lang");
    assert_eq!(repos[0].name(), "rust");
    assert_eq!(
        repos[0].url(),
        format!("{}/rust-lang/rust", server.base_url())
    );
    assert_eq!(
        repos[0].description(),
        Some("Empowering everyone to build reliable and efficient software.")
    );
    assert_eq!(repos[0].language(), Some("Rust"));
    assert_eq!(repos[0].stars(), 250);

    // Verify second repository
    assert_eq!(repos[1].owner(), "serde-rs");
    assert_eq!(repos[1].name(), "serde");
    assert_eq!(
        repos[1].description(),
        Some("Serialization framework for Rust")
    );
    assert_eq!(repos[1].language(), Some("Rust"));
    assert_eq!(repos[1].stars(), 180);
}

#[tokio::test]
async fn test_fetch_trending_with_language_filter() {
    // Start mock server
    let server = MockServer::start();

    // Configure mock response for language-filtered request
    let mock = server.mock(|when, then| {
        when.method(GET).path("/trending/rust");
        then.status(200)
            .header("content-type", "text/html")
            .body(MOCK_TRENDING_HTML);
    });

    // Create client with mock server URL
    let client = TrendingScraperClient::with_base_url(server.base_url()).unwrap();

    // Fetch trending repositories with language filter
    let result = client.fetch_trending(Some("rust".to_string())).await;

    // Verify mock was called
    mock.assert();

    // Assertions
    assert!(result.is_ok());
    let repos = result.unwrap();
    assert_eq!(repos.len(), 2);
}

#[tokio::test]
async fn test_fetch_trending_http_404_error() {
    // Start mock server
    let server = MockServer::start();

    // Configure mock response with 404 error
    let mock = server.mock(|when, then| {
        when.method(GET).path("/trending");
        then.status(404);
    });

    // Create client with mock server URL
    let client = TrendingScraperClient::with_base_url(server.base_url()).unwrap();

    // Fetch trending repositories
    let result = client.fetch_trending(None).await;

    // Verify mock was called
    mock.assert();

    // Assertions
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(matches!(
        error,
        github_trending_summary::domain::errors::AppError::ScrapingError(_)
    ));
    assert!(error.to_string().contains("404"));
}

#[tokio::test]
async fn test_fetch_trending_http_429_rate_limit() {
    // Start mock server
    let server = MockServer::start();

    // Configure mock response with 429 Rate Limit error
    let mock = server.mock(|when, then| {
        when.method(GET).path("/trending");
        then.status(429);
    });

    // Create client with mock server URL
    let client = TrendingScraperClient::with_base_url(server.base_url()).unwrap();

    // Fetch trending repositories
    let result = client.fetch_trending(None).await;

    // Verify mock was called
    mock.assert();

    // Assertions
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(matches!(
        error,
        github_trending_summary::domain::errors::AppError::ScrapingError(_)
    ));
    assert!(error.to_string().contains("429"));
}

#[tokio::test]
async fn test_fetch_trending_empty_response() {
    // Start mock server
    let server = MockServer::start();

    // Configure mock response with empty HTML
    let mock = server.mock(|when, then| {
        when.method(GET).path("/trending");
        then.status(200)
            .header("content-type", "text/html")
            .body("<html><body></body></html>");
    });

    // Create client with mock server URL
    let client = TrendingScraperClient::with_base_url(server.base_url()).unwrap();

    // Fetch trending repositories
    let result = client.fetch_trending(None).await;

    // Verify mock was called
    mock.assert();

    // Assertions - should succeed but return empty list
    assert!(result.is_ok());
    let repos = result.unwrap();
    assert_eq!(repos.len(), 0);
}

#[tokio::test]
async fn test_fetch_trending_malformed_html() {
    // Start mock server
    let server = MockServer::start();

    const MALFORMED_HTML: &str = r#"
    <!DOCTYPE html>
    <html>
    <body>
        <article class="Box-row">
            <h2>Missing proper structure</h2>
        </article>
    </body>
    </html>
    "#;

    // Configure mock response with malformed HTML
    let mock = server.mock(|when, then| {
        when.method(GET).path("/trending");
        then.status(200)
            .header("content-type", "text/html")
            .body(MALFORMED_HTML);
    });

    // Create client with mock server URL
    let client = TrendingScraperClient::with_base_url(server.base_url()).unwrap();

    // Fetch trending repositories
    let result = client.fetch_trending(None).await;

    // Verify mock was called
    mock.assert();

    // Assertions - should succeed but return empty list due to parsing failures
    assert!(result.is_ok());
    let repos = result.unwrap();
    assert_eq!(repos.len(), 0);
}

#[tokio::test]
async fn test_fetch_trending_timeout() {
    // Note: Testing actual timeout requires a server that doesn't respond,
    // which is not easily achievable with httpmock. This test verifies
    // that the client has timeout configured properly.

    // Attempt to connect to a non-routable IP address (should timeout)
    let client_with_timeout =
        TrendingScraperClient::with_base_url("http://10.255.255.1".to_string()).unwrap();

    let result = client_with_timeout.fetch_trending(None).await;

    // Should fail with network error due to timeout
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        github_trending_summary::domain::errors::AppError::NetworkError(_)
    ));
}

#[tokio::test]
async fn test_fetch_trending_no_language_field() {
    // Start mock server
    let server = MockServer::start();

    const HTML_NO_LANGUAGE: &str = r#"
    <!DOCTYPE html>
    <html>
    <body>
        <article class="Box-row">
            <h2 class="h3 lh-condensed">
                <a href="/owner/repo">owner / repo</a>
            </h2>
            <p class="col-9 color-fg-muted my-1 pr-4">
                A repository without language specified
            </p>
        </article>
    </body>
    </html>
    "#;

    // Configure mock response
    let mock = server.mock(|when, then| {
        when.method(GET).path("/trending");
        then.status(200)
            .header("content-type", "text/html")
            .body(HTML_NO_LANGUAGE);
    });

    // Create client with mock server URL
    let client = TrendingScraperClient::with_base_url(server.base_url()).unwrap();

    // Fetch trending repositories
    let result = client.fetch_trending(None).await;

    // Verify mock was called
    mock.assert();

    // Assertions
    assert!(result.is_ok());
    let repos = result.unwrap();
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].owner(), "owner");
    assert_eq!(repos[0].name(), "repo");
    assert_eq!(repos[0].language(), None);
}
