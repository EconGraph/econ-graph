//! CLI interface for the crawler
//!
//! Provides command-line interface for crawler operations

use crate::database::create_pool;
use crate::error::AppResult;
use crate::services::crawler::{CatalogDownloader, SeriesDownloader};
use clap::{Parser, Subcommand};
use reqwest::Client;
use std::env;
use tokio;

#[derive(Parser)]
#[command(name = "crawler")]
#[command(about = "Economic data series crawler")]
#[command(version = "1.0")]
pub struct CrawlerCli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Download catalog for a data source
    Catalog {
        /// Name of the data source (e.g., "FRED", "BLS", "Census", "BEA", "World Bank", "IMF", "FHFA", "ECB", "OECD", "BoE", "WTO", "BoJ", "RBA", "BoC", "SNB", "UN Stats", "ILO")
        #[arg(short, long)]
        source: String,
    },
    /// Download specific series
    Series {
        /// Name of the data source
        #[arg(short, long)]
        source: String,
        /// Specific series ID to download
        #[arg(short, long)]
        series_id: String,
    },
    /// Download a random series from a data source
    Random {
        /// Name of the data source
        #[arg(short, long)]
        source: String,
    },
    /// List available data sources
    List,
}

impl CrawlerCli {
    /// Run the CLI application
    pub async fn run(self) -> AppResult<()> {
        // Initialize logging
        tracing_subscriber::fmt::init();

        // Get database URL from environment
        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgresql://postgres:password@localhost:5432/econ_graph_dev".to_string()
        });

        // Create database pool
        let pool = create_pool(&database_url).await?;
        let client = Client::new();
        let catalog_downloader = CatalogDownloader::new(client.clone());
        let series_downloader = SeriesDownloader::new(client);

        match self.command {
            Commands::Catalog { source } => {
                println!("Downloading catalog for data source: {}", source);
                let count = catalog_downloader.download_catalog(&pool, &source).await?;
                println!("✅ Discovered {} series", count);
            }
            Commands::Series { source, series_id } => {
                println!(
                    "Downloading series {} from data source: {}",
                    series_id, source
                );
                series_downloader
                    .download_specific_series(&pool, &source, &series_id)
                    .await?;
            }
            Commands::Random { source } => {
                println!("Downloading random series from data source: {}", source);
                series_downloader
                    .download_random_series(&pool, &source)
                    .await?;
            }
            Commands::List => {
                list_available_sources().await?;
            }
        }

        Ok(())
    }
}

/// List all available data sources
async fn list_available_sources() -> AppResult<()> {
    println!("📚 Available Data Sources:");
    println!();

    let sources = CatalogDownloader::get_available_sources();

    for (name, description) in sources {
        println!("  • {} - {}", name, description);
    }

    println!();
    println!("💡 Usage examples:");
    println!("  crawler catalog --source FRED");
    println!("  crawler series --source FRED --series-id GDP");
    println!("  crawler random --source BLS");

    Ok(())
}
