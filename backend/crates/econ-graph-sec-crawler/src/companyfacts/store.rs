use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Jsonb, Nullable, Text, Timestamptz, Uuid as SqlUuid},
};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use econ_graph_core::{database::DataPool, models::ResearchPeriod};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use super::parse::{ParsedCompanyFacts, ParsedSubmissions, SubmissionFiling};

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
pub async fn finish_run(
    conn: &mut AsyncPgConnection,
    id: Uuid,
    success: bool,
    counts: (i64, i64, i64),
    error: Option<String>,
    hashes: (Option<String>, Option<String>),
    modified: (Option<DateTime<Utc>>, Option<DateTime<Utc>>),
) -> Result<()> {
    diesel::sql_query("UPDATE company_fact_load_runs SET completed_at=now(),status=$2,companies_count=$3,facts_count=$4,filings_count=$5,error=$6,companyfacts_sha256=$7,submissions_sha256=$8,companyfacts_last_modified=$9,submissions_last_modified=$10 WHERE id=$1")
        .bind::<SqlUuid,_>(id).bind::<Text,_>(if success {"succeeded"}else{"failed"})
        .bind::<BigInt,_>(counts.0).bind::<BigInt,_>(counts.1).bind::<BigInt,_>(counts.2)
        .bind::<Nullable<Text>,_>(error).bind::<Nullable<Text>,_>(hashes.0).bind::<Nullable<Text>,_>(hashes.1)
        .bind::<Nullable<Timestamptz>,_>(modified.0)
        .bind::<Nullable<Timestamptz>,_>(modified.1)
        .execute(&mut *conn).await?;
    Ok(())
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
    let successful = runs
        .iter()
        .find(|run| run.status == "succeeded" && run.source_mode == "downloaded")
        .context("no verified downloaded companyfacts load has succeeded")?;
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

/// One company's metadata, filings and facts are committed together. Retrying a failed run
/// adds missing accessions without deleting any historical facts, notes, or parser records.
pub async fn persist_company(
    conn: &mut AsyncPgConnection,
    facts: ParsedCompanyFacts,
    subs: ParsedSubmissions,
) -> Result<(usize, usize)> {
    if facts.cik != subs.cik {
        bail!("company CIK mismatch");
    }
    conn.transaction::<_, anyhow::Error, _>(async move |conn| {
        use econ_graph_core::schema::companies::dsl as c;
        // Preserve the existing UUID and richer metadata other SEC entry points collected.
        diesel::insert_into(c::companies).values((c::cik.eq(&subs.cik), c::name.eq(&subs.name),
            c::ticker.eq(&subs.ticker),c::sic_code.eq(&subs.sic),c::sic_description.eq(&subs.sic_description),
            c::entity_type.eq(&subs.entity_type)))
            .on_conflict(c::cik).do_update().set((c::name.eq(&subs.name),c::ticker.eq(&subs.ticker),
                c::sic_code.eq(&subs.sic),c::sic_description.eq(&subs.sic_description),
                c::entity_type.eq(&subs.entity_type),c::updated_at.eq(Utc::now())))
            .execute(&mut *conn).await?;
        for chunk in subs.filings.chunks(500) { persist_filings(conn, chunk).await?; }
        for chunk in facts.facts.chunks(500) {
            let rows: Vec<_> = chunk.iter().map(|f| {
                let (start,end) = match f.subject.period { ResearchPeriod::Instant{date} => (None,date),
                    ResearchPeriod::Duration{start,end} => (Some(start),end) };
                json!({"id":f.id,"cik":f.subject.cik,"taxonomy":f.subject.taxonomy,"concept":f.subject.concept,
                    "period_start":start,"period_end":end,"unit":f.subject.unit,"accession":f.accession,
                    "filed":f.filed,"value":f.value.to_string(),"form":f.form,"fiscal_year":f.fiscal_year,
                    "fiscal_period":f.fiscal_period,"frame":f.frame,"label":f.label,"description":f.description})
            }).collect();
            let inserted = diesel::sql_query("INSERT INTO company_facts (id,cik,taxonomy,concept,period_start,period_end,unit,accession,filed,value,form,fiscal_year,fiscal_period,frame,label,description) SELECT id,cik,taxonomy,concept,period_start,period_end,unit,accession,filed,value::numeric,form,fiscal_year,fiscal_period,frame,label,description FROM jsonb_to_recordset($1) AS r(id uuid,cik text,taxonomy text,concept text,period_start date,period_end date,unit text,accession text,filed date,value text,form text,fiscal_year int,fiscal_period text,frame text,label text,description text) ON CONFLICT (id) DO UPDATE SET updated_at=now(),label=EXCLUDED.label,description=EXCLUDED.description WHERE company_facts.value=EXCLUDED.value AND company_facts.filed=EXCLUDED.filed AND company_facts.form=EXCLUDED.form")
                .bind::<Jsonb,_>(json!(rows)).execute(&mut *conn).await?;
            if inserted != chunk.len() { bail!("source changed an already stored accession fact; refusing silent replacement"); }
        }
        Ok((facts.facts.len(),subs.filings.len()))
    }).await
}
async fn persist_filings(conn: &mut AsyncPgConnection, rows: &[SubmissionFiling]) -> Result<()> {
    let inserted = diesel::sql_query("INSERT INTO company_fact_filings (id,cik,accession,filed,report_date,form,primary_document,is_xbrl) SELECT gen_random_uuid(),cik,accession,filed,report_date,form,primary_document,is_xbrl FROM jsonb_to_recordset($1) AS r(cik text,accession text,filed date,report_date date,form text,primary_document text,is_xbrl bool) ON CONFLICT (cik,accession) DO UPDATE SET updated_at=now(),primary_document=EXCLUDED.primary_document,is_xbrl=EXCLUDED.is_xbrl WHERE company_fact_filings.filed=EXCLUDED.filed AND company_fact_filings.form=EXCLUDED.form AND company_fact_filings.report_date IS NOT DISTINCT FROM EXCLUDED.report_date")
        .bind::<Jsonb,_>(json!(rows)).execute(&mut *conn).await?;
    if inserted != rows.len() {
        bail!("source changed an already stored filing; refusing silent replacement");
    }
    Ok(())
}
