use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use econ_graph_core::database::DataPool;
use econ_graph_sec_crawler::companyfacts::{archive::ArchiveLimits, ingest, store, IngestConfig};

/// Load SEC's nightly companyfacts/submissions archives without enabling company pages.
#[derive(Parser)]
#[command(name = "sec-companyfacts-ingest", version)]
struct Cli {
    /// Directory with room for both complete ZIPs (default maximum: 32 GiB combined).
    #[arg(long, default_value = "/tmp")]
    staging_dir: PathBuf,
    /// Operator-staged companyfacts ZIP; requires --submissions-zip. No network in this mode.
    #[arg(long, requires = "submissions_zip")]
    companyfacts_zip: Option<PathBuf>,
    #[arg(long, requires = "companyfacts_zip")]
    submissions_zip: Option<PathBuf>,
    #[arg(long, default_value = "EconGraph/1.0 (support@econgraph.com)")]
    user_agent: String,
    /// Exit unsuccessfully if the latest run failed/is running, or is older than 24 hours.
    #[arg(long, conflicts_with = "companyfacts_zip")]
    check_freshness: bool,
    /// Print up to 30 recorded runs for the release evidence gate; never synthesizes nights.
    #[arg(long, conflicts_with = "companyfacts_zip")]
    evidence: bool,
}
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let cli = Cli::parse();
    let database_url = econ_graph_core::config::data_database_url()?;
    if cli.check_freshness || cli.evidence {
        let pool = DataPool(econ_graph_core::database::create_pool(&database_url).await?);
        let runs = store::load_evidence(&pool).await?;
        if cli.check_freshness {
            store::check_freshness(&runs, chrono::Utc::now())?;
        }
        println!("{}", serde_json::to_string_pretty(&runs)?);
    } else {
        let summary = ingest(
            &database_url,
            IngestConfig {
                staging_dir: cli.staging_dir,
                companyfacts_zip: cli.companyfacts_zip,
                submissions_zip: cli.submissions_zip,
                user_agent: cli.user_agent,
                limits: ArchiveLimits::default(),
            },
        )
        .await?;
        println!("{}", serde_json::to_string_pretty(&summary)?);
    }
    Ok(())
}
