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

        /// Maximum number of rows per bulk INSERT statement
        #[arg(long, default_value = "1000")]
        bulk_insert_limit: usize,
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
            bulk_insert_limit,
        } => export_catalog_to_sql(database_url, output_file, bulk_insert_limit).await,
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
        "u.s. census bureau" | "census" => discovery_service.discover_census_series(pool).await?,
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
    bulk_insert_limit: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        "Exporting catalog data to {} with bulk insert limit of {}",
        output_file, bulk_insert_limit
    );

    // Create database pool
    let pool = create_pool(&database_url).await?;

    // Get all data to export
    let data_sources = DataSource::find_all(&pool).await?;
    let series_metadata =
        econ_graph_backend::models::series_metadata::SeriesMetadata::find_all(&pool).await?;
    let economic_series =
        econ_graph_backend::models::economic_series::EconomicSeries::find_all(&pool).await?;
    let data_points = econ_graph_backend::models::data_point::DataPoint::find_all(&pool).await?;
    let crawl_attempts =
        econ_graph_backend::models::crawl_attempt::CrawlAttempt::find_all(&pool).await?;

    // Generate SQL migration
    let mut sql = String::new();
    sql.push_str("-- Auto-generated catalog migration from crawler integration test\n");
    sql.push_str("-- Generated from real data source catalogs and series data\n");
    sql.push_str(&format!(
        "-- Generated at: {}\n\n",
        chrono::Utc::now().to_rfc3339()
    ));

    // Helper function to escape SQL strings
    fn escape_sql_string(s: &str) -> String {
        s.replace("'", "''")
    }

    // Helper function to format nullable string
    fn format_nullable_string(s: &Option<String>) -> String {
        s.as_ref()
            .map(|s| format!("'{}'", escape_sql_string(s)))
            .unwrap_or_else(|| "NULL".to_string())
    }

    // Helper function to format nullable timestamp
    fn format_nullable_timestamp(t: &Option<chrono::DateTime<chrono::Utc>>) -> String {
        t.map(|t| format!("'{}'", t.to_rfc3339()))
            .unwrap_or_else(|| "NULL".to_string())
    }

    // Helper function to format nullable date
    fn format_nullable_date(d: &Option<chrono::NaiveDate>) -> String {
        d.map(|d| format!("'{}'", d))
            .unwrap_or_else(|| "NULL".to_string())
    }

    // Helper function to format required timestamp
    fn format_required_timestamp(t: &chrono::DateTime<chrono::Utc>) -> String {
        format!("'{}'", t.to_rfc3339())
    }

    // Helper function to create bulk INSERT statements
    fn create_bulk_insert<T, F>(
        table_name: &str,
        columns: &[&str],
        items: &[T],
        limit: usize,
        row_formatter: F,
    ) -> String
    where
        F: Fn(&T) -> String,
    {
        if items.is_empty() {
            return String::new();
        }

        let mut result = String::new();
        let columns_str = columns.join(", ");

        for chunk in items.chunks(limit) {
            result.push_str(&format!(
                "INSERT INTO {} ({}) VALUES\n",
                table_name, columns_str
            ));

            let values: Vec<String> = chunk.iter().map(&row_formatter).collect();
            result.push_str(&values.join(",\n"));
            result.push_str(";\n\n");
        }

        result
    }

    // Export data sources
    if !data_sources.is_empty() {
        sql.push_str("-- Data Sources\n");
        let data_source_insert = create_bulk_insert(
            "data_sources",
            &[
                "id",
                "name",
                "description",
                "base_url",
                "api_key_required",
                "rate_limit_per_minute",
                "created_at",
                "updated_at",
                "is_visible",
                "is_enabled",
                "requires_admin_approval",
                "crawl_frequency_hours",
                "last_crawl_at",
                "crawl_status",
                "crawl_error_message",
                "api_documentation_url",
            ],
            &data_sources,
            bulk_insert_limit,
            |ds| {
                format!(
                    "('{}', '{}', {}, '{}', {}, {}, '{}', '{}', {}, {}, {}, {}, {}, {}, {}, {})",
                    ds.id,
                    escape_sql_string(&ds.name),
                    format_nullable_string(&ds.description),
                    escape_sql_string(&ds.base_url),
                    ds.api_key_required,
                    ds.rate_limit_per_minute,
                    ds.created_at.to_rfc3339(),
                    ds.updated_at.to_rfc3339(),
                    ds.is_visible,
                    ds.is_enabled,
                    ds.requires_admin_approval,
                    ds.crawl_frequency_hours,
                    format_nullable_timestamp(&ds.last_crawl_at),
                    format_nullable_string(&ds.crawl_status),
                    format_nullable_string(&ds.crawl_error_message),
                    format_nullable_string(&ds.api_documentation_url)
                )
            },
        );
        sql.push_str(&data_source_insert);
    }

    // Export series metadata
    if !series_metadata.is_empty() {
        sql.push_str("-- Series Metadata\n");
        let series_metadata_insert = create_bulk_insert(
            "series_metadata",
            &[
                "id",
                "source_id",
                "external_id",
                "title",
                "description",
                "units",
                "frequency",
                "geographic_level",
                "data_url",
                "api_endpoint",
                "last_discovered_at",
                "is_active",
                "created_at",
                "updated_at",
            ],
            &series_metadata,
            bulk_insert_limit,
            |sm| {
                format!(
                    "('{}', '{}', '{}', '{}', {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
                    sm.id,
                    sm.source_id,
                    escape_sql_string(&sm.external_id),
                    escape_sql_string(&sm.title),
                    format_nullable_string(&sm.description),
                    format_nullable_string(&sm.units),
                    format_nullable_string(&sm.frequency),
                    format_nullable_string(&sm.geographic_level),
                    format_nullable_string(&sm.data_url),
                    format_nullable_string(&sm.api_endpoint),
                    format_nullable_timestamp(&sm.last_discovered_at),
                    sm.is_active,
                    format_nullable_timestamp(&sm.created_at),
                    format_nullable_timestamp(&sm.updated_at)
                )
            },
        );
        sql.push_str(&series_metadata_insert);
    }

    // Export economic series
    if !economic_series.is_empty() {
        sql.push_str("-- Economic Series\n");
        let economic_series_insert = create_bulk_insert(
            "economic_series",
            &[
                "id",
                "source_id",
                "external_id",
                "title",
                "description",
                "units",
                "frequency",
                "seasonal_adjustment",
                "start_date",
                "end_date",
                "last_updated",
                "is_active",
                "created_at",
                "updated_at",
                "first_discovered_at",
                "last_crawled_at",
                "first_missing_date",
                "crawl_status",
                "crawl_error_message",
            ],
            &economic_series,
            bulk_insert_limit,
            |es| {
                format!(
                "('{}', '{}', '{}', '{}', {}, {}, '{}', {}, {}, {}, {}, {}, '{}', '{}', {}, {}, {}, {}, {})",
                es.id,
                es.source_id,
                escape_sql_string(&es.external_id),
                escape_sql_string(&es.title),
                format_nullable_string(&es.description),
                format_nullable_string(&es.units),
                escape_sql_string(&es.frequency),
                format_nullable_string(&es.seasonal_adjustment),
                format_nullable_date(&es.start_date),
                format_nullable_date(&es.end_date),
                format_nullable_timestamp(&es.last_updated),
                es.is_active,
                format_required_timestamp(&es.created_at),
                format_required_timestamp(&es.updated_at),
                format_nullable_timestamp(&es.first_discovered_at),
                format_nullable_timestamp(&es.last_crawled_at),
                format_nullable_date(&es.first_missing_date),
                format_nullable_string(&es.crawl_status),
                format_nullable_string(&es.crawl_error_message)
            )
            },
        );
        sql.push_str(&economic_series_insert);
    }

    // Export data points
    if !data_points.is_empty() {
        sql.push_str("-- Data Points\n");
        let data_points_insert = create_bulk_insert(
            "data_points",
            &[
                "id",
                "series_id",
                "date",
                "value",
                "revision_date",
                "is_original_release",
                "created_at",
                "updated_at",
            ],
            &data_points,
            bulk_insert_limit,
            |dp| {
                format!(
                    "('{}', '{}', '{}', {}, '{}', {}, '{}', '{}')",
                    dp.id,
                    dp.series_id,
                    dp.date,
                    dp.value
                        .as_ref()
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "NULL".to_string()),
                    dp.revision_date,
                    dp.is_original_release,
                    format_required_timestamp(&dp.created_at),
                    format_required_timestamp(&dp.updated_at)
                )
            },
        );
        sql.push_str(&data_points_insert);
    }

    // Export crawl attempts
    if !crawl_attempts.is_empty() {
        sql.push_str("-- Crawl Attempts\n");
        let crawl_attempts_insert = create_bulk_insert(
            "crawl_attempts",
            &[
                "id",
                "series_id",
                "attempted_at",
                "completed_at",
                "crawl_method",
                "crawl_url",
                "http_status_code",
                "data_found",
                "new_data_points",
                "latest_data_date",
                "data_freshness_hours",
                "success",
                "error_type",
                "error_message",
                "retry_count",
                "response_time_ms",
                "data_size_bytes",
                "rate_limit_remaining",
                "user_agent",
                "request_headers",
                "response_headers",
                "created_at",
                "updated_at",
            ],
            &crawl_attempts,
            bulk_insert_limit,
            |ca| {
                format!(
                "('{}', '{}', '{}', {}, '{}', {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, '{}', '{}')",
                ca.id,
                ca.series_id,
                ca.attempted_at.to_rfc3339(),
                format_nullable_timestamp(&ca.completed_at),
                escape_sql_string(&ca.crawl_method),
                format_nullable_string(&ca.crawl_url),
                ca.http_status_code.map(|c| c.to_string()).unwrap_or_else(|| "NULL".to_string()),
                ca.data_found,
                ca.new_data_points.map(|n| n.to_string()).unwrap_or_else(|| "NULL".to_string()),
                ca.latest_data_date.map(|d| format!("'{}'", d)).unwrap_or_else(|| "NULL".to_string()),
                ca.data_freshness_hours.map(|h| h.to_string()).unwrap_or_else(|| "NULL".to_string()),
                ca.success,
                format_nullable_string(&ca.error_type),
                format_nullable_string(&ca.error_message),
                ca.retry_count.map(|r| r.to_string()).unwrap_or_else(|| "NULL".to_string()),
                ca.response_time_ms.map(|r| r.to_string()).unwrap_or_else(|| "NULL".to_string()),
                ca.data_size_bytes.map(|d| d.to_string()).unwrap_or_else(|| "NULL".to_string()),
                ca.rate_limit_remaining.map(|r| r.to_string()).unwrap_or_else(|| "NULL".to_string()),
                format_nullable_string(&ca.user_agent),
                ca.request_headers.as_ref().map(|h| format!("'{}'", h)).unwrap_or_else(|| "NULL".to_string()),
                ca.response_headers.as_ref().map(|h| format!("'{}'", h)).unwrap_or_else(|| "NULL".to_string()),
                ca.created_at.to_rfc3339(),
                ca.updated_at.to_rfc3339()
            )
            },
        );
        sql.push_str(&crawl_attempts_insert);
    }

    // Write to file
    std::fs::write(&output_file, sql)?;

    info!(
        "Exported catalog data to {}: {} data sources, {} series metadata, {} economic series, {} data points, {} crawl attempts",
        output_file,
        data_sources.len(),
        series_metadata.len(),
        economic_series.len(),
        data_points.len(),
        crawl_attempts.len()
    );

    Ok(())
}
