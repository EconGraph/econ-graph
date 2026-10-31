//! Nightly SEC companyfacts ingestion. It does not enable the unfinished company pages.
//!
//! Official bulk source contract: https://www.sec.gov/search-filings/edgar-application-programming-interfaces
pub mod archive;
pub mod parse;
pub mod store;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Bool},
};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use econ_graph_crawler::source::SourceId;
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

use archive::{ensure_unique_historical_files, ArchiveLimits, BulkArchive};
use parse::{parse_companyfacts, parse_submission_rows, parse_submissions};
use store::{finish_run, persist_company, start_run};

pub const COMPANYFACTS_URL: &str =
    "https://www.sec.gov/Archives/edgar/daily-index/xbrl/companyfacts.zip";
pub const SUBMISSIONS_URL: &str =
    "https://www.sec.gov/Archives/edgar/daily-index/bulkdata/submissions.zip";
const INGEST_LOCK: i64 = 0x5345_4343_464c_4f41;

#[derive(Debug, Clone)]
pub struct IngestConfig {
    pub staging_dir: PathBuf,
    pub companyfacts_zip: Option<PathBuf>,
    pub submissions_zip: Option<PathBuf>,
    pub user_agent: String,
    pub limits: ArchiveLimits,
}

#[derive(Debug, serde::Serialize)]
pub struct IngestSummary {
    pub id: uuid::Uuid,
    pub companies_count: i64,
    pub facts_count: i64,
    pub filings_count: i64,
    pub companyfacts_sha256: String,
    pub submissions_sha256: String,
}

/// A dedicated connection holds the advisory lock; dropping it on any path releases the lock
/// even if failure reporting fails. Concurrent runs cannot race provenance or run evidence.
pub async fn ingest(database_url: &str, config: IngestConfig) -> Result<IngestSummary> {
    let mut conn = AsyncPgConnection::establish(database_url)
        .await
        .context("connecting ingestion data database")?;
    #[derive(QueryableByName)]
    struct Locked {
        #[diesel(sql_type=Bool)]
        locked: bool,
    }
    let locked: Locked = diesel::sql_query("SELECT pg_try_advisory_lock($1) AS locked")
        .bind::<BigInt, _>(INGEST_LOCK)
        .get_result(&mut conn)
        .await?;
    if !locked.locked {
        bail!("another SEC companyfacts ingestion is running");
    }
    let staged = config.companyfacts_zip.is_some() || config.submissions_zip.is_some();
    let id = start_run(&mut conn, staged).await?;
    let mut counts = (0i64, 0i64, 0i64);
    let mut hashes = (None, None);
    let mut modified = (None, None);
    let result = ingest_inner(&mut conn, config, &mut counts, &mut hashes, &mut modified).await;
    // Store only a bounded diagnostic, never database connection URLs or source payloads.
    let error = result
        .as_ref()
        .err()
        .map(|e| format!("{e}").chars().take(500).collect());
    finish_run(
        &mut conn,
        id,
        result.is_ok(),
        counts,
        error,
        hashes.clone(),
        modified,
    )
    .await
    .context("recording ingestion completion; running row remains for alerting if this fails")?;
    result?;
    Ok(IngestSummary {
        id,
        companies_count: counts.0,
        facts_count: counts.1,
        filings_count: counts.2,
        companyfacts_sha256: hashes.0.context("missing companyfacts hash")?,
        submissions_sha256: hashes.1.context("missing submissions hash")?,
    })
}

async fn ingest_inner(
    conn: &mut AsyncPgConnection,
    config: IngestConfig,
    counts: &mut (i64, i64, i64),
    hashes: &mut (Option<String>, Option<String>),
    modified: &mut (Option<DateTime<Utc>>, Option<DateTime<Utc>>),
) -> Result<()> {
    if config.companyfacts_zip.is_some() != config.submissions_zip.is_some() {
        bail!("supply both local bulk archives or neither");
    }
    let mut temporary = Vec::new();
    let (facts_path, subs_path) =
        if let (Some(facts), Some(subs)) = (&config.companyfacts_zip, &config.submissions_zip) {
            (facts.clone(), subs.clone())
        } else {
            let http = crate::sec_http_fetcher(&config.user_agent, 1)?;
            let mut paths = Vec::new();
            let mut timestamps = Vec::new();
            for url in [COMPANYFACTS_URL, SUBMISSIONS_URL] {
                let tmp = tempfile::Builder::new()
                    .prefix("sec-bulk-")
                    .suffix(".part")
                    .tempfile_in(&config.staging_dir)?;
                let mut file = tokio::fs::File::from_std(tmp.reopen()?);
                let receipt = http
                    .download_to_file(
                        SourceId::Sec,
                        url,
                        &mut file,
                        config.limits.max_download_bytes,
                        Duration::from_secs(4 * 60 * 60),
                    )
                    .await?;
                file.sync_all().await?;
                timestamps.push(validate_source_timestamp(
                    receipt.last_modified.as_deref(),
                    Utc::now(),
                )?);
                paths.push(tmp.path().to_owned());
                temporary.push(tmp);
            }
            *modified = (Some(timestamps[0]), Some(timestamps[1]));
            (paths.remove(0), paths.remove(0))
        };
    // Reading/decompressing and parsing run off the async executor. The bounded channel allows
    // at most one queued company plus the current company's facts in memory, never a whole ZIP.
    let limits = config.limits;
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
    let producer = tokio::task::spawn_blocking(move || -> Result<(String, String)> {
        let _keep_temp_files: Vec<NamedTempFile> = temporary;
        let facts_hash = file_hash(&facts_path)?;
        let subs_hash = file_hash(&subs_path)?;
        let mut facts = BulkArchive::open_verified(&facts_path, limits)?;
        let mut subs = BulkArchive::open_verified(&subs_path, limits)?;
        let names = facts.company_names();
        if names.len() != facts.entries.len() {
            bail!("companyfacts archive contains non-company entries");
        }
        for name in names {
            let parsed_facts = parse_companyfacts(&facts.read(&name)?)?;
            if name != format!("CIK{}.json", parsed_facts.cik) {
                bail!("companyfacts entry CIK mismatch");
            }
            let mut parsed_subs = parse_submissions(&subs.read(&name)?, &parsed_facts.cik)?;
            ensure_unique_historical_files(&parsed_subs.historical_files, &parsed_facts.cik)?;
            for historical in &parsed_subs.historical_files {
                let root = serde_json::from_slice(&subs.read(historical)?)?;
                parsed_subs
                    .filings
                    .extend(parse_submission_rows(&root, &parsed_facts.cik)?);
                if parsed_subs.filings.len() > 100_000 {
                    bail!("company exceeds bounded filing count");
                }
            }
            let mut unique = BTreeMap::new();
            for filing in parsed_subs.filings.drain(..) {
                if let Some(old) = unique.insert(filing.accession.clone(), filing.clone()) {
                    if serde_json::to_value(old)? != serde_json::to_value(&filing)? {
                        bail!("conflicting duplicate filing metadata");
                    }
                }
            }
            parsed_subs.filings = unique.into_values().collect();
            sender
                .blocking_send((parsed_facts, parsed_subs))
                .map_err(|_| anyhow::anyhow!("ingestion consumer stopped"))?;
        }
        Ok((facts_hash, subs_hash))
    });
    let mut consumer_error = None;
    while let Some((facts, subs)) = receiver.recv().await {
        match persist_company(conn, facts, subs).await {
            Ok((facts, filings)) => {
                counts.0 += 1;
                counts.1 += facts as i64;
                counts.2 += filings as i64;
            }
            Err(e) => {
                consumer_error = Some(e);
                break;
            }
        }
    }
    drop(receiver);
    let produced = producer.await.context("bulk parser task failed")?;
    if let Some(error) = consumer_error {
        return Err(error);
    }
    let (facts_hash, subs_hash) = produced?;
    *hashes = (Some(facts_hash), Some(subs_hash));
    for date in [modified.0, modified.1].into_iter().flatten() {
        validate_source_timestamp(Some(&date.to_rfc2822()), Utc::now())?;
    }
    Ok(())
}

fn validate_source_timestamp(modified: Option<&str>, now: DateTime<Utc>) -> Result<DateTime<Utc>> {
    let modified = DateTime::parse_from_rfc2822(
        modified.context("SEC bulk response has no Last-Modified timestamp")?,
    )?;
    let age = now.signed_duration_since(modified);
    if age.num_seconds() > 24 * 60 * 60 || age.num_seconds() < -5 * 60 {
        bail!("SEC bulk archive timestamp is stale or in the future");
    }
    Ok(modified.with_timezone(&Utc))
}
fn file_hash(path: &Path) -> Result<String> {
    use std::io::Read;
    let mut reader = std::fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let size = reader.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        hash.update(&buffer[..size]);
    }
    Ok(hex::encode(hash.finalize()))
}

#[cfg(test)]
mod tests;
