/// Gemini AI client implementation of the `AiSummarizer` trait.
///
/// This client calls the Gemini `generateContent` API once per repository
/// and receives the summary, use cases, and target languages as a JSON
/// object (structured output).
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;

use super::config::SummarizerConfig;
use crate::domain::{AiSummarizer, AppError, AppResult, Repository, RepositoryInsight};

/// Request body for the Gemini generateContent API.
#[derive(Debug, Serialize)]
struct GenerateContentRequest {
    contents: Vec<RequestContent>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
}

#[derive(Debug, Serialize)]
struct GenerationConfig {
    #[serde(rename = "responseMimeType")]
    response_mime_type: &'static str,
    #[serde(rename = "responseJsonSchema")]
    response_json_schema: serde_json::Value,
    #[serde(rename = "thinkingConfig")]
    thinking_config: ThinkingConfig,
}

#[derive(Debug, Serialize)]
struct ThinkingConfig {
    #[serde(rename = "thinkingLevel")]
    thinking_level: &'static str,
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
    text: Option<String>,
    /// `true` for thought (reasoning) parts, which must not reach the result.
    #[serde(default)]
    thought: Option<bool>,
}

/// MIME type requested so that Gemini answers with JSON.
const RESPONSE_MIME_TYPE: &str = "application/json";

/// Thinking level requested from Gemini (kept low to limit latency and cost).
const THINKING_LEVEL: &str = "low";

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
            generation_config: GenerationConfig {
                response_mime_type: RESPONSE_MIME_TYPE,
                response_json_schema: insight_response_schema(),
                thinking_config: ThinkingConfig {
                    thinking_level: THINKING_LEVEL,
                },
            },
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

        extract_text(parsed).ok_or_else(|| {
            AppError::AiServiceError("Gemini returned an empty response".to_string())
        })
    }
}

#[async_trait]
impl AiSummarizer for GeminiClient {
    async fn analyze(&self, repository: &Repository) -> AppResult<RepositoryInsight> {
        let prompt = build_prompt(&self.config.analyze_prompt, repository);
        let text = self.generate_content(&prompt).await?;
        parse_insight(&text)
    }
}

/// JSON payload returned by Gemini under `insight_response_schema`.
///
/// Kept private so the domain type does not depend on the wire format.
#[derive(Debug, Deserialize)]
struct InsightPayload {
    summary: String,
    use_cases: Vec<String>,
    target_languages: Vec<String>,
}

/// Maximum number of items requested for each array in the response schema.
const MAX_LIST_ITEMS: u32 = 5;

/// Returns the JSON schema that constrains Gemini's structured output.
fn insight_response_schema() -> serde_json::Value {
    let list = serde_json::json!({
        "type": "array",
        "items": { "type": "string" },
        "maxItems": MAX_LIST_ITEMS
    });
    serde_json::json!({
        "type": "object",
        "properties": {
            "summary": { "type": "string" },
            "use_cases": list,
            "target_languages": list
        },
        "required": ["summary", "use_cases", "target_languages"]
    })
}

/// Extracts the answer text from the first candidate.
///
/// Thought parts and parts without text are skipped, and the remaining
/// texts are concatenated in order. Returns `None` if the result is blank.
fn extract_text(response: GenerateContentResponse) -> Option<String> {
    let text: String = response
        .candidates
        .into_iter()
        .next()?
        .content?
        .parts
        .into_iter()
        .filter(|part| part.thought != Some(true))
        .filter_map(|part| part.text)
        .collect();
    Some(text).filter(|text| !text.trim().is_empty())
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

/// Parses the answer text into a `RepositoryInsight`.
///
/// Strips an optional code fence, deserializes the JSON, trims the summary
/// and list items, and drops empty items. A blank summary is an error.
fn parse_insight(text: &str) -> AppResult<RepositoryInsight> {
    let payload: InsightPayload = serde_json::from_str(strip_code_fence(text)).map_err(|e| {
        AppError::AiServiceError(format!("Failed to parse Gemini JSON response: {}", e))
    })?;

    let summary = payload.summary.trim().to_string();
    if summary.is_empty() {
        return Err(AppError::AiServiceError(
            "Gemini returned an empty response".to_string(),
        ));
    }

    Ok(RepositoryInsight {
        summary,
        use_cases: clean_items(payload.use_cases),
        target_languages: clean_items(payload.target_languages),
    })
}

/// Trims each item and removes empty ones, keeping the original order.
fn clean_items(items: Vec<String>) -> Vec<String> {
    items
        .into_iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

/// Removes a surrounding Markdown code fence (```` ```json ```` or ```` ``` ````).
///
/// Surrounding whitespace is trimmed; text without a fence is returned trimmed.
fn strip_code_fence(text: &str) -> &str {
    let trimmed = text.trim();
    let Some(rest) = trimmed.strip_prefix("```") else {
        return trimmed;
    };
    // Drop the opening fence line (the language tag, if any).
    let body = rest.split_once('\n').map_or("", |(_, body)| body);
    body.trim_end().trim_end_matches("```").trim()
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    const INSIGHT_JSON: &str = r#"{"summary":"Rust製のCLIツール。","use_cases":["CLIツール","CI連携"],"target_languages":["Rust"]}"#;

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

    fn expected_insight() -> RepositoryInsight {
        RepositoryInsight {
            summary: "Rust製のCLIツール。".to_string(),
            use_cases: vec!["CLIツール".to_string(), "CI連携".to_string()],
            target_languages: vec!["Rust".to_string()],
        }
    }

    fn insight(summary: &str, use_cases: &[&str], target_languages: &[&str]) -> RepositoryInsight {
        RepositoryInsight {
            summary: summary.to_string(),
            use_cases: use_cases.iter().map(|s| s.to_string()).collect(),
            target_languages: target_languages.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Starts a mock server returning the given `parts` and returns the
    /// server together with a client pointing at it.
    fn start_server_with_parts(parts: serde_json::Value) -> (MockServer, GeminiClient) {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST)
                .path("/v1beta/models/gemini-3.8-flash:generateContent");
            then.status(200).json_body(serde_json::json!({
                "candidates": [ { "content": { "parts": parts, "role": "model" } } ]
            }));
        });
        let client = create_test_client(server.base_url());
        (server, client)
    }

    /// Starts a mock server answering with the given status and body.
    fn start_server_with_status(status: u16, body: &str) -> (MockServer, GeminiClient) {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST)
                .path("/v1beta/models/gemini-3.8-flash:generateContent");
            then.status(status).body(body);
        });
        let client = create_test_client(server.base_url());
        (server, client)
    }

    fn assert_ai_error_contains<T: std::fmt::Debug>(result: AppResult<T>, expected: &str) {
        match result.unwrap_err() {
            AppError::AiServiceError(msg) => {
                assert!(
                    msg.contains(expected),
                    "msg: {}, expected: {}",
                    msg,
                    expected
                )
            }
            other => panic!("Expected AiServiceError, got {:?}", other),
        }
    }

    // ---- insight_response_schema (T1) ----

    #[test]
    fn test_insight_response_schema_has_three_properties_and_required() {
        // Given: no input
        // When: building the schema
        let schema = insight_response_schema();

        // Then: object with the three properties and required keys
        let list = serde_json::json!({
            "type": "array", "items": { "type": "string" }, "maxItems": 5
        });
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["summary"]["type"], "string");
        assert_eq!(schema["properties"]["use_cases"], list);
        assert_eq!(schema["properties"]["target_languages"], list);
        assert_eq!(
            schema["required"],
            serde_json::json!(["summary", "use_cases", "target_languages"])
        );
    }

    // ---- parse_insight (T2-T12) ----

    #[test]
    fn test_parse_insight_plain_json() {
        // Given: plain JSON text
        // When
        let result = parse_insight(INSIGHT_JSON);

        // Then
        assert_eq!(result.unwrap(), expected_insight());
    }

    #[test]
    fn test_parse_insight_json_fenced() {
        // Given: a ```json fenced JSON
        let text = "```json\n{\"summary\":\"要約。\",\"use_cases\":[\"A\"],\"target_languages\":[\"Rust\"]}\n```";

        // When
        let result = parse_insight(text);

        // Then
        assert_eq!(result.unwrap(), insight("要約。", &["A"], &["Rust"]));
    }

    #[test]
    fn test_parse_insight_plain_fence_with_surrounding_whitespace() {
        // Given: a language-less fence with surrounding whitespace
        let text =
            "  \n```\n{\"summary\":\"要約。\",\"use_cases\":[],\"target_languages\":[]}\n```\n ";

        // When
        let result = parse_insight(text);

        // Then
        assert_eq!(result.unwrap(), insight("要約。", &[], &[]));
    }

    #[test]
    fn test_parse_insight_trims_and_removes_empty_items() {
        // Given
        let text = r#"{"summary":"  要約。 \n","use_cases":["  CLIツール ","","   ","CI連携"],"target_languages":[" Rust ","\n"]}"#;

        // When
        let result = parse_insight(text);

        // Then
        assert_eq!(
            result.unwrap(),
            insight("要約。", &["CLIツール", "CI連携"], &["Rust"])
        );
    }

    #[test]
    fn test_parse_insight_empty_arrays_are_ok() {
        // Given
        let text = r#"{"summary":"要約。","use_cases":[],"target_languages":[]}"#;

        // When
        let result = parse_insight(text);

        // Then
        assert_eq!(result.unwrap(), insight("要約。", &[], &[]));
    }

    #[test]
    fn test_parse_insight_blank_summary_is_empty_response() {
        // T7 (empty) and T8 (whitespace only)
        for summary in ["", "  \n "] {
            // Given
            let text = format!(
                r#"{{"summary":{},"use_cases":["A"],"target_languages":["Rust"]}}"#,
                serde_json::to_string(summary).unwrap()
            );

            // When
            let result = parse_insight(&text);

            // Then
            assert_ai_error_contains(result, "empty response");
        }
    }

    #[test]
    fn test_parse_insight_invalid_json_is_parse_error() {
        // T9 (not JSON), T10 (missing field), T11 (type mismatch)
        let cases = [
            "Rust製のCLIツールです。",
            r#"{"summary":"要約。","use_cases":[]}"#,
            r#"{"summary":"要約。","use_cases":"CLIツール","target_languages":[]}"#,
        ];
        for text in cases {
            // Given / When
            let result = parse_insight(text);

            // Then
            assert_ai_error_contains(result, "Failed to parse Gemini JSON response");
        }
    }

    #[test]
    fn test_parse_insight_ignores_extra_fields() {
        // Given
        let text = r#"{"summary":"要約。","use_cases":[],"target_languages":[],"extra":1}"#;

        // When
        let result = parse_insight(text);

        // Then
        assert_eq!(result.unwrap(), insight("要約。", &[], &[]));
    }

    // ---- parse_insight invariants (P1, P2, P4) ----

    #[test]
    fn test_parse_insight_invariants_p1_p2() {
        // Given: boundary inputs around whitespace and empty items
        let cases = [
            (" a ", vec![" x ", "", "y"], vec!["\tz\n"]),
            ("\n要約\n", vec![], vec![" ", ""]),
            ("s", vec!["1", "2", "3", "4", "5"], vec!["Rust", " Go "]),
        ];
        for (summary, use_cases, langs) in cases {
            let text = serde_json::json!({
                "summary": summary, "use_cases": use_cases, "target_languages": langs
            })
            .to_string();

            // When
            let result = parse_insight(&text).unwrap();

            // Then: P1 summary non-empty and trimmed
            assert!(!result.summary.is_empty() && result.summary == result.summary.trim());
            // P2: items trimmed, non-empty, order preserved
            let expect = |items: &[&str]| -> Vec<String> {
                items
                    .iter()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect()
            };
            assert_eq!(result.use_cases, expect(&use_cases));
            assert_eq!(result.target_languages, expect(&langs));
        }
    }

    #[test]
    fn test_parse_insight_fence_does_not_change_result_p4() {
        // Given: several raw JSON texts
        let cases = [
            INSIGHT_JSON,
            r#"{"summary":"要約。","use_cases":[],"target_languages":[]}"#,
            r#"{"summary":" a ","use_cases":[" b ",""],"target_languages":["c"]}"#,
        ];
        for json in cases {
            // When
            let plain = parse_insight(json).unwrap();
            let fenced = parse_insight(&format!("```json\n{}\n```", json)).unwrap();

            // Then
            assert_eq!(plain, fenced, "json: {}", json);
        }
    }

    // ---- GeminiClient::analyze (T13-T25, P3) ----

    #[tokio::test]
    async fn test_analyze_sends_structured_output_config() {
        // Given: a mock that matches only when schema/mime/thinking are sent
        let server = MockServer::start();
        let expected_config = serde_json::json!({
            "generationConfig": {
                "responseMimeType": "application/json",
                "responseJsonSchema": insight_response_schema(),
                "thinkingConfig": { "thinkingLevel": "low" }
            }
        });
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1beta/models/gemini-3.8-flash:generateContent")
                .json_body_includes(expected_config.to_string());
            then.status(200).json_body(serde_json::json!({
                "candidates": [ { "content": { "parts": [ { "text": INSIGHT_JSON } ] } } ]
            }));
        });
        let client = create_test_client(server.base_url());

        // When
        let result = client.analyze(&create_test_repository()).await;

        // Then
        mock.assert();
        assert_eq!(result.unwrap(), expected_insight());
    }

    #[tokio::test]
    async fn test_analyze_sends_prompt_and_key_with_single_call() {
        // Given
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1beta/models/gemini-3.8-flash:generateContent")
                .header("x-goog-api-key", "test-api-key")
                .body_includes("Name: rust-lang/rust");
            then.status(200).json_body(serde_json::json!({
                "candidates": [ { "content": { "parts": [ { "text": INSIGHT_JSON } ] } } ]
            }));
        });
        let client = create_test_client(server.base_url());

        // When
        let result = client.analyze(&create_test_repository()).await;

        // Then
        mock.assert_calls(1);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_analyze_excludes_thought_part_with_json_like_text() {
        // Given
        let (_server, client) = start_server_with_parts(serde_json::json!([
            { "text": "{\"summary\":\"思考中\",\"use_cases\":[],\"target_languages\":[]}", "thought": true },
            { "text": INSIGHT_JSON }
        ]));

        // When
        let result = client.analyze(&create_test_repository()).await;

        // Then
        assert_eq!(result.unwrap(), expected_insight());
    }

    #[tokio::test]
    async fn test_analyze_skips_signature_only_thought_part() {
        // Given
        let (_server, client) = start_server_with_parts(serde_json::json!([
            { "thought": true, "thoughtSignature": "AbC123" },
            { "text": INSIGHT_JSON }
        ]));

        // When
        let result = client.analyze(&create_test_repository()).await;

        // Then
        assert_eq!(result.unwrap(), expected_insight());
    }

    #[tokio::test]
    async fn test_analyze_concatenates_json_split_across_parts() {
        // Given
        let (_server, client) = start_server_with_parts(serde_json::json!([
            { "text": "{\"summary\":\"要約。\",\"use_cases\":[\"A\"]," },
            { "text": "\"target_languages\":[\"Rust\"]}" }
        ]));

        // When
        let result = client.analyze(&create_test_repository()).await;

        // Then
        assert_eq!(result.unwrap(), insight("要約。", &["A"], &["Rust"]));
    }

    #[tokio::test]
    async fn test_analyze_non_json_text_is_parse_error() {
        // Given
        let (_server, client) =
            start_server_with_parts(serde_json::json!([ { "text": "Rust製のCLIツールです。" } ]));

        // When
        let result = client.analyze(&create_test_repository()).await;

        // Then
        assert_ai_error_contains(result, "Failed to parse Gemini JSON response");
    }

    #[tokio::test]
    async fn test_analyze_only_thought_parts_is_empty_response() {
        // Given
        let (_server, client) = start_server_with_parts(serde_json::json!([
            { "text": "考え中", "thought": true }
        ]));

        // When
        let result = client.analyze(&create_test_repository()).await;

        // Then
        assert_ai_error_contains(result, "empty response");
    }

    #[tokio::test]
    async fn test_analyze_thought_false_is_regular_text() {
        // Given
        let (_server, client) = start_server_with_parts(serde_json::json!([
            { "text": INSIGHT_JSON, "thought": false }
        ]));

        // When
        let result = client.analyze(&create_test_repository()).await;

        // Then
        assert_eq!(result.unwrap(), expected_insight());
    }

    #[tokio::test]
    async fn test_analyze_empty_parts_is_empty_response() {
        // Given
        let (_server, client) = start_server_with_parts(serde_json::json!([]));

        // When
        let result = client.analyze(&create_test_repository()).await;

        // Then
        assert_ai_error_contains(result, "empty response");
    }

    #[tokio::test]
    async fn test_analyze_part_without_text_or_thought_is_empty_response() {
        // Given
        let (_server, client) = start_server_with_parts(serde_json::json!([
            { "thoughtSignature": "xyz" }
        ]));

        // When
        let result = client.analyze(&create_test_repository()).await;

        // Then
        assert_ai_error_contains(result, "empty response");
    }

    #[tokio::test]
    async fn test_analyze_empty_candidates_is_empty_response() {
        // Given
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST)
                .path("/v1beta/models/gemini-3.8-flash:generateContent");
            then.status(200)
                .json_body(serde_json::json!({ "candidates": [] }));
        });
        let client = create_test_client(server.base_url());

        // When
        let result = client.analyze(&create_test_repository()).await;

        // Then
        assert_ai_error_contains(result, "empty response");
    }

    #[tokio::test]
    async fn test_analyze_http_error_statuses() {
        // T21 (400/401/403), T22 (429), T23 (500)
        let cases = [
            (400u16, "API key not valid", "400"),
            (401, "unauthorized", "401"),
            (403, "API key not valid", "403"),
            (429, "Resource has been exhausted", "rate limit"),
            (500, "Internal error", "server error 500"),
        ];
        for (status, body, expected) in cases {
            // Given
            let (_server, client) = start_server_with_status(status, body);

            // When
            let result = client.analyze(&create_test_repository()).await;

            // Then
            assert_ai_error_contains(result, expected);
        }
    }

    #[tokio::test]
    async fn test_analyze_envelope_not_json_is_parse_error() {
        // Given
        let (_server, client) = start_server_with_status(200, "not a json");

        // When
        let result = client.analyze(&create_test_repository()).await;

        // Then: the envelope error, not the JSON body error
        assert_ai_error_contains(result, "Failed to parse Gemini response");
    }

    #[tokio::test]
    async fn test_analyze_empty_summary_is_empty_response() {
        // Given
        let (_server, client) = start_server_with_parts(serde_json::json!([
            { "text": "{\"summary\":\"\",\"use_cases\":[],\"target_languages\":[]}" }
        ]));

        // When
        let result = client.analyze(&create_test_repository()).await;

        // Then
        assert_ai_error_contains(result, "empty response");
    }

    #[tokio::test]
    async fn test_analyze_thought_position_invariant_p3() {
        // P3: the result is the same wherever the thought part sits.
        let thought = serde_json::json!({ "text": "漏れてはいけない思考", "thought": true });
        let body = serde_json::json!({ "text": INSIGHT_JSON });
        let cases = [
            serde_json::json!([thought, body]),
            serde_json::json!([body, thought]),
        ];
        // Also between two text parts of one JSON document.
        let split_at = INSIGHT_JSON.find("\"use_cases\"").unwrap();
        let (head, tail) = INSIGHT_JSON.split_at(split_at);
        let between = serde_json::json!([{ "text": head }, thought, { "text": tail }]);

        for parts in cases.into_iter().chain([between]) {
            // Given
            let (_server, client) = start_server_with_parts(parts.clone());

            // When
            let result = client.analyze(&create_test_repository()).await;

            // Then
            assert_eq!(result.unwrap(), expected_insight(), "parts: {}", parts);
        }
    }

    #[test]
    fn test_build_prompt_replaces_placeholder() {
        let repo = create_test_repository();
        let prompt = build_prompt("Analyze:\n{{repository}}\n", &repo);

        assert!(!prompt.contains("{{repository}}"));
        assert!(prompt.contains("Name: rust-lang/rust"));
        assert!(prompt.contains("URL: https://github.com/rust-lang/rust"));
        assert!(prompt.contains("Stars: 80000"));
        assert!(prompt.contains("Description: Empowering everyone"));
        assert!(prompt.contains("Primary Language: Rust"));
        assert!(prompt.contains("Topics: compiler, language"));
    }
}
