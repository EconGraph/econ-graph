//! Bounded, source-preserving SEC company research reads on the data database.
//! Caller authorization belongs to the API boundary; these helpers never open the app DB.
use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use diesel::{
    prelude::*,
    sql_types::{BigInt, Bool, Date, Integer, Nullable, Numeric, Text, Uuid as SqlUuid},
};
use diesel_async::RunQueryDsl;
use econ_graph_core::{
    database::DatabasePool,
    error::{AppError, AppResult},
    models::{Company, CompanyFact, CompanyFactSubject, ResearchPeriod},
    schema::companies,
};

/// A query's source revision choice. Filtering happens before paging.
#[derive(Debug, Clone, Default)]
pub enum CompanyRevision {
    #[default]
    Latest,
    Original,
    AsOf(NaiveDate),
    Accession(String),
}

#[derive(Debug, Clone, Default)]
pub struct CompanyFactFilter {
    pub taxonomy: Option<String>,
    pub concept: Option<String>,
    pub unit: Option<String>,
    pub period_start: Option<NaiveDate>,
    pub period_end: Option<NaiveDate>,
    pub instant_only: bool,
    pub include_revisions: bool,
    pub revision: CompanyRevision,
}

/// SEC identifiers are ten digits; accept common unpadded search input.
pub fn canonical_cik(raw: &str) -> AppResult<String> {
    if raw.is_empty()
        || raw.len() > 10
        || !raw.bytes().all(|b| b.is_ascii_digit())
        || raw.bytes().all(|b| b == b'0')
    {
        return Err(AppError::Validation(
            "CIK must contain one to ten digits and be nonzero".into(),
        ));
    }
    Ok(format!("{raw:0>10}"))
}

pub async fn list_companies(
    pool: &DatabasePool,
    search: Option<&str>,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<Company>> {
    if !(1..=101).contains(&limit) || !(0..=1_000_000).contains(&offset) {
        return Err(AppError::Validation(
            "Company page is outside supported bounds".into(),
        ));
    }
    let search = search.unwrap_or("").trim();
    if search.len() > 200 {
        return Err(AppError::Validation("Company search is too long".into()));
    }
    let mut conn = pool
        .get()
        .await
        .map_err(|e| AppError::DatabasePool(e.to_string()))?;
    let mut query = companies::table.into_boxed();
    if !search.is_empty() {
        // Literal substring search: wildcard characters in user input are never operators.
        let pattern = format!(
            "%{}%",
            search
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        query = query.filter(
            companies::name
                .ilike(pattern.clone())
                .or(companies::ticker.ilike(pattern))
                .or(companies::cik.eq(canonical_cik(search).unwrap_or_default())),
        );
    }
    // Discover all known XBRL issuers, including those without standardized facts.
    // The UI reports unavailable data explicitly rather than concealing the company.
    query = query.filter(diesel::dsl::sql::<Bool>(
        "(EXISTS (SELECT 1 FROM company_facts cf WHERE cf.cik = companies.cik) OR EXISTS (SELECT 1 FROM company_fact_filings filing WHERE filing.cik=companies.cik AND filing.is_xbrl))",
    ));
    Ok(query
        .order((companies::name.asc(), companies::cik.asc()))
        .limit(limit)
        .offset(offset)
        .select(Company::as_select())
        .load(&mut conn)
        .await?)
}

pub async fn company(pool: &DatabasePool, cik: &str) -> AppResult<Option<Company>> {
    let cik = canonical_cik(cik)?;
    let mut conn = pool
        .get()
        .await
        .map_err(|e| AppError::DatabasePool(e.to_string()))?;
    Ok(companies::table
        .filter(companies::cik.eq(cik))
        .select(Company::as_select())
        .first(&mut conn)
        .await
        .optional()?)
}

#[derive(Debug, QueryableByName)]
struct FactRow {
    #[diesel(sql_type = SqlUuid)]
    id: uuid::Uuid,
    #[diesel(sql_type = Text)]
    cik: String,
    #[diesel(sql_type = Text)]
    taxonomy: String,
    #[diesel(sql_type = Text)]
    concept: String,
    #[diesel(sql_type = Nullable<Date>)]
    period_start: Option<NaiveDate>,
    #[diesel(sql_type = Date)]
    period_end: NaiveDate,
    #[diesel(sql_type = Text)]
    unit: String,
    #[diesel(sql_type = Text)]
    accession: String,
    #[diesel(sql_type = Date)]
    filed: NaiveDate,
    #[diesel(sql_type = Numeric)]
    value: BigDecimal,
    #[diesel(sql_type = Text)]
    form: String,
    #[diesel(sql_type = Nullable<Integer>)]
    fiscal_year: Option<i32>,
    #[diesel(sql_type = Nullable<Text>)]
    fiscal_period: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    frame: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    label: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
}
impl From<FactRow> for CompanyFact {
    fn from(row: FactRow) -> Self {
        Self {
            id: row.id,
            subject: CompanyFactSubject {
                cik: row.cik,
                taxonomy: row.taxonomy,
                concept: row.concept,
                unit: row.unit,
                period: match row.period_start {
                    Some(start) => ResearchPeriod::Duration {
                        start,
                        end: row.period_end,
                    },
                    None => ResearchPeriod::Instant {
                        date: row.period_end,
                    },
                },
            },
            accession: row.accession,
            filed: row.filed,
            value: row.value,
            form: row.form,
            fiscal_year: row.fiscal_year,
            fiscal_period: row.fiscal_period,
            frame: row.frame,
            label: row.label,
            description: row.description,
        }
    }
}

/// Read selected revisions before pagination. Subject dates, units and taxonomy remain distinct.
/// This fixed SQL never interpolates caller text. LIMIT+1 callers can expose continuation.
pub async fn company_facts(
    pool: &DatabasePool,
    cik: &str,
    filter: &CompanyFactFilter,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<CompanyFact>> {
    if !(1..=100_001).contains(&limit) || !(0..=1_000_000).contains(&offset) {
        return Err(AppError::Validation(
            "Fact page is outside supported bounds".into(),
        ));
    }
    let cik = canonical_cik(cik)?;
    let (original, as_of, accession) = match &filter.revision {
        CompanyRevision::Latest => (false, None, None),
        CompanyRevision::Original => (true, None, None),
        CompanyRevision::AsOf(date) => (false, Some(*date), None),
        CompanyRevision::Accession(acc) => (false, None, Some(acc.clone())),
    };
    let mut conn = pool
        .get()
        .await
        .map_err(|e| AppError::DatabasePool(e.to_string()))?;
    let rows = diesel::sql_query("WITH ranked AS (
        SELECT cf.*, ROW_NUMBER() OVER (PARTITION BY cik,taxonomy,concept,unit,period_start,period_end
          ORDER BY CASE WHEN $9 THEN filed END ASC, CASE WHEN NOT $9 THEN filed END DESC,
                   CASE WHEN $9 THEN accession END ASC, CASE WHEN NOT $9 THEN accession END DESC, id) AS revision_rank
        FROM company_facts cf WHERE cik=$1
          AND ($2::text IS NULL OR taxonomy=$2) AND ($3::text IS NULL OR concept=$3)
          AND ($4::text IS NULL OR unit=$4) AND ($5::date IS NULL OR period_start=$5)
          AND ($6::date IS NULL OR period_end=$6) AND (NOT $7 OR period_start IS NULL)
          AND ($8::date IS NULL OR filed <= $8) AND ($10::text IS NULL OR accession=$10))
        SELECT id,cik,taxonomy,concept,period_start,period_end,unit,accession,filed,value,form,
               fiscal_year,fiscal_period,frame,label,description FROM ranked WHERE revision_rank=1 OR $13
        ORDER BY period_end DESC,period_start DESC NULLS LAST,taxonomy,concept,unit,id LIMIT $11 OFFSET $12")
        .bind::<Text,_>(cik).bind::<Nullable<Text>,_>(&filter.taxonomy).bind::<Nullable<Text>,_>(&filter.concept)
        .bind::<Nullable<Text>,_>(&filter.unit).bind::<Nullable<Date>,_>(filter.period_start).bind::<Nullable<Date>,_>(filter.period_end)
        .bind::<Bool,_>(filter.instant_only).bind::<Nullable<Date>,_>(as_of).bind::<Bool,_>(original).bind::<Nullable<Text>,_>(accession)
        .bind::<BigInt,_>(limit).bind::<BigInt,_>(offset).bind::<Bool,_>(filter.include_revisions).load::<FactRow>(&mut conn).await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// All selected facts for calculations, with an explicit cap rather than silent truncation.
pub async fn calculation_facts(pool: &DatabasePool, cik: &str) -> AppResult<Vec<CompanyFact>> {
    let cik = canonical_cik(cik)?;
    let mut conn = pool
        .get()
        .await
        .map_err(|e| AppError::DatabasePool(e.to_string()))?;
    let rows=diesel::sql_query("SELECT id,cik,taxonomy,concept,period_start,period_end,unit,accession,filed,value,form,fiscal_year,fiscal_period,frame,label,description FROM company_facts WHERE cik=$1 ORDER BY period_end,taxonomy,concept,unit,period_start,filed,accession,id LIMIT 100001")
      .bind::<Text,_>(cik).load::<FactRow>(&mut conn).await?;
    if rows.len() > 100_000 {
        return Err(AppError::Validation(
            "Company exceeds calculation evidence limit".into(),
        ));
    }
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn sic_peers(pool: &DatabasePool, cik: &str, limit: i64) -> AppResult<Vec<Company>> {
    let Some(subject) = company(pool, cik).await? else {
        return Ok(vec![]);
    };
    let Some(sic) = subject.sic_code else {
        return Ok(vec![]);
    };
    let mut conn = pool
        .get()
        .await
        .map_err(|e| AppError::DatabasePool(e.to_string()))?;
    Ok(companies::table
        .filter(companies::sic_code.eq(sic))
        .filter(companies::cik.ne(subject.cik))
        .filter(diesel::dsl::sql::<Bool>(
            "EXISTS (SELECT 1 FROM company_facts cf WHERE cf.cik=companies.cik)",
        ))
        .order(companies::cik.asc())
        .limit(limit.clamp(1, 20))
        .select(Company::as_select())
        .load(&mut conn)
        .await?)
}

#[derive(Debug, QueryableByName)]
struct Exists {
    #[diesel(sql_type=Bool)]
    exists: bool,
}

pub async fn filing_exists(pool: &DatabasePool, cik: &str, accession: &str) -> AppResult<bool> {
    let mut conn = pool
        .get()
        .await
        .map_err(|e| AppError::DatabasePool(e.to_string()))?;
    Ok(diesel::sql_query("SELECT EXISTS(SELECT 1 FROM company_facts WHERE cik=$1 AND accession=$2 UNION ALL SELECT 1 FROM company_fact_filings WHERE cik=$1 AND accession=$2 UNION ALL SELECT 1 FROM financial_statements f JOIN companies c ON c.id=f.company_id WHERE c.cik=$1 AND f.accession_number=$2) AS exists")
       .bind::<Text,_>(cik).bind::<Text,_>(accession).get_result::<Exists>(&mut conn).await?.exists)
}

pub async fn fact_exists(
    pool: &DatabasePool,
    subject: &CompanyFactSubject,
    accession: Option<&str>,
) -> AppResult<bool> {
    subject
        .validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;
    let (start, end) = match subject.period {
        ResearchPeriod::Instant { date } => (None, date),
        ResearchPeriod::Duration { start, end } => (Some(start), end),
    };
    let mut conn = pool
        .get()
        .await
        .map_err(|e| AppError::DatabasePool(e.to_string()))?;
    Ok(diesel::sql_query("SELECT EXISTS(SELECT 1 FROM company_facts WHERE cik=$1 AND taxonomy=$2 AND concept=$3 AND unit=$4 AND period_start IS NOT DISTINCT FROM $5 AND period_end=$6 AND ($7::text IS NULL OR accession=$7)) AS exists")
       .bind::<Text,_>(&subject.cik).bind::<Text,_>(&subject.taxonomy).bind::<Text,_>(&subject.concept).bind::<Text,_>(&subject.unit).bind::<Nullable<Date>,_>(start).bind::<Date,_>(end).bind::<Nullable<Text>,_>(accession).get_result::<Exists>(&mut conn).await?.exists)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cik_normalization_is_strict() {
        assert_eq!(canonical_cik("320193").unwrap(), "0000320193");
        for raw in ["", "0", "0000000000", "12345678901", " 320193", "1%", "1e2"] {
            assert!(canonical_cik(raw).is_err(), "{raw}");
        }
    }
}
