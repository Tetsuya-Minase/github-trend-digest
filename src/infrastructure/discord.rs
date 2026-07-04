/// Discord Webhook notification sender implementation.
use async_trait::async_trait;
use reqwest::Client;
use tracing::{debug, error, info};

use crate::domain::{AppError, AppResult, NotificationSender, TrendingSummary};

/// Client for sending notifications to Discord via Webhook.
pub struct DiscordNotificationSender {
    webhook_url: String,
    client: Client,
}

impl DiscordNotificationSender {
    /// Creates a new DiscordNotificationSender instance.
    pub fn new(webhook_url: String) -> Self {
        Self {
            webhook_url,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl NotificationSender for DiscordNotificationSender {
    async fn send(&self, summary: &TrendingSummary) -> AppResult<()> {
        let content = summary.format_markdown();
        info!("Sending notification to Discord for {}", summary.repository().full_name());

        // Discord limit is 2000 characters
        let content = if content.len() > 2000 {
            let mut truncated = content[..1997].to_string();
            truncated.push_str("...");
            truncated
        } else {
            content
        };

        let payload = serde_json::json!({
            "content": content
        });

        let response = self.client.post(&self.webhook_url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                error!("Network error sending Discord notification: {}", e);
                AppError::NetworkError(e.to_string())
            })?;

        let status = response.status();
        if !status.is_success() {
            let error_msg = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            error!("Discord API error: status={}, body={}", status, error_msg);
            return Err(AppError::NotificationError(format!(
                "Discord API error {}: {}",
                status.as_u16(),
                error_msg
            )));
        }

        debug!("Discord notification sent successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Repository;
    use httpmock::prelude::*;

    #[tokio::test]
    async fn test_discord_send_success() {
        let server = MockServer::start();

        // We expect a POST request to our mock server with JSON containing the content
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("content-type", "application/json");
            then.status(204);
        });

        let sender = DiscordNotificationSender {
            webhook_url: server.base_url(),
            client: Client::new(),
        };

        let repo = Repository::new(
            "rust".to_string(),
            "rust-lang".to_string(),
            "https://github.com/rust-lang/rust".to_string(),
            Some("A language empowering everyone".to_string()),
            80000,
            Some("Rust".to_string()),
            vec![],
        )
        .unwrap();

        let summary = TrendingSummary::new(
            repo,
            "A systems programming language.".to_string(),
            vec![],
            vec![],
        )
        .unwrap();

        let result = sender.send(&summary).await;

        // Verify that the mock server received the request
        mock.assert();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_discord_send_api_error() {
        let server = MockServer::start();

        // Configure mock response to return 400 Bad Request
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("content-type", "application/json");
            then.status(400)
                .body("Bad Request");
        });

        let sender = DiscordNotificationSender {
            webhook_url: server.base_url(),
            client: Client::new(),
        };

        let repo = Repository::new(
            "rust".to_string(),
            "rust-lang".to_string(),
            "https://github.com/rust-lang/rust".to_string(),
            None,
            80000,
            None,
            vec![],
        )
        .unwrap();

        let summary = TrendingSummary::new(
            repo,
            "Summary".to_string(),
            vec![],
            vec![],
        )
        .unwrap();

        let result = sender.send(&summary).await;

        mock.assert();
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::NotificationError(msg) => {
                assert!(msg.contains("400"));
                assert!(msg.contains("Bad Request"));
            }
            _ => panic!("Expected NotificationError"),
        }
    }

    #[tokio::test]
    async fn test_discord_send_truncated() {
        let server = MockServer::start();

        // We expect the payload content to be truncated to 2000 characters (including ...)
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("content-type", "application/json")
                .is_true(|req| {
                    let body_bytes = req.body();
                    if let Ok(body) = serde_json::from_slice::<serde_json::Value>(body_bytes.as_ref()) {
                        if let Some(content) = body.get("content").and_then(|c| c.as_str()) {
                            return content.len() == 2000 && content.ends_with("...");
                        }
                    }
                    false
                });
            then.status(204);
        });

        let sender = DiscordNotificationSender {
            webhook_url: server.base_url(),
            client: Client::new(),
        };

        let repo = Repository::new(
            "rust".to_string(),
            "rust-lang".to_string(),
            "https://github.com/rust-lang/rust".to_string(),
            None,
            80000,
            None,
            vec![],
        )
        .unwrap();

        // Create a very long AI summary (more than 2000 characters)
        let long_summary_text = "A".repeat(2500);

        let summary = TrendingSummary::new(
            repo,
            long_summary_text,
            vec![],
            vec![],
        )
        .unwrap();

        let result = sender.send(&summary).await;

        mock.assert();
        assert!(result.is_ok());
    }
}
