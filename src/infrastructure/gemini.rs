/// Gemini AI client implementation of the `AiSummarizer` trait.
///
/// This client calls the Gemini `generateContent` API to summarize
/// repositories, extract use cases, and identify target languages.
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;

use super::config::SummarizerConfig;
use crate::domain::{AiSummarizer, AppError, AppResult, Repository};

/// Request body for the Gemini generateContent API.
#[derive(Debug, Serialize)]
struct GenerateContentRequest {
    contents: Vec<RequestContent>,
}

#[derive(Debug, Serialize)]
struct RequestContent {
    parts: Vec<RequestPart>,
}

#[derive(Debug, Serialize)]
struct RequestPart {
    text: String,
}

/// Response body of the Gemini generateContent API (success case).
#[derive(Debug, Deserialize)]
struct GenerateContentResponse {
    #[serde(default)]
    candidates: Vec<Candidate>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: Option<CandidateContent>,
}

#[derive(Debug, Deserialize)]
struct CandidateContent {
    #[serde(default)]
    parts: Vec<ResponsePart>,
}

#[derive(Debug, Deserialize)]
struct ResponsePart {
    #[serde(default)]
    text: String,
}

/// Base URL of the Gemini API in production.
const GEMINI_API_BASE_URL: &str = "https://generativelanguage.googleapis.com";

/// Timeout for Gemini API requests.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Client for the Gemini generateContent API.
///
/// Implements the `AiSummarizer` trait using the model and prompt
/// templates provided via `SummarizerConfig`.
pub struct GeminiClient {
    client: Client,
    base_url: String,
    api_key: String,
    config: SummarizerConfig,
}

impl GeminiClient {
    /// Creates a new GeminiClient pointing at the production Gemini API.
    pub fn new(api_key: String, config: SummarizerConfig) -> AppResult<Self> {
        Self::with_base_url(GEMINI_API_BASE_URL.to_string(), api_key, config)
    }

    /// Creates a new GeminiClient with a custom base URL (used in tests).
    pub fn with_base_url(
        base_url: String,
        api_key: String,
        config: SummarizerConfig,
    ) -> AppResult<Self> {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| AppError::InternalError(format!("Failed to build HTTP client: {}", e)))?;

        Ok(Self {
            client,
            base_url,
            api_key,
            config,
        })
    }

    /// Sends a prompt to the Gemini generateContent API and returns the
    /// text of the first candidate.
    async fn generate_content(&self, prompt: &str) -> AppResult<String> {
        let url = format!(
            "{}/v1beta/models/{}:generateContent",
            self.base_url, self.config.model
        );
        let body = GenerateContentRequest {
            contents: vec![RequestContent {
                parts: vec![RequestPart {
                    text: prompt.to_string(),
                }],
            }],
        };

        debug!("Calling Gemini API with model {}", self.config.model);
        let response = self
            .client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let status_code = status.as_u16();
            let error_body = response.text().await.unwrap_or_default();
            return Err(match status_code {
                400 | 401 | 403 => AppError::AiServiceError(format!(
                    "Gemini API auth/request error {}: {}",
                    status_code, error_body
                )),
                429 => AppError::AiServiceError(format!(
                    "Gemini API rate limit exceeded: {}",
                    error_body
                )),
                500..=599 => {
                    AppError::AiServiceError(format!("Gemini API server error {}", status_code))
                }
                _ => AppError::AiServiceError(format!(
                    "Gemini API unexpected status {}: {}",
                    status_code, error_body
                )),
            });
        }

        let parsed: GenerateContentResponse = response.json().await.map_err(|e| {
            AppError::AiServiceError(format!("Failed to parse Gemini response: {}", e))
        })?;

        parsed
            .candidates
            .into_iter()
            .next()
            .and_then(|candidate| candidate.content)
            .and_then(|content| content.parts.into_iter().next())
            .map(|part| part.text)
            .filter(|text| !text.trim().is_empty())
            .ok_or_else(|| {
                AppError::AiServiceError("Gemini returned an empty response".to_string())
            })
    }
}

#[async_trait]
impl AiSummarizer for GeminiClient {
    async fn summarize(&self, repository: &Repository) -> AppResult<String> {
        let prompt = build_prompt(&self.config.summarize_prompt, repository);
        let text = self.generate_content(&prompt).await?;
        let summary = text.trim().to_string();
        if summary.is_empty() {
            return Err(AppError::AiServiceError(
                "Gemini returned an empty response".to_string(),
            ));
        }
        Ok(summary)
    }

    async fn extract_use_cases(&self, repository: &Repository) -> AppResult<Vec<String>> {
        let prompt = build_prompt(&self.config.use_cases_prompt, repository);
        let text = self.generate_content(&prompt).await?;
        Ok(parse_list(&text))
    }

    async fn identify_target_languages(&self, repository: &Repository) -> AppResult<Vec<String>> {
        let prompt = build_prompt(&self.config.target_languages_prompt, repository);
        let text = self.generate_content(&prompt).await?;
        Ok(parse_list(&text))
    }
}

/// Builds a prompt by replacing the `{{repository}}` placeholder in the
/// template with a repository info block.
fn build_prompt(template: &str, repo: &Repository) -> String {
    let topics = if repo.topics().is_empty() {
        "(none)".to_string()
    } else {
        repo.topics().join(", ")
    };
    let repository_block = format!(
        "Name: {}\nURL: {}\nDescription: {}\nPrimary Language: {}\nStars: {}\nTopics: {}",
        repo.full_name(),
        repo.url(),
        repo.description().unwrap_or("(none)"),
        repo.language().unwrap_or("(unknown)"),
        repo.stars(),
        topics,
    );
    template.replace("{{repository}}", &repository_block)
}

/// Parses a bullet-point list returned by the AI into a plain string list.
///
/// Strips leading `- `, `* `, and numeric (`1. `) prefixes from each line,
/// trims whitespace, and removes empty lines. An empty result is allowed.
fn parse_list(text: &str) -> Vec<String> {
    text.lines()
        .map(strip_list_prefix)
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

/// Strips a leading `- `, `* `, or numeric (`1. `) list prefix from a line.
fn strip_list_prefix(line: &str) -> &str {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
    {
        return rest;
    }
    if let Some(dot_pos) = trimmed.find(". ") {
        if dot_pos > 0 && trimmed[..dot_pos].chars().all(|c| c.is_ascii_digit()) {
            return &trimmed[dot_pos + 2..];
        }
    }
    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    fn create_test_client(base_url: String) -> GeminiClient {
        GeminiClient::with_base_url(
            base_url,
            "test-api-key".to_string(),
            SummarizerConfig::default(),
        )
        .unwrap()
    }

    fn create_test_repository() -> Repository {
        Repository::new(
            "rust".to_string(),
            "rust-lang".to_string(),
            "https://github.com/rust-lang/rust".to_string(),
            Some("Empowering everyone".to_string()),
            80000,
            Some("Rust".to_string()),
            vec!["compiler".to_string(), "language".to_string()],
        )
        .unwrap()
    }

    #[test]
    fn test_parse_list_strips_bullets() {
        let result = parse_list("- A\n* B\n1. C\n\n");
        assert_eq!(result, vec!["A", "B", "C"]);
    }

    #[test]
    fn test_build_prompt_replaces_placeholder() {
        let repo = create_test_repository();
        let prompt = build_prompt("Summarize this:\n{{repository}}\n", &repo);

        assert!(!prompt.contains("{{repository}}"));
        assert!(prompt.contains("Name: rust-lang/rust"));
        assert!(prompt.contains("URL: https://github.com/rust-lang/rust"));
        assert!(prompt.contains("Stars: 80000"));
        assert!(prompt.contains("Description: Empowering everyone"));
        assert!(prompt.contains("Primary Language: Rust"));
        assert!(prompt.contains("Topics: compiler, language"));
    }

    #[tokio::test]
    async fn test_summarize_success() {
        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1beta/models/gemini-2.5-flash:generateContent")
                .header("x-goog-api-key", "test-api-key");
            then.status(200).json_body(serde_json::json!({
                "candidates": [
                    {
                        "content": {
                            "parts": [ { "text": "テスト要約です。" } ],
                            "role": "model"
                        }
                    }
                ]
            }));
        });

        let client = create_test_client(server.base_url());
        let repo = create_test_repository();

        let result = client.summarize(&repo).await;

        mock.assert();
        assert_eq!(result.unwrap(), "テスト要約です。");
    }

    #[tokio::test]
    async fn test_extract_use_cases_success() {
        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1beta/models/gemini-2.5-flash:generateContent")
                .header("x-goog-api-key", "test-api-key");
            then.status(200).json_body(serde_json::json!({
                "candidates": [
                    {
                        "content": {
                            "parts": [ { "text": "- CLIツール\n- CI連携" } ],
                            "role": "model"
                        }
                    }
                ]
            }));
        });

        let client = create_test_client(server.base_url());
        let repo = create_test_repository();

        let result = client.extract_use_cases(&repo).await;

        mock.assert();
        assert_eq!(result.unwrap(), vec!["CLIツール", "CI連携"]);
    }

    #[tokio::test]
    async fn test_identify_target_languages_success() {
        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1beta/models/gemini-2.5-flash:generateContent")
                .header("x-goog-api-key", "test-api-key");
            then.status(200).json_body(serde_json::json!({
                "candidates": [
                    {
                        "content": {
                            "parts": [ { "text": "- Rust\n- Go" } ],
                            "role": "model"
                        }
                    }
                ]
            }));
        });

        let client = create_test_client(server.base_url());
        let repo = create_test_repository();

        let result = client.identify_target_languages(&repo).await;

        mock.assert();
        assert_eq!(result.unwrap(), vec!["Rust", "Go"]);
    }

    #[tokio::test]
    async fn test_gemini_api_error_unauthorized() {
        for status in [400u16, 403] {
            let server = MockServer::start();

            let mock = server.mock(|when, then| {
                when.method(POST)
                    .path("/v1beta/models/gemini-2.5-flash:generateContent");
                then.status(status).body("API key not valid");
            });

            let client = create_test_client(server.base_url());
            let repo = create_test_repository();

            let result = client.summarize(&repo).await;

            mock.assert();
            match result.unwrap_err() {
                AppError::AiServiceError(msg) => {
                    assert!(msg.contains(&status.to_string()));
                    assert!(msg.contains("API key not valid"));
                }
                other => panic!("Expected AiServiceError, got {:?}", other),
            }
        }
    }

    #[tokio::test]
    async fn test_gemini_api_error_rate_limited() {
        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1beta/models/gemini-2.5-flash:generateContent");
            then.status(429).body("Resource has been exhausted");
        });

        let client = create_test_client(server.base_url());
        let repo = create_test_repository();

        let result = client.summarize(&repo).await;

        mock.assert();
        match result.unwrap_err() {
            AppError::AiServiceError(msg) => {
                assert!(msg.contains("rate limit"));
            }
            other => panic!("Expected AiServiceError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_gemini_api_error_server_error() {
        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1beta/models/gemini-2.5-flash:generateContent");
            then.status(500).body("Internal error");
        });

        let client = create_test_client(server.base_url());
        let repo = create_test_repository();

        let result = client.summarize(&repo).await;

        mock.assert();
        match result.unwrap_err() {
            AppError::AiServiceError(msg) => {
                assert!(msg.contains("server error 500"));
            }
            other => panic!("Expected AiServiceError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_gemini_whitespace_only_text_is_empty_response() {
        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1beta/models/gemini-2.5-flash:generateContent");
            then.status(200).json_body(serde_json::json!({
                "candidates": [
                    {
                        "content": {
                            "parts": [ { "text": "  \n " } ],
                            "role": "model"
                        }
                    }
                ]
            }));
        });

        let client = create_test_client(server.base_url());
        let repo = create_test_repository();

        let result = client.summarize(&repo).await;

        mock.assert();
        match result.unwrap_err() {
            AppError::AiServiceError(msg) => {
                assert!(msg.contains("empty response"));
            }
            other => panic!("Expected AiServiceError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_gemini_invalid_json_response() {
        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1beta/models/gemini-2.5-flash:generateContent");
            then.status(200).body("not a json");
        });

        let client = create_test_client(server.base_url());
        let repo = create_test_repository();

        let result = client.summarize(&repo).await;

        mock.assert();
        match result.unwrap_err() {
            AppError::AiServiceError(msg) => {
                assert!(msg.contains("Failed to parse Gemini response"));
            }
            other => panic!("Expected AiServiceError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_gemini_empty_candidates() {
        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1beta/models/gemini-2.5-flash:generateContent");
            then.status(200)
                .json_body(serde_json::json!({ "candidates": [] }));
        });

        let client = create_test_client(server.base_url());
        let repo = create_test_repository();

        let result = client.summarize(&repo).await;

        mock.assert();
        match result.unwrap_err() {
            AppError::AiServiceError(msg) => {
                assert!(msg.contains("empty response"));
            }
            other => panic!("Expected AiServiceError, got {:?}", other),
        }
    }
}
