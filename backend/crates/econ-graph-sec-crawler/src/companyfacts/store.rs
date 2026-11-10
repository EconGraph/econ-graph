use std::collections::{BTreeMap, HashMap};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Bool, Jsonb, Nullable, Text, Timestamptz, Uuid as SqlUuid},
};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use econ_graph_core::{
    database::{DataPool, DatabasePool},
    models::{CompanyFact, ResearchPeriod},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use super::parse::{ParsedCompanyFacts, ParsedSubmissions, SubmissionFiling};
use crate::crawler::CompanyFields;

/// Statuses of a load that finished its archives. Conflicts do not fail a load.
pub const SUCCEEDED: &str = "succeeded";
pub const SUCCEEDED_WITH_CONFLICTS: &str = "succeeded_with_conflicts";

#[derive(Debug, Clone, Serialize, Deserialize, QueryableByName)]
pub struct LoadRun {
    #[diesel(sql_type = SqlUuid)]
    pub id: Uuid,
    #[diesel(sql_type = Timestamptz)]
    pub started_at: DateTime<Utc>,
    #[diesel(sql_type = Nullable<Timestamptz>)]
    pub completed_at: Option<DateTime<Utc>>,
    #[diesel(sql_type = Text)]
    pub status: String,
    #[diesel(sql_type = Text)]
    pub source_mode: String,
    #[diesel(sql_type = Nullable<Timestamptz>)]
    pub companyfacts_last_modified: Option<DateTime<Utc>>,
    #[diesel(sql_type = Nullable<Timestamptz>)]
    pub submissions_last_modified: Option<DateTime<Utc>>,
    #[diesel(sql_type = BigInt)]
    pub companies_count: i64,
    #[diesel(sql_type = BigInt)]
    pub facts_count: i64,
    #[diesel(sql_type = BigInt)]
    pub filings_count: i64,
    #[diesel(sql_type = Nullable<Text>)]
    pub error: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub companyfacts_sha256: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub submissions_sha256: Option<String>,
    /// Open conflicts this run saw (new or still unresolved).
    #[diesel(sql_type = BigInt)]
    pub conflicts_count: i64,
    #[diesel(sql_type = BigInt)]
    pub fact_rows_written: i64,
    #[diesel(sql_type = BigInt)]
    pub filing_rows_written: i64,
    /// Companies whose archive entries matched the digest of their last clean load.
    #[diesel(sql_type = BigInt)]
    pub companies_skipped: i64,
    /// A receipt was outside 24 hours before to 5 minutes after completion.
    #[diesel(sql_type = Bool)]
    pub archive_stale: bool,
}

impl LoadRun {
    pub fn succeeded(&self) -> bool {
        self.status == SUCCEEDED || self.status == SUCCEEDED_WITH_CONFLICTS
    }
}

/// Totals for one load, accumulated per company. `companies`, `facts` and `filings` describe
/// the loaded snapshot (skipped companies count with their recorded totals); `*_rows_written`
/// count rows inserted or updated.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RunCounts {
    pub companies: i64,
    pub facts: i64,
    pub filings: i64,
    pub conflicts: i64,
    pub fact_rows_written: i64,
    pub filing_rows_written: i64,
    pub companies_skipped: i64,
}

impl RunCounts {
    pub fn add(&mut self, company: &CompanyOutcome) {
        self.companies += 1;
        self.facts += company.facts;
        self.filings += company.filings;
        self.conflicts += company.conflicts;
        self.fact_rows_written += company.fact_rows_written;
        self.filing_rows_written += company.filing_rows_written;
    }
}

pub async fn start_run(conn: &mut AsyncPgConnection, staged: bool) -> Result<Uuid> {
    #[derive(QueryableByName)]
    struct Id {
        #[diesel(sql_type=SqlUuid)]
        id: Uuid,
    }
    let result: Id = diesel::sql_query(
        "INSERT INTO company_fact_load_runs (id,status,source_mode) VALUES ($1,'running',$2) RETURNING id",
    )
    .bind::<SqlUuid, _>(Uuid::new_v4())
        .bind::<Text,_>(if staged { "staged" } else { "downloaded" })
    .get_result(&mut *conn)
    .await?;
    Ok(result.id)
}

/// Records the outcome. A load with open conflicts is `succeeded_with_conflicts`. Archive
/// staleness is computed here against the completion time, as the provenance check requires.
/// Returns whether the archive was stale.
pub async fn finish_run(
    conn: &mut AsyncPgConnection,
    id: Uuid,
    success: bool,
    counts: &RunCounts,
    error: Option<String>,
    hashes: (Option<String>, Option<String>),
    modified: (Option<DateTime<Utc>>, Option<DateTime<Utc>>),
) -> Result<bool> {
    #[derive(QueryableByName)]
    struct Stale {
        #[diesel(sql_type = Bool)]
        archive_stale: bool,
    }
    let status = match (success, counts.conflicts > 0) {
        (false, _) => "failed",
        (true, false) => SUCCEEDED,
        (true, true) => SUCCEEDED_WITH_CONFLICTS,
    };
    let stale: Stale = diesel::sql_query("UPDATE company_fact_load_runs SET completed_at=now(),status=$2,companies_count=$3,facts_count=$4,filings_count=$5,error=$6,companyfacts_sha256=$7,submissions_sha256=$8,companyfacts_last_modified=$9,submissions_last_modified=$10,conflicts_count=$11,fact_rows_written=$12,filing_rows_written=$13,companies_skipped=$14,
        archive_stale = ($9 IS NOT NULL AND $10 IS NOT NULL AND NOT ($9 >= now() - INTERVAL '24 hours' AND $10 >= now() - INTERVAL '24 hours' AND $9 <= now() + INTERVAL '5 minutes' AND $10 <= now() + INTERVAL '5 minutes'))
        WHERE id=$1 RETURNING archive_stale")
        .bind::<SqlUuid,_>(id).bind::<Text,_>(status)
        .bind::<BigInt,_>(counts.companies).bind::<BigInt,_>(counts.facts).bind::<BigInt,_>(counts.filings)
        .bind::<Nullable<Text>,_>(error).bind::<Nullable<Text>,_>(hashes.0).bind::<Nullable<Text>,_>(hashes.1)
        .bind::<Nullable<Timestamptz>,_>(modified.0)
        .bind::<Nullable<Timestamptz>,_>(modified.1)
        .bind::<BigInt,_>(counts.conflicts).bind::<BigInt,_>(counts.fact_rows_written)
        .bind::<BigInt,_>(counts.filing_rows_written).bind::<BigInt,_>(counts.companies_skipped)
        .get_result(&mut *conn).await?;
    Ok(stale.archive_stale)
}
pub async fn load_evidence(pool: &DataPool) -> Result<Vec<LoadRun>> {
    let mut conn = pool.get().await?;
    Ok(
        diesel::sql_query("SELECT * FROM company_fact_load_runs ORDER BY started_at DESC LIMIT 30")
            .load(&mut conn)
            .await?,
    )
}
pub fn check_freshness(runs: &[LoadRun], now: DateTime<Utc>) -> Result<()> {
    let latest = runs.first().context("no companyfacts load has run")?;
    // A normal active load does not make a still-fresh successful snapshot disappear.
    // A killed process leaves a running row; the same eight-hour bound as the Job deadline
    // detects it even when an older successful snapshot would otherwise remain fresh.
    if latest.status == "running" {
        let age = now.signed_duration_since(latest.started_at).num_seconds();
        if !(0..=8 * 60 * 60).contains(&age) {
            bail!("latest companyfacts load is stuck or has an invalid start timestamp");
        }
    }
    // Conflicts are quarantined rows, not missing data: such a load is still a snapshot.
    let successful = runs
        .iter()
        .find(|run| run.succeeded() && run.source_mode == "downloaded")
        .context("no verified downloaded companyfacts load has succeeded")?;
    if successful.companies_count <= 0
        || successful.facts_count <= 0
        || successful.filings_count <= 0
    {
        bail!("successful load has no usable companies, facts or filings");
    }
    let completed = successful
        .completed_at
        .context("load has no completion timestamp")?;
    if completed > now || now.signed_duration_since(completed).num_seconds() > 24 * 60 * 60 {
        bail!("companyfacts successful load is stale");
    }
    // Source publication must have been fresh when this run completed. Comparing it with
    // now instead would introduce a false daily gap before the next scheduled ingestion.
    for modified in [
        successful.companyfacts_last_modified,
        successful.submissions_last_modified,
    ] {
        let modified = modified.context("missing upstream publication timestamp")?;
        let age = completed.signed_duration_since(modified).num_seconds();
        if !(-5 * 60..=24 * 60 * 60).contains(&age) {
            bail!("successful load did not have fresh upstream companyfacts/submissions receipts");
        }
    }
    Ok(())
}

/// Per-company archive digests recorded by successful loads, keyed by CIK:
/// (digest, facts, filings). A company is skipped while its entries still hash to the digest.
pub async fn skip_digests(
    conn: &mut AsyncPgConnection,
) -> Result<HashMap<String, (String, i64, i64)>> {
    #[derive(QueryableByName)]
    struct Digest {
        #[diesel(sql_type = Text)]
        cik: String,
        #[diesel(sql_type = Text)]
        entry_sha256: String,
        #[diesel(sql_type = BigInt)]
        facts_count: i64,
        #[diesel(sql_type = BigInt)]
        filings_count: i64,
    }
    let rows: Vec<Digest> = diesel::sql_query(
        "SELECT d.cik, d.entry_sha256, d.facts_count, d.filings_count
         FROM company_fact_company_digests d JOIN company_fact_load_runs r ON r.id = d.run_id
         WHERE r.status IN ('succeeded', 'succeeded_with_conflicts')",
    )
    .load(&mut *conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|d| (d.cik, (d.entry_sha256, d.facts_count, d.filings_count)))
        .collect())
}

/// What one company's load did. `facts`/`filings` are the source rows in the archive;
/// `conflicts` counts open conflicts (new or seen again), which leave stored rows unchanged.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompanyOutcome {
    pub facts: i64,
    pub filings: i64,
    pub conflicts: i64,
    pub fact_rows_written: i64,
    pub filing_rows_written: i64,
}

#[derive(QueryableByName)]
struct ChunkResult {
    #[diesel(sql_type = BigInt)]
    written: i64,
    #[diesel(sql_type = BigInt)]
    conflicts: i64,
}

/// One company's metadata, filings and facts are committed together. Unchanged rows are not
/// written. A row whose value differs from the stored (latest accepted) revision is recorded in
/// `company_fact_conflicts` and the stored row is left as it is; the rest of the company loads.
/// The company's archive digest is recorded only when it has no open conflicts, so a company
/// with an unresolved conflict is checked again on every run.
pub async fn persist_company(
    conn: &mut AsyncPgConnection,
    run_id: Uuid,
    digest: &str,
    facts: ParsedCompanyFacts,
    subs: ParsedSubmissions,
) -> Result<CompanyOutcome> {
    if facts.cik != subs.cik {
        bail!("company CIK mismatch");
    }
    conn.transaction::<_, anyhow::Error, _>(async move |conn| {
        // Preserve the existing UUID and richer metadata other SEC entry points collected.
        let company = CompanyFields::clipped(&subs.cik, &subs.name, subs.ticker.as_deref(),
            subs.sic.as_deref(), subs.sic_description.as_deref(), subs.entity_type.as_deref());
        diesel::sql_query("INSERT INTO companies (cik,name,ticker,sic_code,sic_description,entity_type) VALUES ($1,$2,$3,$4,$5,$6)
            ON CONFLICT (cik) DO UPDATE SET name=EXCLUDED.name,ticker=EXCLUDED.ticker,sic_code=EXCLUDED.sic_code,
              sic_description=EXCLUDED.sic_description,entity_type=EXCLUDED.entity_type,updated_at=now()
            WHERE (companies.name,companies.ticker,companies.sic_code,companies.sic_description,companies.entity_type)
              IS DISTINCT FROM (EXCLUDED.name,EXCLUDED.ticker,EXCLUDED.sic_code,EXCLUDED.sic_description,EXCLUDED.entity_type)")
            .bind::<Text,_>(&subs.cik).bind::<Text,_>(&company.name).bind::<Nullable<Text>,_>(&company.ticker)
            .bind::<Nullable<Text>,_>(&company.sic).bind::<Nullable<Text>,_>(&company.sic_description)
            .bind::<Nullable<Text>,_>(&company.entity_type)
            .execute(&mut *conn).await?;
        let mut outcome = CompanyOutcome {
            facts: facts.facts.len() as i64,
            filings: subs.filings.len() as i64,
            ..CompanyOutcome::default()
        };
        for chunk in subs.filings.chunks(500) {
            let r = persist_filings(conn, run_id, chunk).await?;
            outcome.filing_rows_written += r.written;
            outcome.conflicts += r.conflicts;
        }
        persist_concepts(conn, &facts.facts).await?;
        for chunk in facts.facts.chunks(500) {
            let r = persist_facts(conn, run_id, chunk).await?;
            outcome.fact_rows_written += r.written;
            outcome.conflicts += r.conflicts;
        }
        outcome.conflicts += record_source_duplicates(conn, run_id, &facts, &subs).await?;
        if outcome.conflicts == 0 {
            diesel::sql_query("INSERT INTO company_fact_company_digests (cik,entry_sha256,facts_count,filings_count,run_id) VALUES ($1,$2,$3,$4,$5)
                ON CONFLICT (cik) DO UPDATE SET entry_sha256=EXCLUDED.entry_sha256,facts_count=EXCLUDED.facts_count,
                  filings_count=EXCLUDED.filings_count,run_id=EXCLUDED.run_id,updated_at=now()")
                .bind::<Text,_>(&subs.cik).bind::<Text,_>(digest).bind::<BigInt,_>(outcome.facts)
                .bind::<BigInt,_>(outcome.filings).bind::<SqlUuid,_>(run_id)
                .execute(&mut *conn).await?;
        }
        Ok(outcome)
    }).await
}

fn fact_json(f: &CompanyFact) -> serde_json::Value {
    let (start, end) = match f.subject.period {
        ResearchPeriod::Instant { date } => (None, date),
        ResearchPeriod::Duration { start, end } => (Some(start), end),
    };
    json!({"id":f.id,"cik":f.subject.cik,"taxonomy":f.subject.taxonomy,"concept":f.subject.concept,
        "period_start":start,"period_end":end,"unit":f.subject.unit,"accession":f.accession,
        "filed":f.filed,"value":f.value.to_string(),"form":f.form,"fiscal_year":f.fiscal_year,
        "fiscal_period":f.fiscal_period,"frame":f.frame})
}

// Conflict rows are keyed by a digest of the stored revision number and the proposal: the
// same proposal against the same revision only refreshes last_seen_*. Accepted proposals
// become the latest revision and stop conflicting; rejected ones are refreshed but not counted.
const UPSERT_CONFLICT: &str = "ON CONFLICT (kind,cik,target_key,proposed_digest) DO UPDATE
    SET last_seen_run_id=EXCLUDED.last_seen_run_id,last_seen_at=now(),seen_count=company_fact_conflicts.seen_count+1
    RETURNING status";

/// Inserts new facts with `DO NOTHING`; compares the others with their latest revision.
async fn persist_facts(
    conn: &mut AsyncPgConnection,
    run_id: Uuid,
    chunk: &[CompanyFact],
) -> Result<ChunkResult> {
    let rows: Vec<_> = chunk.iter().map(fact_json).collect();
    Ok(diesel::sql_query(format!("WITH r AS (
          SELECT * FROM jsonb_to_recordset($1) AS r(id uuid,cik text,taxonomy text,concept text,period_start date,period_end date,unit text,accession text,filed date,value text,form text,fiscal_year int,fiscal_period text,frame text)),
        cur AS (
          SELECT f.id, coalesce(rev.revision,1) AS revision, coalesce(rev.value,f.value) AS value,
                 coalesce(rev.filed,f.filed) AS filed, coalesce(rev.form,f.form) AS form
          FROM r JOIN company_facts f ON f.id=r.id
          LEFT JOIN LATERAL (SELECT revision,value,filed,form FROM company_fact_revisions x WHERE x.fact_id=f.id ORDER BY revision DESC LIMIT 1) rev ON true),
        ins AS (
          INSERT INTO company_facts (id,cik,taxonomy,concept,period_start,period_end,unit,accession,filed,value,form,fiscal_year,fiscal_period,frame)
          SELECT id,cik,taxonomy,concept,period_start,period_end,unit,accession,filed,value::numeric,form,fiscal_year,fiscal_period,frame
          FROM r WHERE NOT EXISTS (SELECT 1 FROM cur WHERE cur.id=r.id)
          ON CONFLICT DO NOTHING RETURNING 1),
        conf AS (
          INSERT INTO company_fact_conflicts (id,kind,cik,accession,target_key,stored,proposed,proposed_digest,first_seen_run_id,last_seen_run_id)
          SELECT gen_random_uuid(),'fact',r.cik,r.accession,r.id::text,
                 jsonb_build_object('revision',cur.revision,'value',cur.value::text,'filed',cur.filed,'form',cur.form),
                 p.proposed, md5(cur.revision||':'||p.proposed::text), $2, $2
          FROM r JOIN cur ON cur.id=r.id
          CROSS JOIN LATERAL (SELECT jsonb_build_object('value',r.value,'filed',r.filed,'form',r.form) AS proposed) p
          WHERE cur.value<>r.value::numeric OR cur.filed<>r.filed OR cur.form<>r.form
          {UPSERT_CONFLICT})
        SELECT (SELECT count(*) FROM ins) AS written, (SELECT count(*) FROM conf WHERE status='open') AS conflicts"))
        .bind::<Jsonb,_>(json!(rows)).bind::<SqlUuid,_>(run_id)
        .get_result(&mut *conn).await?)
}

/// New filings are inserted; `primary_document`/`is_xbrl` are refreshed only when they
/// differ and the filing's identity fields (filed, form, report date) still match.
async fn persist_filings(
    conn: &mut AsyncPgConnection,
    run_id: Uuid,
    rows: &[SubmissionFiling],
) -> Result<ChunkResult> {
    Ok(diesel::sql_query(format!("WITH r AS (
          SELECT * FROM jsonb_to_recordset($1) AS r(cik text,accession text,filed date,report_date date,form text,primary_document text,is_xbrl bool)),
        cur AS (
          SELECT f.id, f.accession, f.primary_document, f.is_xbrl, coalesce(rev.revision,1) AS revision,
                 coalesce(rev.filed,f.filed) AS filed, CASE WHEN rev.revision IS NULL THEN f.report_date ELSE rev.report_date END AS report_date,
                 coalesce(rev.form,f.form) AS form
          FROM r JOIN company_fact_filings f ON f.cik=r.cik AND f.accession=r.accession
          LEFT JOIN LATERAL (SELECT revision,filed,report_date,form FROM company_fact_filing_revisions x WHERE x.filing_id=f.id ORDER BY revision DESC LIMIT 1) rev ON true),
        ins AS (
          INSERT INTO company_fact_filings (id,cik,accession,filed,report_date,form,primary_document,is_xbrl)
          SELECT gen_random_uuid(),cik,accession,filed,report_date,form,primary_document,is_xbrl
          FROM r WHERE NOT EXISTS (SELECT 1 FROM cur WHERE cur.accession=r.accession)
          ON CONFLICT DO NOTHING RETURNING 1),
        upd AS (
          UPDATE company_fact_filings f SET primary_document=r.primary_document,is_xbrl=r.is_xbrl,updated_at=now()
          FROM r JOIN cur ON cur.accession=r.accession
          WHERE f.id=cur.id AND cur.filed=r.filed AND cur.form=r.form AND cur.report_date IS NOT DISTINCT FROM r.report_date
            AND (cur.primary_document,cur.is_xbrl) IS DISTINCT FROM (r.primary_document,r.is_xbrl)
          RETURNING 1),
        conf AS (
          INSERT INTO company_fact_conflicts (id,kind,cik,accession,target_key,stored,proposed,proposed_digest,first_seen_run_id,last_seen_run_id)
          SELECT gen_random_uuid(),'filing',r.cik,r.accession,r.accession,
                 jsonb_build_object('revision',cur.revision,'filed',cur.filed,'report_date',cur.report_date,'form',cur.form),
                 p.proposed, md5(cur.revision||':'||p.proposed::text), $2, $2
          FROM r JOIN cur ON cur.accession=r.accession
          CROSS JOIN LATERAL (SELECT jsonb_build_object('filed',r.filed,'report_date',r.report_date,'form',r.form) AS proposed) p
          WHERE cur.filed<>r.filed OR cur.form<>r.form OR cur.report_date IS DISTINCT FROM r.report_date
          {UPSERT_CONFLICT})
        SELECT (SELECT count(*) FROM ins)+(SELECT count(*) FROM upd) AS written, (SELECT count(*) FROM conf WHERE status='open') AS conflicts"))
        .bind::<Jsonb,_>(json!(rows)).bind::<SqlUuid,_>(run_id)
        .get_result(&mut *conn).await?)
}

/// Label and description per (taxonomy, concept), written only when new or different.
async fn persist_concepts(conn: &mut AsyncPgConnection, facts: &[CompanyFact]) -> Result<()> {
    let mut concepts = BTreeMap::new();
    for f in facts {
        if f.label.is_some() || f.description.is_some() {
            concepts
                .entry((&f.subject.taxonomy, &f.subject.concept))
                .or_insert((&f.label, &f.description));
        }
    }
    if concepts.is_empty() {
        return Ok(());
    }
    let rows: Vec<_> = concepts
        .into_iter()
        .map(|((taxonomy, concept), (label, description))| {
            json!({"taxonomy":taxonomy,"concept":concept,"label":label,"description":description})
        })
        .collect();
    diesel::sql_query("INSERT INTO company_fact_concepts (taxonomy,concept,label,description)
        SELECT taxonomy,concept,label,description FROM jsonb_to_recordset($1) AS r(taxonomy text,concept text,label text,description text)
        ON CONFLICT (taxonomy,concept) DO UPDATE SET label=EXCLUDED.label,description=EXCLUDED.description,updated_at=now()
        WHERE (company_fact_concepts.label,company_fact_concepts.description) IS DISTINCT FROM (EXCLUDED.label,EXCLUDED.description)")
        .bind::<Jsonb, _>(json!(rows))
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// The archive listed one revision key twice with different values; neither row was loaded.
async fn record_source_duplicates(
    conn: &mut AsyncPgConnection,
    run_id: Uuid,
    facts: &ParsedCompanyFacts,
    subs: &ParsedSubmissions,
) -> Result<i64> {
    let fact_value =
        |f: &CompanyFact| json!({"value":f.value.to_string(),"filed":f.filed,"form":f.form});
    let filing_value =
        |f: &SubmissionFiling| json!({"filed":f.filed,"report_date":f.report_date,"form":f.form});
    let rows: Vec<_> = facts
        .duplicates
        .iter()
        .map(|(a, b)| {
            json!({"kind":"fact_source_duplicate","accession":a.accession,"target_key":a.id.to_string(),
                "stored":fact_value(a),"proposed":fact_value(b)})
        })
        .chain(subs.duplicates.iter().map(|(a, b)| {
            json!({"kind":"filing_source_duplicate","accession":a.accession,"target_key":a.accession,
                "stored":filing_value(a),"proposed":filing_value(b)})
        }))
        .collect();
    if rows.is_empty() {
        return Ok(0);
    }
    #[derive(QueryableByName)]
    struct Count {
        #[diesel(sql_type = BigInt)]
        conflicts: i64,
    }
    let count: Count = diesel::sql_query(format!("WITH conf AS (
          INSERT INTO company_fact_conflicts (id,kind,cik,accession,target_key,stored,proposed,proposed_digest,first_seen_run_id,last_seen_run_id)
          SELECT gen_random_uuid(),kind,$2,accession,target_key,stored,proposed,md5(proposed::text),$3,$3
          FROM jsonb_to_recordset($1) AS r(kind text,accession text,target_key text,stored jsonb,proposed jsonb)
          {UPSERT_CONFLICT})
        SELECT count(*) AS conflicts FROM conf WHERE status='open'"))
        .bind::<Jsonb, _>(json!(rows))
        .bind::<Text, _>(&facts.cik)
        .bind::<SqlUuid, _>(run_id)
        .get_result(&mut *conn)
        .await?;
    Ok(count.conflicts)
}

#[derive(Debug, Clone, Serialize, QueryableByName)]
pub struct Conflict {
    #[diesel(sql_type = SqlUuid)]
    pub id: Uuid,
    #[diesel(sql_type = Text)]
    pub kind: String,
    #[diesel(sql_type = Text)]
    pub cik: String,
    #[diesel(sql_type = Text)]
    pub accession: String,
    #[diesel(sql_type = Text)]
    pub target_key: String,
    #[diesel(sql_type = Jsonb)]
    pub stored: serde_json::Value,
    #[diesel(sql_type = Jsonb)]
    pub proposed: serde_json::Value,
    #[diesel(sql_type = Text)]
    pub status: String,
    #[diesel(sql_type = BigInt)]
    pub seen_count: i64,
    #[diesel(sql_type = Timestamptz)]
    pub first_seen_at: DateTime<Utc>,
    #[diesel(sql_type = Timestamptz)]
    pub last_seen_at: DateTime<Utc>,
    #[diesel(sql_type = Nullable<Text>)]
    pub resolved_by: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub resolution_note: Option<String>,
}

/// Conflicts with `status` (`open`, `accepted`, `rejected`), newest first, at most 1000.
pub async fn list_conflicts(conn: &mut AsyncPgConnection, status: &str) -> Result<Vec<Conflict>> {
    Ok(diesel::sql_query(
        "SELECT id,kind,cik::text AS cik,accession,target_key,stored,proposed,status,seen_count,first_seen_at,last_seen_at,resolved_by,resolution_note
         FROM company_fact_conflicts WHERE status=$1 ORDER BY last_seen_at DESC, id LIMIT 1000",
    )
    .bind::<Text, _>(status)
    .load(&mut *conn)
    .await?)
}

/// Operator path: accepts an open fact or filing conflict as the next revision of the stored
/// row. The stored row is not changed; later loads compare against the new revision. Refuses
/// if another revision was accepted since the conflict was recorded. Returns the revision id.
pub async fn accept_conflict(
    conn: &mut AsyncPgConnection,
    conflict_id: Uuid,
    operator: &str,
    note: Option<&str>,
) -> Result<Uuid> {
    let operator = operator.trim();
    if operator.is_empty() {
        bail!("an operator name is required");
    }
    conn.transaction::<_, anyhow::Error, _>(async move |conn| {
        let conflict = open_conflict(conn, conflict_id).await?;
        let stored_revision = conflict.stored.get("revision").and_then(serde_json::Value::as_i64)
            .context("conflict has no stored revision")?;
        #[derive(QueryableByName)]
        struct Target {
            #[diesel(sql_type = SqlUuid)]
            id: Uuid,
            #[diesel(sql_type = BigInt)]
            revision: i64,
        }
        let (target_sql, insert_sql) = match conflict.kind.as_str() {
            "fact" => (
                "SELECT f.id, coalesce(max(x.revision),1)::bigint AS revision FROM company_facts f
                 LEFT JOIN company_fact_revisions x ON x.fact_id=f.id WHERE f.id=$1::uuid AND f.cik=$2 GROUP BY f.id",
                "INSERT INTO company_fact_revisions (id,fact_id,revision,value,filed,form,conflict_id,accepted_by,note)
                 SELECT $1,$2,$3,(p->>'value')::numeric,(p->>'filed')::date,p->>'form',$4,$5,$6 FROM (SELECT $7::jsonb AS p) s",
            ),
            "filing" => (
                "SELECT f.id, coalesce(max(x.revision),1)::bigint AS revision FROM company_fact_filings f
                 LEFT JOIN company_fact_filing_revisions x ON x.filing_id=f.id WHERE f.accession=$1 AND f.cik=$2 GROUP BY f.id",
                "INSERT INTO company_fact_filing_revisions (id,filing_id,revision,filed,report_date,form,conflict_id,accepted_by,note)
                 SELECT $1,$2,$3,(p->>'filed')::date,(p->>'report_date')::date,p->>'form',$4,$5,$6 FROM (SELECT $7::jsonb AS p) s",
            ),
            _ => bail!("the archive contradicts itself for this key; reject the conflict or wait for a corrected archive"),
        };
        let target: Target = diesel::sql_query(target_sql)
            .bind::<Text, _>(&conflict.target_key).bind::<Text, _>(&conflict.cik)
            .get_result(&mut *conn).await.context("stored row for this conflict is missing")?;
        if target.revision != stored_revision {
            bail!("revision {} was accepted after this conflict was recorded; rerun the load to compare against it", target.revision);
        }
        let revision = i32::try_from(target.revision + 1)?;
        let id = Uuid::new_v5(&Uuid::NAMESPACE_URL,
            format!("sec-companyfacts:{}-revision:v1:{}:{revision}", conflict.kind, target.id).as_bytes());
        diesel::sql_query(insert_sql)
            .bind::<SqlUuid, _>(id).bind::<SqlUuid, _>(target.id).bind::<diesel::sql_types::Integer, _>(revision)
            .bind::<SqlUuid, _>(conflict_id).bind::<Text, _>(operator).bind::<Nullable<Text>, _>(note)
            .bind::<Jsonb, _>(&conflict.proposed)
            .execute(&mut *conn).await?;
        resolve(conn, conflict_id, "accepted", operator, note).await?;
        Ok(id)
    }).await
}

/// Operator path: keeps the stored row and stops counting this proposal as a conflict.
pub async fn reject_conflict(
    conn: &mut AsyncPgConnection,
    conflict_id: Uuid,
    operator: &str,
    note: Option<&str>,
) -> Result<()> {
    let operator = operator.trim();
    if operator.is_empty() {
        bail!("an operator name is required");
    }
    conn.transaction::<_, anyhow::Error, _>(async move |conn| {
        open_conflict(conn, conflict_id).await?;
        resolve(conn, conflict_id, "rejected", operator, note).await
    })
    .await
}

async fn open_conflict(conn: &mut AsyncPgConnection, id: Uuid) -> Result<Conflict> {
    let conflict: Conflict = diesel::sql_query(
        "SELECT id,kind,cik::text AS cik,accession,target_key,stored,proposed,status,seen_count,first_seen_at,last_seen_at,resolved_by,resolution_note
         FROM company_fact_conflicts WHERE id=$1 FOR UPDATE",
    )
    .bind::<SqlUuid, _>(id)
    .get_result(&mut *conn)
    .await
    .optional()?
    .context("no such companyfacts conflict")?;
    if conflict.status != "open" {
        bail!("conflict is already {}", conflict.status);
    }
    Ok(conflict)
}

async fn resolve(
    conn: &mut AsyncPgConnection,
    id: Uuid,
    status: &str,
    operator: &str,
    note: Option<&str>,
) -> Result<()> {
    diesel::sql_query("UPDATE company_fact_conflicts SET status=$2,resolved_at=now(),resolved_by=$3,resolution_note=$4 WHERE id=$1")
        .bind::<SqlUuid, _>(id)
        .bind::<Text, _>(status)
        .bind::<Text, _>(operator)
        .bind::<Nullable<Text>, _>(note)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// What the crawler-worker exports as `crawler_companyfacts_*` gauges.
#[derive(Debug, Clone, Default, PartialEq, QueryableByName)]
pub struct GaugeSnapshot {
    /// The newest finished run, or a running one older than the 8-hour Job deadline.
    #[diesel(sql_type = Nullable<Timestamptz>)]
    pub last_run_at: Option<DateTime<Utc>>,
    #[diesel(sql_type = Bool)]
    pub last_run_failed: bool,
    #[diesel(sql_type = BigInt)]
    pub last_run_conflicts: i64,
    #[diesel(sql_type = BigInt)]
    pub last_run_fact_rows_written: i64,
    #[diesel(sql_type = BigInt)]
    pub last_run_filing_rows_written: i64,
    #[diesel(sql_type = Bool)]
    pub last_run_archive_stale: bool,
    /// Completion of the newest successful downloaded run.
    #[diesel(sql_type = Nullable<Timestamptz>)]
    pub last_success_at: Option<DateTime<Utc>>,
    #[diesel(sql_type = BigInt)]
    pub open_conflicts: i64,
}

pub async fn gauge_snapshot(pool: &DatabasePool) -> Result<GaugeSnapshot> {
    let mut conn = pool.get().await?;
    Ok(diesel::sql_query(
        "WITH last AS (
           SELECT coalesce(completed_at, started_at) AS at, status, conflicts_count, fact_rows_written, filing_rows_written, archive_stale
           FROM company_fact_load_runs
           WHERE status <> 'running' OR started_at < now() - INTERVAL '8 hours'
           ORDER BY coalesce(completed_at, started_at) DESC LIMIT 1)
         SELECT (SELECT at FROM last) AS last_run_at,
                coalesce((SELECT status IN ('failed','running') FROM last), false) AS last_run_failed,
                coalesce((SELECT conflicts_count FROM last), 0) AS last_run_conflicts,
                coalesce((SELECT fact_rows_written FROM last), 0) AS last_run_fact_rows_written,
                coalesce((SELECT filing_rows_written FROM last), 0) AS last_run_filing_rows_written,
                coalesce((SELECT archive_stale FROM last), false) AS last_run_archive_stale,
                (SELECT max(completed_at) FROM company_fact_load_runs
                  WHERE status IN ('succeeded','succeeded_with_conflicts') AND source_mode='downloaded') AS last_success_at,
                (SELECT count(*) FROM company_fact_conflicts WHERE status='open') AS open_conflicts",
    )
    .get_result(&mut conn)
    .await?)
}
