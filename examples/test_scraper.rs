/// Example: Test GitHub Trending scraper
///
/// This example demonstrates how to use the TrendingScraperClient
/// to fetch trending repositories from GitHub.
///
/// Usage:
///   cargo run --example test_scraper
///   cargo run --example test_scraper -- rust
///   cargo run --example test_scraper -- python
use github_trending_summary::domain::repositories::TrendingRepository;
use github_trending_summary::infrastructure::{init_logging, TrendingScraperClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    init_logging("info")?;

    // Get language filter from command line arguments
    let args: Vec<String> = std::env::args().collect();
    let language = if args.len() > 1 {
        Some(args[1].clone())
    } else {
        None
    };

    println!("==============================================");
    println!("  GitHub Trending Scraper Test");
    println!("==============================================");

    if let Some(ref lang) = language {
        println!("Language filter: {}", lang);
    } else {
        println!("Language filter: All languages");
    }
    println!();

    // Create scraper client
    println!("Creating scraper client...");
    let client = TrendingScraperClient::new()?;

    // Fetch trending repositories
    println!("Fetching trending repositories...");
    let repos = client.fetch_trending(language).await?;

    println!("Successfully fetched {} repositories!", repos.len());
    println!();

    // Display results
    println!("==============================================");
    println!("  Trending Repositories");
    println!("==============================================");
    println!();

    for (index, repo) in repos.iter().enumerate() {
        println!("{}. {}", index + 1, repo.full_name());
        println!("   URL: {}", repo.url());

        if let Some(desc) = repo.description() {
            println!("   Description: {}", desc);
        }

        if let Some(lang) = repo.language() {
            println!("   Language: {}", lang);
        }

        println!("   Stars today: {}", repo.stars());
        println!();
    }

    println!("==============================================");
    println!("Test completed successfully!");
    println!("==============================================");

    Ok(())
}
