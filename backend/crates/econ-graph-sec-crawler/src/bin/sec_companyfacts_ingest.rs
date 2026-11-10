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
    /// Require a verified downloaded success within 24h; running loads expire after 8h.
    #[arg(long, conflicts_with = "companyfacts_zip")]
    check_freshness: bool,
    /// Print up to 30 recorded runs for the release evidence gate; never synthesizes nights.
    #[arg(long, conflicts_with = "companyfacts_zip")]
    evidence: bool,
    /// Print up to 1000 conflicts with this status (open, accepted or rejected) and exit.
    #[arg(long, value_name = "STATUS", conflicts_with_all = ["companyfacts_zip", "check_freshness", "evidence"])]
    list_conflicts: Option<String>,
    /// Accept an open fact or filing conflict as a new revision of the stored row. The stored
    /// row is not changed. Requires --operator.
    #[arg(long, value_name = "ID", requires = "operator", conflicts_with_all = ["companyfacts_zip", "check_freshness", "evidence", "list_conflicts", "reject_conflict"])]
    accept_conflict: Option<uuid::Uuid>,
    /// Reject an open conflict: keep the stored row and stop counting the proposal.
    #[arg(long, value_name = "ID", requires = "operator", conflicts_with_all = ["companyfacts_zip", "check_freshness", "evidence", "list_conflicts"])]
    reject_conflict: Option<uuid::Uuid>,
    /// Who resolves the conflict; recorded with the decision.
    #[arg(long)]
    operator: Option<String>,
    /// Why; recorded with the decision.
    #[arg(long, requires = "operator")]
    note: Option<String>,
}
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let cli = Cli::parse();
    let database_url = econ_graph_core::config::data_database_url()?;
    if cli.list_conflicts.is_some()
        || cli.accept_conflict.is_some()
        || cli.reject_conflict.is_some()
    {
        use diesel_async::AsyncConnection;
        let mut conn = diesel_async::AsyncPgConnection::establish(&database_url).await?;
        let operator = cli.operator.as_deref().unwrap_or_default();
        if let Some(status) = cli.list_conflicts {
            let conflicts = store::list_conflicts(&mut conn, &status).await?;
            println!("{}", serde_json::to_string_pretty(&conflicts)?);
        } else if let Some(id) = cli.accept_conflict {
            let revision =
                store::accept_conflict(&mut conn, id, operator, cli.note.as_deref()).await?;
            println!(
                "{}",
                serde_json::json!({"conflict": id, "status": "accepted", "revision_id": revision})
            );
        } else if let Some(id) = cli.reject_conflict {
            store::reject_conflict(&mut conn, id, operator, cli.note.as_deref()).await?;
            println!(
                "{}",
                serde_json::json!({"conflict": id, "status": "rejected"})
            );
        }
    } else if cli.check_freshness || cli.evidence {
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
