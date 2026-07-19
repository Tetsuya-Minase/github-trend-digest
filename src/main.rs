//! Main entry point for the GitHub Trending Summary application.
use std::process::ExitCode;
use std::sync::Arc;

use tracing::{error, info, warn};

use github_trending_summary::{
    init_logging, AppConfig, AppResult, DiscordNotificationSender,
    FetchAndSummarizeTrendingUseCase, GeminiClient, GithubApiClient, GithubApiClientImpl,
    TrendingScraperClient,
};

#[tokio::main]
async fn main() -> ExitCode {
    // Load configuration from environment (before logging is available)
    let config = match AppConfig::from_env() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Configuration error: {e}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(e) = init_logging(config.log_level()) {
        eprintln!("Failed to initialize logging: {e}");
        return ExitCode::FAILURE;
    }

    info!("GitHub Trending Summary application started");

    let language = resolve_language(std::env::args().nth(1), config.target_language());
    info!("Language filter: {:?}", language);

    match run(&config, language).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            error!("Application failed: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Builds the dependency graph and executes the main use case.
async fn run(config: &AppConfig, language: Option<String>) -> AppResult<()> {
    let scraper = Arc::new(TrendingScraperClient::new()?);
    let github_client = Arc::new(GithubApiClientImpl::new(
        config.github_token().map(String::from),
    )?);
    let gemini_client = Arc::new(GeminiClient::new(
        config.gemini_api_key().to_string(),
        config.summarizer().clone(),
    )?);
    let notification_sender = Arc::new(DiscordNotificationSender::new(
        config.discord_webhook_url().to_string(),
    )?);

    let use_case = FetchAndSummarizeTrendingUseCase::new(
        scraper,
        github_client.clone(),
        gemini_client,
        notification_sender,
    );

    // Fail fast on misconfigured tokens; skip the check when unauthenticated
    if config.github_token().is_some() {
        github_client.verify_credentials().await?;
        info!("GitHub credentials verified");
    } else {
        info!("GITHUB_TOKEN not set; using unauthenticated GitHub API access");
    }

    let summaries = use_case.execute(language).await?;

    if summaries.is_empty() {
        warn!("No summaries were generated");
    } else {
        info!(
            "Successfully generated and sent {} summaries",
            summaries.len()
        );
    }

    Ok(())
}

/// Resolves the language filter with priority: CLI argument > config value.
///
/// Values are trimmed; an empty result is treated as `None` (all languages).
fn resolve_language(cli_arg: Option<String>, config_lang: Option<&str>) -> Option<String> {
    cli_arg
        .as_deref()
        .or(config_lang)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_language_cli_arg_wins() {
        // Given: CLI引数とconfigの両方に言語が設定されている
        let cli_arg = Some("rust".to_string());
        let config_lang = Some("go");

        // When: 言語を解決する
        let result = resolve_language(cli_arg, config_lang);

        // Then: CLI引数が優先される
        assert_eq!(result, Some("rust".to_string()));
    }

    #[test]
    fn test_resolve_language_falls_back_to_config() {
        // Given: CLI引数がなくconfigに言語が設定されている
        let cli_arg = None;
        let config_lang = Some("go");

        // When: 言語を解決する
        let result = resolve_language(cli_arg, config_lang);

        // Then: configの値にフォールバックする
        assert_eq!(result, Some("go".to_string()));
    }

    #[test]
    fn test_resolve_language_empty_string_is_none() {
        // Given: CLI引数もconfigも空文字列
        let cli_arg = Some("".to_string());
        let config_lang = Some("");

        // When: 言語を解決する
        let result = resolve_language(cli_arg, config_lang);

        // Then: Noneとして扱う（全言語トレンド）
        assert_eq!(result, None);
    }
}
