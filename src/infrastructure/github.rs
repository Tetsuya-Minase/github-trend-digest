//! GitHub REST API client implementation.
//!
//! This module implements the `GithubApiClient` trait using the GitHub REST API
//! to retrieve detailed repository information (topics, accurate star counts, etc.)
//! and to verify API credentials.
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, info};

use crate::domain::entities::Repository;
use crate::domain::errors::{AppError, AppResult};
use crate::domain::repositories::GithubApiClient;

/// Default base URL for the GitHub REST API.
const GITHUB_API_BASE_URL: &str = "https://api.github.com";

/// User-Agent header value. GitHub API requires a User-Agent header;
/// requests without it are rejected with 403.
const USER_AGENT: &str = "github-trending-summary";

/// Client for interacting with the GitHub REST API.
///
/// # Design Notes
/// - Uses reqwest with a request timeout
/// - Supports optional authentication via GITHUB_TOKEN (unauthenticated access
///   is allowed by the API with lower rate limits)
/// - Base URL is injectable for testing with mock servers
pub struct GithubApiClientImpl {
    client: Client,
    base_url: String,
    token: Option<String>,
}

impl GithubApiClientImpl {
    /// Creates a new GithubApiClientImpl targeting the production GitHub API.
    ///
    /// # Arguments
    /// - `token`: Optional GitHub token. When `None`, requests are unauthenticated.
    ///
    /// # Returns
    /// - `Ok(GithubApiClientImpl)`: Successfully created client
    /// - `Err(AppError)`: Failed to build the HTTP client
    pub fn new(token: Option<String>) -> AppResult<Self> {
        Self::with_base_url(GITHUB_API_BASE_URL.to_string(), token)
    }

    /// Creates a new GithubApiClientImpl with a custom base URL.
    ///
    /// This is primarily useful for testing with mock servers.
    ///
    /// # Arguments
    /// - `base_url`: Base URL for the GitHub API (e.g., "https://api.github.com")
    /// - `token`: Optional GitHub token
    ///
    /// # Returns
    /// - `Ok(GithubApiClientImpl)`: Successfully created client
    /// - `Err(AppError)`: Failed to build the HTTP client
    pub fn with_base_url(base_url: String, token: Option<String>) -> AppResult<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| AppError::InternalError(format!("Failed to create HTTP client: {}", e)))?;

        info!(
            "GithubApiClientImpl initialized with base_url: {}",
            base_url
        );

        Ok(Self {
            client,
            base_url,
            token,
        })
    }

    /// Builds a GET request with the common headers required by the GitHub API.
    fn get_request(&self, url: &str) -> reqwest::RequestBuilder {
        let mut request = self
            .client
            .get(url)
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");

        if let Some(token) = &self.token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        request
    }

    /// Maps a non-success GitHub API response to an AppError.
    ///
    /// Consumes the response to include its body (the GitHub API `message`
    /// field) in the error for diagnosability. A 403 is reported as a rate
    /// limit error only when the `x-ratelimit-remaining` header is exhausted;
    /// otherwise (SAML enforcement, insufficient scope, etc.) it is reported
    /// as forbidden with the API message.
    async fn error_for_status(
        status: reqwest::StatusCode,
        response: reqwest::Response,
        owner: &str,
        name: &str,
    ) -> AppError {
        let rate_limit_exhausted = response
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            == Some("0");
        let body_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        error!(
            "GitHub API error fetching {}/{}: {} - {}",
            owner, name, status, body_text
        );

        match status {
            reqwest::StatusCode::NOT_FOUND => {
                AppError::GithubApiError(format!("Repository not found: {}/{}", owner, name))
            }
            reqwest::StatusCode::TOO_MANY_REQUESTS => AppError::GithubApiError(format!(
                "GitHub API rate limit exceeded: status {}",
                status.as_u16()
            )),
            reqwest::StatusCode::FORBIDDEN if rate_limit_exhausted => AppError::GithubApiError(
                format!("GitHub API rate limit exceeded: status {}", status.as_u16()),
            ),
            reqwest::StatusCode::FORBIDDEN => {
                AppError::GithubApiError(format!("GitHub API access forbidden: {}", body_text))
            }
            reqwest::StatusCode::UNAUTHORIZED => {
                AppError::GithubApiError("Unauthorized: invalid GitHub token".to_string())
            }
            _ => AppError::GithubApiError(format!(
                "GitHub API returned status {}: {}",
                status.as_u16(),
                body_text
            )),
        }
    }
}

/// DTO for deserializing the GitHub API repository response.
#[derive(Debug, Deserialize)]
struct GithubRepoResponse {
    name: String,
    owner: GithubOwner,
    html_url: String,
    description: Option<String>,
    stargazers_count: u32,
    language: Option<String>,
    #[serde(default)]
    topics: Vec<String>,
}

/// DTO for the `owner` object in the GitHub API repository response.
#[derive(Debug, Deserialize)]
struct GithubOwner {
    login: String,
}

#[async_trait]
impl GithubApiClient for GithubApiClientImpl {
    async fn get_repository_details(&self, owner: &str, name: &str) -> AppResult<Repository> {
        let url = format!("{}/repos/{}/{}", self.base_url, owner, name);
        info!("Fetching repository details: {}/{}", owner, name);

        let response = self.get_request(&url).send().await?;

        let status = response.status();
        if !status.is_success() {
            return Err(Self::error_for_status(status, response, owner, name).await);
        }

        let body: GithubRepoResponse = response.json().await.map_err(|e| {
            error!("Failed to parse GitHub API response: {}", e);
            AppError::ParseError(format!("Failed to parse GitHub API response: {}", e))
        })?;

        Repository::new(
            body.name,
            body.owner.login,
            body.html_url,
            body.description,
            body.stargazers_count,
            body.language,
            body.topics,
        )
        .map_err(|e| AppError::ValidationError(format!("Invalid repository data: {}", e)))
    }

    /// Checks if the API client has valid credentials.
    ///
    /// Calls `GET /rate_limit`, which works for both authenticated and
    /// unauthenticated clients and returns 401 for invalid tokens.
    async fn verify_credentials(&self) -> AppResult<()> {
        let url = format!("{}/rate_limit", self.base_url);
        debug!("Verifying GitHub API credentials");

        let response = self.get_request(&url).send().await?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            error!("GitHub API credential verification failed: invalid token");
            return Err(AppError::GithubApiError(
                "Unauthorized: invalid GitHub token".to_string(),
            ));
        }
        if !status.is_success() {
            error!("GitHub API credential verification failed: {}", status);
            return Err(AppError::GithubApiError(format!(
                "GitHub API returned status {}",
                status.as_u16()
            )));
        }

        debug!("Credential verification succeeded: {}", status);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[tokio::test]
    async fn test_verify_credentials_success() {
        // Given: a mock server that returns 200 for GET /rate_limit
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/rate_limit")
                .header("authorization", "Bearer test-token")
                .header("user-agent", "github-trending-summary")
                .header("accept", "application/vnd.github+json")
                .header("x-github-api-version", "2022-11-28");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"resources":{}}"#);
        });

        let client =
            GithubApiClientImpl::with_base_url(server.base_url(), Some("test-token".to_string()))
                .unwrap();

        // When: verifying credentials
        let result = client.verify_credentials().await;

        // Then: the request is sent with the expected headers and Ok is returned
        mock.assert();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_verify_credentials_unauthorized() {
        // Given: a mock server that returns 401 for GET /rate_limit
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/rate_limit");
            then.status(401)
                .header("content-type", "application/json")
                .body(r#"{"message":"Bad credentials"}"#);
        });

        let client = GithubApiClientImpl::with_base_url(
            server.base_url(),
            Some("invalid-token".to_string()),
        )
        .unwrap();

        // When: verifying credentials
        let result = client.verify_credentials().await;

        // Then: a GithubApiError containing "Unauthorized" is returned
        mock.assert();
        match result {
            Err(AppError::GithubApiError(msg)) => {
                assert!(msg.contains("Unauthorized"), "unexpected message: {}", msg)
            }
            other => panic!("Expected GithubApiError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_verify_credentials_without_token() {
        // Given: a mock server that expects a request without an Authorization header
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/rate_limit")
                .header_missing("authorization");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"resources":{}}"#);
        });

        let client = GithubApiClientImpl::with_base_url(server.base_url(), None).unwrap();

        // When: verifying credentials without a token
        let result = client.verify_credentials().await;

        // Then: the request has no Authorization header and Ok is returned
        mock.assert();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_repository_details_success() {
        // Given: a mock server that returns repository details JSON
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/repos/rust-lang/rust");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"{
                        "name": "rust",
                        "owner": { "login": "rust-lang" },
                        "html_url": "https://github.com/rust-lang/rust",
                        "description": "Empowering everyone",
                        "stargazers_count": 80000,
                        "language": "Rust",
                        "topics": ["compiler", "rust"]
                    }"#,
                );
        });

        let client = GithubApiClientImpl::with_base_url(server.base_url(), None).unwrap();

        // When: fetching repository details
        let result = client.get_repository_details("rust-lang", "rust").await;

        // Then: the parsed Repository exposes the expected values via getters
        mock.assert();
        let repo = result.unwrap();
        assert_eq!(repo.name(), "rust");
        assert_eq!(repo.owner(), "rust-lang");
        assert_eq!(repo.url(), "https://github.com/rust-lang/rust");
        assert_eq!(repo.description(), Some("Empowering everyone"));
        assert_eq!(repo.stars(), 80000);
        assert_eq!(repo.language(), Some("Rust"));
        assert_eq!(repo.topics(), &["compiler".to_string(), "rust".to_string()]);
    }

    #[tokio::test]
    async fn test_get_repository_details_null_optional_fields() {
        // Given: a mock server response with null description/language and empty topics
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/repos/owner/repo");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"{
                        "name": "repo",
                        "owner": { "login": "owner" },
                        "html_url": "https://github.com/owner/repo",
                        "description": null,
                        "stargazers_count": 42,
                        "language": null,
                        "topics": []
                    }"#,
                );
        });

        let client = GithubApiClientImpl::with_base_url(server.base_url(), None).unwrap();

        // When: fetching repository details
        let result = client.get_repository_details("owner", "repo").await;

        // Then: parsing succeeds and optional fields are None/empty
        mock.assert();
        let repo = result.unwrap();
        assert_eq!(repo.description(), None);
        assert_eq!(repo.language(), None);
        assert!(repo.topics().is_empty());
    }

    #[tokio::test]
    async fn test_get_repository_details_not_found() {
        // Given: a mock server that returns 404
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/repos/owner/missing");
            then.status(404)
                .header("content-type", "application/json")
                .body(r#"{"message":"Not Found"}"#);
        });

        let client = GithubApiClientImpl::with_base_url(server.base_url(), None).unwrap();

        // When: fetching a non-existent repository
        let result = client.get_repository_details("owner", "missing").await;

        // Then: a GithubApiError whose message contains "not found" is returned
        mock.assert();
        match result {
            Err(AppError::GithubApiError(msg)) => assert!(
                msg.to_lowercase().contains("not found"),
                "unexpected message: {}",
                msg
            ),
            other => panic!("Expected GithubApiError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_get_repository_details_rate_limited() {
        // Given: a mock server that returns 403 with an exhausted rate limit header
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/repos/owner/repo");
            then.status(403)
                .header("content-type", "application/json")
                .header("x-ratelimit-remaining", "0")
                .body(r#"{"message":"API rate limit exceeded"}"#);
        });

        let client = GithubApiClientImpl::with_base_url(server.base_url(), None).unwrap();

        // When: fetching repository details while rate limited
        let result = client.get_repository_details("owner", "repo").await;

        // Then: a GithubApiError whose message contains "rate limit" is returned
        mock.assert();
        match result {
            Err(AppError::GithubApiError(msg)) => {
                assert!(msg.contains("rate limit"), "unexpected message: {}", msg)
            }
            other => panic!("Expected GithubApiError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_get_repository_details_too_many_requests() {
        // Given: a mock server that returns 429
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/repos/owner/repo");
            then.status(429)
                .header("content-type", "application/json")
                .body(r#"{"message":"You have exceeded a secondary rate limit"}"#);
        });

        let client = GithubApiClientImpl::with_base_url(server.base_url(), None).unwrap();

        // When: fetching repository details while throttled
        let result = client.get_repository_details("owner", "repo").await;

        // Then: a GithubApiError whose message contains "rate limit" is returned
        mock.assert();
        match result {
            Err(AppError::GithubApiError(msg)) => {
                assert!(msg.contains("rate limit"), "unexpected message: {}", msg)
            }
            other => panic!("Expected GithubApiError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_get_repository_details_forbidden_not_rate_limited() {
        // Given: a mock server that returns 403 without an exhausted rate limit
        // header (e.g., SAML enforcement or insufficient token scope)
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/repos/owner/repo");
            then.status(403)
                .header("content-type", "application/json")
                .header("x-ratelimit-remaining", "42")
                .body(r#"{"message":"Resource protected by organization SAML enforcement"}"#);
        });

        let client = GithubApiClientImpl::with_base_url(server.base_url(), None).unwrap();

        // When: fetching repository details
        let result = client.get_repository_details("owner", "repo").await;

        // Then: the error is reported as forbidden (not rate limit) and
        // includes the API message for diagnosability
        mock.assert();
        match result {
            Err(AppError::GithubApiError(msg)) => {
                assert!(msg.contains("forbidden"), "unexpected message: {}", msg);
                assert!(
                    msg.contains("SAML enforcement"),
                    "unexpected message: {}",
                    msg
                );
            }
            other => panic!("Expected GithubApiError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_get_repository_details_unauthorized() {
        // Given: a mock server that returns 401
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/repos/owner/repo");
            then.status(401)
                .header("content-type", "application/json")
                .body(r#"{"message":"Bad credentials"}"#);
        });

        let client =
            GithubApiClientImpl::with_base_url(server.base_url(), Some("bad-token".to_string()))
                .unwrap();

        // When: fetching repository details with an invalid token
        let result = client.get_repository_details("owner", "repo").await;

        // Then: a GithubApiError containing "Unauthorized" is returned
        mock.assert();
        match result {
            Err(AppError::GithubApiError(msg)) => {
                assert!(msg.contains("Unauthorized"), "unexpected message: {}", msg)
            }
            other => panic!("Expected GithubApiError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_get_repository_details_server_error_includes_body() {
        // Given: a mock server that returns 500 with an error message body
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/repos/owner/repo");
            then.status(500)
                .header("content-type", "application/json")
                .body(r#"{"message":"Internal boom"}"#);
        });

        let client = GithubApiClientImpl::with_base_url(server.base_url(), None).unwrap();

        // When: fetching repository details
        let result = client.get_repository_details("owner", "repo").await;

        // Then: the error includes both the status code and the response body
        mock.assert();
        match result {
            Err(AppError::GithubApiError(msg)) => {
                assert!(msg.contains("500"), "unexpected message: {}", msg);
                assert!(msg.contains("Internal boom"), "unexpected message: {}", msg);
            }
            other => panic!("Expected GithubApiError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_get_repository_details_invalid_json() {
        // Given: a mock server that returns a non-JSON body with status 200
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/repos/owner/repo");
            then.status(200)
                .header("content-type", "application/json")
                .body("not json");
        });

        let client = GithubApiClientImpl::with_base_url(server.base_url(), None).unwrap();

        // When: fetching repository details
        let result = client.get_repository_details("owner", "repo").await;

        // Then: a ParseError is returned
        mock.assert();
        assert!(
            matches!(result, Err(AppError::ParseError(_))),
            "Expected ParseError, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_get_repository_details_invalid_domain_data() {
        // Given: a mock server response that is valid JSON but violates
        // domain invariants (empty repository name)
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/repos/owner/repo");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"{
                        "name": "",
                        "owner": { "login": "owner" },
                        "html_url": "https://github.com/owner/repo",
                        "description": null,
                        "stargazers_count": 0,
                        "language": null,
                        "topics": []
                    }"#,
                );
        });

        let client = GithubApiClientImpl::with_base_url(server.base_url(), None).unwrap();

        // When: fetching repository details
        let result = client.get_repository_details("owner", "repo").await;

        // Then: a ValidationError is returned (consistent with the scraper impl)
        mock.assert();
        match result {
            Err(AppError::ValidationError(msg)) => assert!(
                msg.contains("Invalid repository data"),
                "unexpected message: {}",
                msg
            ),
            other => panic!("Expected ValidationError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_get_repository_details_network_error() {
        // Given: a base URL that refuses connections (reserved port 1)
        let client =
            GithubApiClientImpl::with_base_url("http://127.0.0.1:1".to_string(), None).unwrap();

        // When: fetching repository details
        let result = client.get_repository_details("owner", "repo").await;

        // Then: a NetworkError is returned via the From<reqwest::Error> conversion
        assert!(
            matches!(result, Err(AppError::NetworkError(_))),
            "Expected NetworkError, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_verify_credentials_server_error() {
        // Given: a mock server that returns 500 for GET /rate_limit
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/rate_limit");
            then.status(500)
                .header("content-type", "application/json")
                .body(r#"{"message":"Internal error"}"#);
        });

        let client = GithubApiClientImpl::with_base_url(server.base_url(), None).unwrap();

        // When: verifying credentials
        let result = client.verify_credentials().await;

        // Then: a GithubApiError containing the status code is returned
        mock.assert();
        match result {
            Err(AppError::GithubApiError(msg)) => {
                assert!(msg.contains("500"), "unexpected message: {}", msg)
            }
            other => panic!("Expected GithubApiError, got {:?}", other),
        }
    }
}
