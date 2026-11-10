//! Nightly SEC companyfacts ingestion. It does not enable the unfinished company pages.
//!
//! Official bulk source contract: <https://www.sec.gov/search-filings/edgar-application-programming-interfaces>
pub mod archive;
pub mod parse;
pub mod store;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Bool},
};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use econ_graph_crawler::{source::SourceId, CrawlError};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

use archive::{ensure_unique_historical_files, ArchiveLimits, BulkArchive};
use parse::{parse_companyfacts, parse_submission_rows, parse_submissions};
use store::{finish_run, persist_company, skip_digests, start_run, RunCounts};

pub const COMPANYFACTS_URL: &str =
    "https://www.sec.gov/Archives/edgar/daily-index/xbrl/companyfacts.zip";
pub const SUBMISSIONS_URL: &str =
    "https://www.sec.gov/Archives/edgar/daily-index/bulkdata/submissions.zip";
const INGEST_LOCK: i64 = 0x5345_4343_464c_4f41;
/// Part of every company digest. Change it when loading logic changes so that every company
/// is compared with the database again instead of being skipped.
const LOADER_VERSION: &str = "sec-companyfacts-loader:v1";
/// Download attempts per bulk archive; transient failures and throttling are retried.
const DOWNLOAD_ATTEMPTS: u32 = 3;
const DOWNLOAD_RETRY_DELAY: Duration = Duration::from_secs(30);

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
    pub status: &'static str,
    pub companies_count: i64,
    pub facts_count: i64,
    pub filings_count: i64,
    pub conflicts_count: i64,
    pub fact_rows_written: i64,
    pub filing_rows_written: i64,
    pub companies_skipped: i64,
    pub archive_stale: bool,
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
    let mut counts = RunCounts::default();
    let mut hashes = (None, None);
    let mut modified = (None, None);
    let result = ingest_inner(
        &mut conn,
        id,
        config,
        &mut counts,
        &mut hashes,
        &mut modified,
    )
    .await;
    // Store only a bounded diagnostic, never database connection URLs or source payloads.
    let error = result
        .as_ref()
        .err()
        .map(|e| format!("{e}").chars().take(500).collect());
    let archive_stale = finish_run(
        &mut conn,
        id,
        result.is_ok(),
        &counts,
        error,
        hashes.clone(),
        modified,
    )
    .await
    .context("recording ingestion completion; running row remains for alerting if this fails")?;
    result?;
    if archive_stale {
        tracing::warn!(run = %id, "SEC bulk archive Last-Modified is outside the 24-hour freshness window; recorded as archive_stale");
    }
    if counts.conflicts > 0 {
        tracing::warn!(run = %id, conflicts = counts.conflicts, "companyfacts load left stored rows unchanged for conflicting source values; see company_fact_conflicts");
    }
    Ok(IngestSummary {
        id,
        status: if counts.conflicts > 0 {
            store::SUCCEEDED_WITH_CONFLICTS
        } else {
            store::SUCCEEDED
        },
        companies_count: counts.companies,
        facts_count: counts.facts,
        filings_count: counts.filings,
        conflicts_count: counts.conflicts,
        fact_rows_written: counts.fact_rows_written,
        filing_rows_written: counts.filing_rows_written,
        companies_skipped: counts.companies_skipped,
        archive_stale,
        companyfacts_sha256: hashes.0.context("missing companyfacts hash")?,
        submissions_sha256: hashes.1.context("missing submissions hash")?,
    })
}

/// One company from the producer: load it, or count it with the totals its digest recorded.
/// At most two are alive at once (channel capacity 1), so the variant size does not matter.
#[allow(clippy::large_enum_variant)]
enum Company {
    Load {
        digest: String,
        facts: parse::ParsedCompanyFacts,
        subs: parse::ParsedSubmissions,
    },
    Unchanged {
        facts: i64,
        filings: i64,
    },
}

async fn ingest_inner(
    conn: &mut AsyncPgConnection,
    run_id: uuid::Uuid,
    config: IngestConfig,
    counts: &mut RunCounts,
    hashes: &mut (Option<String>, Option<String>),
    modified: &mut (Option<DateTime<Utc>>, Option<DateTime<Utc>>),
) -> Result<()> {
    if config.companyfacts_zip.is_some() != config.submissions_zip.is_some() {
        bail!("supply both local bulk archives or neither");
    }
    let mut temporary = Vec::new();
    let (facts_path, subs_path) = if let (Some(facts), Some(subs)) =
        (&config.companyfacts_zip, &config.submissions_zip)
    {
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
            // download_to_file truncates the file first, so a retry never appends.
            let mut attempt = 1;
            let receipt = loop {
                match http
                    .download_to_file(
                        SourceId::Sec,
                        url,
                        &mut file,
                        config.limits.max_download_bytes,
                        Duration::from_secs(4 * 60 * 60),
                    )
                    .await
                {
                    Ok(receipt) => break receipt,
                    Err(e) => {
                        let Some(delay) = retry_delay(&e, attempt) else {
                            return Err(e.into());
                        };
                        tracing::warn!(url, attempt, error = %e, ?delay, "SEC bulk download failed; retrying");
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                    }
                }
            };
            file.sync_all().await?;
            // Staleness is recorded on the run (archive_stale), not fatal.
            timestamps.push(source_timestamp(receipt.last_modified.as_deref())?);
            paths.push(tmp.path().to_owned());
            temporary.push(tmp);
        }
        *modified = (Some(timestamps[0]), Some(timestamps[1]));
        (paths.remove(0), paths.remove(0))
    };
    // Reading/decompressing and parsing run off the async executor. The bounded channel allows
    // at most one queued company plus the current company's facts in memory, never a whole ZIP.
    let limits = config.limits;
    let digests = skip_digests(conn).await?;
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
    let producer = tokio::task::spawn_blocking(move || -> Result<(String, String)> {
        let _keep_temp_files: Vec<NamedTempFile> = temporary;
        let facts_hash = file_hash(&facts_path)?;
        let subs_hash = file_hash(&subs_path)?;
        let mut facts = BulkArchive::open_verified(&facts_path, limits)?;
        let mut subs = BulkArchive::open_verified(&subs_path, limits)?;
        let fact_names = facts.company_names();
        if fact_names.len() != facts.entries.len() {
            bail!("companyfacts archive contains non-company entries");
        }
        // Some XBRL issuers use only custom concepts and have no companyfacts document.
        // Submissions, including its historical chunks, is the issuer-discovery authority.
        let names: BTreeSet<_> = fact_names.into_iter().chain(subs.company_names()).collect();
        for name in names {
            let cik = &name[3..13]; // BulkArchive validated the CIK10 main-document name.
            let main = subs.read(&name)?;
            let mut parsed_subs = parse_submissions(&main, cik)?;
            ensure_unique_historical_files(&parsed_subs.historical_files, cik)?;
            let mut digest = Sha256::new();
            digest.update(LOADER_VERSION.as_bytes());
            digest_entry(&mut digest, &name, &main);
            for historical in &parsed_subs.historical_files {
                let bytes = subs.read(historical)?;
                digest_entry(&mut digest, historical, &bytes);
                let root = serde_json::from_slice(&bytes)?;
                parsed_subs
                    .filings
                    .extend(parse_submission_rows(&root, cik)?);
                if parsed_subs.filings.len() > 100_000 {
                    bail!("company exceeds bounded filing count");
                }
            }
            let facts_entry = if facts.entries.contains_key(&name) {
                Some(facts.read(&name)?)
            } else {
                None
            };
            if let Some(bytes) = &facts_entry {
                digest.update(b"companyfacts");
                digest_entry(&mut digest, &name, bytes);
            }
            let digest = hex::encode(digest.finalize());
            if let Some((recorded, facts_count, filings_count)) = digests.get(cik)
                && *recorded == digest
            {
                sender
                    .blocking_send(Company::Unchanged {
                        facts: *facts_count,
                        filings: *filings_count,
                    })
                    .map_err(|_| anyhow::anyhow!("ingestion consumer stopped"))?;
                continue;
            }
            let parsed_facts = if let Some(bytes) = facts_entry {
                let parsed = parse_companyfacts(&bytes)?;
                if parsed.cik != cik {
                    bail!("companyfacts entry CIK mismatch");
                }
                parsed
            } else {
                if !parsed_subs.filings.iter().any(|filing| filing.is_xbrl) {
                    continue;
                }
                // Preserve discovery and filing provenance without inventing standard facts.
                parse::ParsedCompanyFacts {
                    cik: cik.to_owned(),
                    name: parsed_subs.name.clone(),
                    facts: Vec::new(),
                    duplicates: Vec::new(),
                }
            };
            // An accession listed twice with different metadata is quarantined, not loaded.
            let mut unique: BTreeMap<String, parse::SubmissionFiling> = BTreeMap::new();
            let mut duplicates = BTreeMap::new();
            for filing in parsed_subs.filings.drain(..) {
                match unique.get(&filing.accession) {
                    Some(old) if *old != filing => {
                        duplicates
                            .entry(filing.accession.clone())
                            .or_insert_with(|| (old.clone(), filing));
                    }
                    Some(_) => {}
                    None => {
                        unique.insert(filing.accession.clone(), filing);
                    }
                }
            }
            for accession in duplicates.keys() {
                unique.remove(accession);
            }
            parsed_subs.filings = unique.into_values().collect();
            parsed_subs.duplicates = duplicates.into_values().collect();
            sender
                .blocking_send(Company::Load {
                    digest,
                    facts: parsed_facts,
                    subs: parsed_subs,
                })
                .map_err(|_| anyhow::anyhow!("ingestion consumer stopped"))?;
        }
        Ok((facts_hash, subs_hash))
    });
    let mut consumer_error = None;
    while let Some(company) = receiver.recv().await {
        match company {
            Company::Unchanged { facts, filings } => {
                counts.companies += 1;
                counts.companies_skipped += 1;
                counts.facts += facts;
                counts.filings += filings;
            }
            // Conflicts are recorded and do not stop the load; a database error does.
            Company::Load {
                digest,
                facts,
                subs,
            } => match persist_company(conn, run_id, &digest, facts, subs).await {
                Ok(outcome) => counts.add(&outcome),
                Err(e) => {
                    consumer_error = Some(e);
                    break;
                }
            },
        }
    }
    drop(receiver);
    let produced = producer.await.context("bulk parser task failed")?;
    if let Some(error) = consumer_error {
        return Err(error);
    }
    let (facts_hash, subs_hash) = produced?;
    *hashes = (Some(facts_hash), Some(subs_hash));
    if counts.companies <= 0 || counts.facts <= 0 || counts.filings <= 0 {
        bail!("bulk load has no usable companies, facts or filings; refusing successful evidence");
    }
    Ok(())
}

/// The upstream Last-Modified receipt. A missing or unparseable one fails the load: without it
/// the run cannot state whether the archive was fresh.
fn source_timestamp(modified: Option<&str>) -> Result<DateTime<Utc>> {
    let modified = DateTime::parse_from_rfc2822(
        modified.context("SEC bulk response has no Last-Modified timestamp")?,
    )?;
    Ok(modified.with_timezone(&Utc))
}

/// The wait before download attempt `attempt + 1`, or None when the error is permanent or the
/// attempts are used up. Throttling honours Retry-After up to ten minutes.
fn retry_delay(error: &CrawlError, attempt: u32) -> Option<Duration> {
    if !error.is_retryable() || attempt >= DOWNLOAD_ATTEMPTS {
        return None;
    }
    Some(
        error
            .retry_after()
            .map(|d| d.min(Duration::from_secs(600)))
            .unwrap_or(DOWNLOAD_RETRY_DELAY * 2u32.pow(attempt - 1)),
    )
}

/// Adds one archive entry to a company digest; the length prefix keeps entries separate.
fn digest_entry(digest: &mut Sha256, name: &str, bytes: &[u8]) {
    digest.update((name.len() as u64).to_le_bytes());
    digest.update(name.as_bytes());
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
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
