//! Catalog Crawler Binary
//!
//! This binary crawls all data source catalogs, populates the series metadata table,
//! and downloads a small number of series from each source for initial data population.

use clap::{Parser, Subcommand};
use econ_graph_backend::database::{create_pool, DatabasePool};
use econ_graph_backend::models::data_source::DataSource;
use econ_graph_backend::services::crawler::catalog_downloader::CatalogDownloader;
use econ_graph_backend::services::crawler::series_downloader::SeriesDownloader;
use econ_graph_backend::services::series_discovery::SeriesDiscoveryService;
use reqwest::Client;
use std::collections::HashMap;
use tracing::{error, info, warn};

/// Catalog crawler for populating data sources with real catalog data
#[derive(Parser)]
#[command(name = "catalog_crawler")]
#[command(about = "Crawl all data source catalogs and populate series metadata")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Crawl all data source catalogs and populate series metadata
    CrawlAll {
        /// Number of series to download from each source (default: 5)
        #[arg(long, default_value = "5")]
        series_count: usize,

        /// Database URL
        #[arg(long)]
        database_url: String,

        /// API keys for data sources (format: SOURCE_NAME=API_KEY)
        #[arg(long, num_args = 0..)]
        api_key: Vec<String>,

        /// Dry run - don't actually download data
        #[arg(long)]
        dry_run: bool,

        /// Skip downloading actual series data
        #[arg(long)]
        skip_data_download: bool,
    },

    /// Crawl a specific data source
    CrawlSource {
        /// Data source name to crawl
        source_name: String,

        /// Number of series to download (default: 5)
        #[arg(long, default_value = "5")]
        series_count: usize,

        /// Database URL
        #[arg(long)]
        database_url: String,

        /// API key for the data source
        #[arg(long)]
        api_key: Option<String>,

        /// Dry run - don't actually download data
        #[arg(long)]
        dry_run: bool,

        /// Skip downloading actual series data
        #[arg(long)]
        skip_data_download: bool,
    },

    /// Export catalog data to SQL migration
    ExportCatalog {
        /// Database URL
        #[arg(long)]
        database_url: String,

        /// Output file path
        #[arg(long, default_value = "catalog_migration.sql")]
        output_file: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::CrawlAll {
            series_count,
            database_url,
            api_key,
            dry_run,
            skip_data_download,
        } => {
            crawl_all_sources(
                database_url,
                api_key,
                series_count,
                dry_run,
                skip_data_download,
            )
            .await
        }
        Commands::CrawlSource {
            source_name,
            series_count,
            database_url,
            api_key,
            dry_run,
            skip_data_download,
        } => {
            crawl_single_source(
                source_name,
                database_url,
                api_key,
                series_count,
                dry_run,
                skip_data_download,
            )
            .await
        }
        Commands::ExportCatalog {
            database_url,
            output_file,
        } => export_catalog_to_sql(database_url, output_file).await,
    }
}

/// Crawl all available data sources
async fn crawl_all_sources(
    database_url: String,
    api_keys: Vec<String>,
    series_count: usize,
    dry_run: bool,
    skip_data_download: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting catalog crawl for all data sources");

    // Parse API keys
    let api_key_map = parse_api_keys(api_keys);

    // Create database pool
    let pool = create_pool(&database_url).await?;

    // Create HTTP client
    let client = Client::new();

    // Create services
    let discovery_service = SeriesDiscoveryService::new(None, None, None, None);
    let catalog_downloader = CatalogDownloader::new(client.clone());
    let series_downloader = SeriesDownloader::new(client);

    // Get all data sources
    let data_sources = DataSource::find_all(&pool).await?;
    info!("Found {} data sources to crawl", data_sources.len());

    let mut total_series_discovered = 0;
    let mut total_series_downloaded = 0;

    for data_source in data_sources {
        if !data_source.is_enabled {
            info!("Skipping disabled data source: {}", data_source.name);
            continue;
        }

        info!("Crawling data source: {}", data_source.name);

        // Get API key for this source
        let api_key = api_key_map.get(&data_source.name.to_uppercase());

        match crawl_data_source(
            &data_source,
            &pool,
            &discovery_service,
            &catalog_downloader,
            &series_downloader,
            api_key.map(|k| k.as_str()),
            series_count,
            dry_run,
            skip_data_download,
        )
        .await
        {
            Ok((discovered, downloaded)) => {
                total_series_discovered += discovered;
                total_series_downloaded += downloaded;
                info!(
                    "Completed {}: discovered {}, downloaded {}",
                    data_source.name, discovered, downloaded
                );
            }
            Err(e) => {
                error!("Failed to crawl {}: {}", data_source.name, e);
                // Continue with other sources
            }
        }

        // Add delay between sources to be respectful
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }

    info!(
        "Catalog crawl completed: {} series discovered, {} series downloaded",
        total_series_discovered, total_series_downloaded
    );

    Ok(())
}

/// Crawl a single data source
async fn crawl_single_source(
    source_name: String,
    database_url: String,
    api_key: Option<String>,
    series_count: usize,
    dry_run: bool,
    skip_data_download: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting catalog crawl for data source: {}", source_name);

    // Create database pool
    let pool = create_pool(&database_url).await?;

    // Find the data source
    let data_source = DataSource::find_by_name(&pool, &source_name)
        .await?
        .ok_or_else(|| format!("Data source '{}' not found", source_name))?;

    if !data_source.is_enabled {
        return Err(format!("Data source '{}' is disabled", source_name).into());
    }

    // Create HTTP client
    let client = Client::new();

    // Create services
    let discovery_service = SeriesDiscoveryService::new(api_key.clone(), None, None, None);
    let catalog_downloader = CatalogDownloader::new(client.clone());
    let series_downloader = SeriesDownloader::new(client);

    // Crawl the data source
    let (discovered, downloaded) = crawl_data_source(
        &data_source,
        &pool,
        &discovery_service,
        &catalog_downloader,
        &series_downloader,
        api_key.as_deref(),
        series_count,
        dry_run,
        skip_data_download,
    )
    .await?;

    info!(
        "Completed {}: discovered {}, downloaded {}",
        data_source.name, discovered, downloaded
    );

    Ok(())
}

/// Crawl a specific data source
async fn crawl_data_source(
    data_source: &DataSource,
    pool: &DatabasePool,
    discovery_service: &SeriesDiscoveryService,
    catalog_downloader: &CatalogDownloader,
    series_downloader: &SeriesDownloader,
    api_key: Option<&str>,
    series_count: usize,
    dry_run: bool,
    skip_data_download: bool,
) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    info!("Discovering series for {}", data_source.name);

    // Discover series using the appropriate method
    let discovered_series = match data_source.name.to_lowercase().as_str() {
        "fred" => discovery_service.discover_fred_series(pool).await?,
        "bls" => discovery_service.discover_bls_series(pool).await?,
        "census" => discovery_service.discover_census_series(pool).await?,
        "bea" => discovery_service.discover_bea_series(pool).await?,
        "world bank" => discovery_service.discover_world_bank_series(pool).await?,
        "imf" => discovery_service.discover_imf_series(pool).await?,
        "fhfa" => discovery_service.discover_fhfa_series(pool).await?,
        "ecb" => discovery_service.discover_ecb_series(pool).await?,
        "oecd" => discovery_service.discover_oecd_series(pool).await?,
        "bank of england" | "boe" => discovery_service.discover_boe_series(pool).await?,
        "wto" => discovery_service.discover_wto_series(pool).await?,
        "bank of japan" | "boj" => discovery_service.discover_boj_series(pool).await?,
        "reserve bank of australia" | "rba" => discovery_service.discover_rba_series(pool).await?,
        "bank of canada" | "boc" => discovery_service.discover_boc_series(pool).await?,
        "swiss national bank" | "snb" => discovery_service.discover_snb_series(pool).await?,
        "un statistics division" | "unstats" => {
            discovery_service.discover_unstats_series(pool).await?
        }
        "ilo" => discovery_service.discover_ilo_series(pool).await?,
        _ => {
            warn!("Unknown data source: {}", data_source.name);
            vec![]
        }
    };

    info!(
        "Discovered {} series for {}",
        discovered_series.len(),
        data_source.name
    );

    if dry_run {
        info!("Dry run mode - skipping data download");
        return Ok((discovered_series.len(), 0));
    }

    if skip_data_download {
        info!("Skipping data download as requested");
        return Ok((discovered_series.len(), 0));
    }

    // Download a subset of series
    let series_to_download = discovered_series
        .iter()
        .take(series_count)
        .map(|s| s.as_str())
        .collect::<Vec<_>>();

    info!(
        "Downloading {} series for {}",
        series_to_download.len(),
        data_source.name
    );

    let mut downloaded_count = 0;
    for series_id in series_to_download {
        match series_downloader
            .download_specific_series(pool, &data_source.name, series_id)
            .await
        {
            Ok(_) => {
                downloaded_count += 1;
                info!("Downloaded series: {}", series_id);
            }
            Err(e) => {
                warn!("Failed to download series {}: {}", series_id, e);
            }
        }

        // Add delay between downloads to be respectful
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    Ok((discovered_series.len(), downloaded_count))
}

/// Parse API keys from command line arguments
fn parse_api_keys(api_keys: Vec<String>) -> HashMap<String, String> {
    let mut map = HashMap::new();

    for key_arg in api_keys {
        if let Some((key, value)) = key_arg.split_once('=') {
            map.insert(key.to_uppercase(), value.to_string());
        }
    }

    map
}

/// Export catalog data to SQL migration file
async fn export_catalog_to_sql(
    database_url: String,
    output_file: String,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Exporting catalog data to {}", output_file);

    // Create database pool
    let pool = create_pool(&database_url).await?;

    // Get all series metadata
    let series_metadata =
        econ_graph_backend::models::series_metadata::SeriesMetadata::find_all(&pool).await?;

    // Generate SQL migration
    let mut sql = String::new();
    sql.push_str("-- Auto-generated catalog migration\n");
    sql.push_str("-- Generated from real data source catalogs\n\n");

    // Insert series metadata
    sql.push_str("-- Insert series metadata\n");
    for metadata in &series_metadata {
        sql.push_str(&format!(
            "INSERT INTO series_metadata (id, source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, last_discovered_at, is_active, created_at, updated_at) VALUES ('{}', '{}', '{}', '{}', {}, {}, {}, {}, {}, {}, {}, {}, '{}', '{}');\n",
            metadata.id,
            metadata.source_id,
            metadata.external_id,
            metadata.title.replace("'", "''"),
            metadata.description.as_ref().map(|d| format!("'{}'", d.replace("'", "''"))).unwrap_or_else(|| "NULL".to_string()),
            metadata.units.as_ref().map(|u| format!("'{}'", u.replace("'", "''"))).unwrap_or_else(|| "NULL".to_string()),
            metadata.frequency.as_ref().map(|f| format!("'{}'", f.replace("'", "''"))).unwrap_or_else(|| "NULL".to_string()),
            metadata.geographic_level.as_ref().map(|g| format!("'{}'", g.replace("'", "''"))).unwrap_or_else(|| "NULL".to_string()),
            metadata.data_url.as_ref().map(|u| format!("'{}'", u.replace("'", "''"))).unwrap_or_else(|| "NULL".to_string()),
            metadata.api_endpoint.as_ref().map(|e| format!("'{}'", e.replace("'", "''"))).unwrap_or_else(|| "NULL".to_string()),
            metadata.last_discovered_at.map(|t| t.to_rfc3339()).unwrap_or_else(|| "NULL".to_string()),
            metadata.is_active,
            metadata.created_at.map(|t| t.to_rfc3339()).unwrap_or_else(|| "NULL".to_string()),
            metadata.updated_at.map(|t| t.to_rfc3339()).unwrap_or_else(|| "NULL".to_string())
        ));
    }

    // Write to file
    std::fs::write(&output_file, sql)?;

    info!(
        "Exported {} series metadata entries to {}",
        series_metadata.len(),
        output_file
    );

    Ok(())
}
