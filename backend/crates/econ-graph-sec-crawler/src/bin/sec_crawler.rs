use anyhow::Result;
use clap::{Parser, Subcommand};
use econ_graph_core::database::DatabasePool;
use econ_graph_sec_crawler::{CrawlConfig, SecEdgarCrawler};
use std::path::PathBuf;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// **SEC EDGAR Crawler CLI**
///
/// Command-line interface for crawling SEC EDGAR XBRL filings.
/// Supports crawling individual companies or batch operations.
#[derive(Parser)]
#[command(name = "sec-crawler")]
#[command(about = "SEC EDGAR XBRL filing crawler")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Crawl filings for a specific company
    CrawlCompany {
        /// Company CIK (Central Index Key)
        #[arg(short, long)]
        cik: String,

        /// Maximum requests per second
        #[arg(short, long, default_value = "10")]
        rate_limit: u32,

        /// Maximum file size to download (bytes)
        #[arg(short, long, default_value = "52428800")] // 50MB
        max_file_size: u64,

        /// Form types to include (comma-separated)
        #[arg(short, long)]
        form_types: Option<String>,

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
    },

    /// Get storage statistics
    Stats,

    /// Validate XBRL file
    Validate {
        /// Path to XBRL file
        #[arg(short, long)]
        file: PathBuf,
    },

    /// Parse XBRL file
    Parse {
        /// Path to XBRL file
        #[arg(short, long)]
        file: PathBuf,

        /// Output format (json, csv)
        #[arg(short, long, default_value = "json")]
        format: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sec_crawler=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    // Initialize database connection
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost/econ_graph".to_string());

    let pool = econ_graph_core::database::create_pool(&database_url).await?;
    let crawler = SecEdgarCrawler::new(pool).await?;

    match cli.command {
        Commands::CrawlCompany {
            cik,
            rate_limit,
            max_file_size,
            form_types,
            start_date,
            end_date,
            exclude_amended,
            exclude_restated,
        } => {
            crawl_company_command(
                crawler,
                cik,
                rate_limit,
                max_file_size,
                form_types,
                start_date,
                end_date,
                exclude_amended,
                exclude_restated,
            )
            .await?;
        }

        Commands::Stats => {
            stats_command(crawler).await?;
        }

        Commands::Validate { file } => {
            validate_command(file).await?;
        }

        Commands::Parse { file, format } => {
            parse_command(file, format).await?;
        }
    }

    Ok(())
}

async fn crawl_company_command(
    crawler: SecEdgarCrawler,
    cik: String,
    rate_limit: u32,
    max_file_size: u64,
    form_types: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    exclude_amended: bool,
    exclude_restated: bool,
) -> Result<()> {
    info!("Starting crawl for company CIK: {}", cik);

    // Parse form types
    let form_types_vec: Vec<String> = form_types
        .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    // Parse dates
    let start_date_parsed = if let Some(date_str) = start_date {
        Some(chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")?)
    } else {
        None
    };

    let end_date_parsed = if let Some(date_str) = end_date {
        Some(chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")?)
    } else {
        None
    };

    // Create crawl configuration
    let config = CrawlConfig {
        max_requests_per_second: rate_limit,
        max_retries: 3,
        retry_delay_seconds: 5,
        max_file_size_bytes: max_file_size,
        start_date: start_date_parsed,
        end_date: end_date_parsed,
        form_types: if form_types_vec.is_empty() {
            None
        } else {
            Some(form_types_vec)
        },
        exclude_amended,
        exclude_restated,
        user_agent: "EconGraph-SEC-Crawler/1.0".to_string(),
    };

    // Create crawler with custom config
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost/econ_graph".to_string());
    let pool = econ_graph_core::database::create_pool(&database_url).await?;
    let crawler = SecEdgarCrawler::with_config(pool, config).await?;

    // Execute crawl
    let result = crawler.crawl_company_filings(&cik).await?;

    // Print results
    println!("Crawl Results:");
    println!("  Operation ID: {}", result.operation_id);
    println!("  Company CIK: {}", cik);
    println!("  Total filings found: {}", result.total_filings_found);
    println!("  Filings downloaded: {}", result.filings_downloaded);
    println!("  Filings failed: {}", result.filings_failed);
    println!(
        "  Total bytes downloaded: {}",
        result.total_bytes_downloaded
    );
    println!("  Success: {}", result.success);

    if !result.errors.is_empty() {
        println!("  Errors:");
        for error in &result.errors {
            println!("    - {}", error);
        }
    }

    if let Some(duration) = result.end_time.map(|end| end - result.start_time) {
        println!("  Duration: {:?}", duration);
    }

    Ok(())
}

async fn stats_command(crawler: SecEdgarCrawler) -> Result<()> {
    info!("Getting storage statistics");

    let stats = crawler.get_storage_stats().await?;

    println!("Storage Statistics:");
    println!("  Total files: {}", stats.total_files);
    println!("  Total size: {} bytes", stats.total_size_bytes);
    println!("  Large object files: {}", stats.large_object_files);
    println!("  Bytea files: {}", stats.bytea_files);
    println!("  Compressed files: {}", stats.compressed_files);
    println!("  Uncompressed files: {}", stats.uncompressed_files);

    Ok(())
}

async fn validate_command(file: PathBuf) -> Result<()> {
    info!("Validating XBRL file: {:?}", file);

    use econ_graph_sec_crawler::XbrlParser;

    let parser = XbrlParser::new().await?;
    let report = parser.validate_xbrl_document(&file).await?;

    println!("Validation Results:");
    println!("  Valid: {}", report.is_valid);

    if !report.errors.is_empty() {
        println!("  Errors:");
        for error in &report.errors {
            println!("    - {}", error);
        }
    }

    if !report.warnings.is_empty() {
        println!("  Warnings:");
        for warning in &report.warnings {
            println!("    - {}", warning);
        }
    }

    Ok(())
}

async fn parse_command(file: PathBuf, format: String) -> Result<()> {
    info!("Parsing XBRL file: {:?}", file);

    use econ_graph_sec_crawler::XbrlParser;

    let parser = XbrlParser::new().await?;
    let statements = parser.parse_xbrl_document(&file).await?;

    match format.as_str() {
        "json" => {
            let json = serde_json::to_string_pretty(&statements)?;
            println!("{}", json);
        }
        "csv" => {
            // Simple CSV output
            println!("id,company_id,filing_type,period_end_date,fiscal_year,fiscal_quarter");
            for statement in &statements.statements {
                println!(
                    "{},{},{},{},{},{}",
                    statement.id,
                    statement.company_id,
                    statement.filing_type,
                    statement.period_end_date,
                    statement.fiscal_year,
                    statement.fiscal_quarter.unwrap_or(0)
                );
            }
        }
        _ => {
            return Err(anyhow::anyhow!("Unsupported format: {}", format));
        }
    }

    Ok(())
}
