use anyhow::Result;
use clap::Parser;
use econ_graph_sec_crawler::{CrawlConfig, SecEdgarCrawler};
use econ_graph_services::database::DatabasePool;
use std::collections::HashMap;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// **SEC Company Crawler**
///
/// Specialized crawler for crawling multiple companies with batch processing.
/// Supports configuration files and parallel processing.
#[derive(Parser)]
#[command(name = "sec-company-crawler")]
#[command(about = "Batch SEC EDGAR company crawler")]
#[command(version)]
struct Cli {
    /// Company CIKs to crawl (comma-separated)
    #[arg(short, long)]
    ciks: String,

    /// Maximum requests per second
    #[arg(short, long, default_value = "10")]
    rate_limit: u32,

    /// Maximum file size to download (bytes)
    #[arg(short, long, default_value = "52428800")] // 50MB
    max_file_size: u64,

    /// Form types to include (comma-separated)
    #[arg(short, long, default_value = "10-K,10-Q")]
    form_types: String,

    /// Start date for filing search (YYYY-MM-DD)
    #[arg(short, long)]
    start_date: Option<String>,

    /// End date for filing search (YYYY-MM-DD)
    #[arg(short, long)]
    end_date: Option<String>,

    /// Exclude amended filings
    #[arg(long)]
    exclude_amended: bool,

    /// Exclude restated filings
    #[arg(long)]
    exclude_restated: bool,

    /// Maximum number of concurrent crawls
    #[arg(short, long, default_value = "3")]
    max_concurrent: usize,

    /// Output results to file
    #[arg(short, long)]
    output: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sec_company_crawler=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    // Parse CIKs
    let ciks: Vec<String> = cli
        .ciks
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if ciks.is_empty() {
        return Err(anyhow::anyhow!("No CIKs provided"));
    }

    info!("Starting batch crawl for {} companies", ciks.len());

    // Parse form types
    let form_types: Vec<String> = cli
        .form_types
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    // Parse dates
    let start_date = if let Some(date_str) = cli.start_date {
        Some(chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")?)
    } else {
        None
    };

    let end_date = if let Some(date_str) = cli.end_date {
        Some(chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")?)
    } else {
        None
    };

    // Create crawl configuration
    let config = CrawlConfig {
        max_requests_per_second: cli.rate_limit,
        max_retries: 3,
        retry_delay_seconds: 5,
        max_file_size_bytes: cli.max_file_size,
        start_date,
        end_date,
        form_types: Some(form_types),
        exclude_amended: cli.exclude_amended,
        exclude_restated: cli.exclude_restated,
        user_agent: "EconGraph-SEC-Company-Crawler/1.0".to_string(),
    };

    // Initialize database connection
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost/econ_graph".to_string());

    let pool = DatabasePool::new(&database_url).await?;
    let crawler = SecEdgarCrawler::with_config(pool, config).await?;

    // Execute batch crawl
    let results = batch_crawl_companies(crawler, ciks, cli.max_concurrent).await?;

    // Print summary
    print_summary(&results);

    // Save results to file if requested
    if let Some(output_file) = cli.output {
        save_results_to_file(&results, &output_file).await?;
        info!("Results saved to: {}", output_file);
    }

    Ok(())
}

async fn batch_crawl_companies(
    crawler: SecEdgarCrawler,
    ciks: Vec<String>,
    max_concurrent: usize,
) -> Result<HashMap<String, econ_graph_sec_crawler::CrawlResult>> {
    use futures::stream::{self, StreamExt};

    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(max_concurrent));
    let mut results = HashMap::new();

    let crawl_futures = ciks.into_iter().map(|cik| {
        let crawler = &crawler;
        let semaphore = semaphore.clone();

        async move {
            let _permit = semaphore.acquire().await.unwrap();
            info!("Starting crawl for CIK: {}", cik);

            match crawler.crawl_company_filings(&cik).await {
                Ok(result) => {
                    info!(
                        "Completed crawl for CIK {}: {} downloaded, {} failed",
                        cik, result.filings_downloaded, result.filings_failed
                    );
                    Ok((cik, result))
                }
                Err(e) => {
                    error!("Failed to crawl CIK {}: {}", cik, e);
                    Err((cik, e))
                }
            }
        }
    });

    let crawl_results: Vec<_> = stream::iter(crawl_futures)
        .buffer_unordered(max_concurrent)
        .collect()
        .await;

    for result in crawl_results {
        match result {
            Ok((cik, crawl_result)) => {
                results.insert(cik, crawl_result);
            }
            Err((cik, error)) => {
                warn!("Failed to crawl company {}: {}", cik, error);
                // Create a failed result
                let failed_result = econ_graph_sec_crawler::CrawlResult {
                    operation_id: uuid::Uuid::new_v4(),
                    company_cik: Some(cik.clone()),
                    operation_type: "company_filings".to_string(),
                    start_time: chrono::Utc::now(),
                    end_time: Some(chrono::Utc::now()),
                    total_filings_found: 0,
                    filings_downloaded: 0,
                    filings_failed: 0,
                    total_bytes_downloaded: 0,
                    errors: vec![error.to_string()],
                    success: false,
                };
                results.insert(cik, failed_result);
            }
        }
    }

    Ok(results)
}

fn print_summary(results: &HashMap<String, econ_graph_sec_crawler::CrawlResult>) {
    let total_companies = results.len();
    let successful_companies = results.values().filter(|r| r.success).count();
    let failed_companies = total_companies - successful_companies;

    let total_filings_found: u32 = results.values().map(|r| r.total_filings_found).sum();
    let total_filings_downloaded: u32 = results.values().map(|r| r.filings_downloaded).sum();
    let total_filings_failed: u32 = results.values().map(|r| r.filings_failed).sum();
    let total_bytes_downloaded: u64 = results.values().map(|r| r.total_bytes_downloaded).sum();

    println!("\n=== BATCH CRAWL SUMMARY ===");
    println!("Total companies: {}", total_companies);
    println!("Successful companies: {}", successful_companies);
    println!("Failed companies: {}", failed_companies);
    println!("Total filings found: {}", total_filings_found);
    println!("Total filings downloaded: {}", total_filings_downloaded);
    println!("Total filings failed: {}", total_filings_failed);
    println!("Total bytes downloaded: {}", total_bytes_downloaded);

    if failed_companies > 0 {
        println!("\n=== FAILED COMPANIES ===");
        for (cik, result) in results.iter().filter(|(_, r)| !r.success) {
            println!("CIK {}: {}", cik, result.errors.join(", "));
        }
    }

    println!("\n=== COMPANY DETAILS ===");
    for (cik, result) in results {
        println!(
            "CIK {}: {} found, {} downloaded, {} failed",
            cik, result.total_filings_found, result.filings_downloaded, result.filings_failed
        );
    }
}

async fn save_results_to_file(
    results: &HashMap<String, econ_graph_sec_crawler::CrawlResult>,
    output_file: &str,
) -> Result<()> {
    let json = serde_json::to_string_pretty(results)?;
    tokio::fs::write(output_file, json).await?;
    Ok(())
}
