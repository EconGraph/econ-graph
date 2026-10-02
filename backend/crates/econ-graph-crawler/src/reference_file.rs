// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! A source's own reference files (code lists, state lists, ...): refreshed from the source on
//! every scheduled crawl, and seeded into a new database by a SQL migration recorded from a real
//! download. One path for both.
//!
//! # Refresh
//!
//! An adapter lists the files that back a dataset dimension's `codes` in
//! [`SourceAdapter::code_lists`](crate::adapter::SourceAdapter::code_lists); the default
//! [`refresh_reference_data`](crate::adapter::SourceAdapter::refresh_reference_data) runs
//! [`refresh_code_list`] on each. That is [`refresh`]: a conditional GET with the `ETag` stored
//! in `reference_file_cache`. `304` keeps what is stored; `200` goes through the parser, the
//! labels are merged into the dataset, and the new `ETag` is stored. The `ETag` is stored only
//! once the labels landed, so a body that fails to parse, or arrives before its dataset row
//! exists, is fetched again next time.
//!
//! # Seed
//!
//! A new database gets usable labels before the first crawl from a seed migration,
//! `backend/migrations/*_seed_{source}_reference_codes/up.sql`, written by
//! `crawler record-reference-seeds --source SOURCE` ([`seed_migration_sql`]) from a real download
//! parsed by the same [`CodeList::parse`]. It calls the SQL function `seed_reference_codes` once
//! per file, which does nothing when `reference_file_cache` already has a row for that URL (this
//! database has crawled or been seeded with it), and otherwise merges the labels into the
//! dataset's dimension and stores the `ETag` the file was downloaded with. The first refresh then
//! sends that `ETag`, so only a file the source changed since the recording is downloaded again.
//! A seed may create the dataset row with just that dimension; the worker's `sync_datasets` fills
//! in the rest from `data/datasets/{source}.toml` and keeps the seeded codes.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use econ_graph_core::models::{Code, DatasetComponent};

use crate::adapter::CrawlCtx;
use crate::dataset::DatasetCatalog;
use crate::error::CrawlError;
use crate::http::{ConditionalText, HttpFetcher};
use crate::persist;
use crate::source::SourceId;

/// What [`refresh`]'s `apply` returns for one body: parse it and store what the adapter needs.
/// `Ok(false)` means there was nowhere to store it (e.g. the dataset row doesn't exist yet), so
/// the `ETag` is not stored and the next refresh downloads the file again.
pub type Apply<'a> = Pin<Box<dyn Future<Output = Result<bool, CrawlError>> + Send + 'a>>;

/// Parses one of a source's code-list files into `(code, label)` pairs.
pub type ParseLabels = Arc<dyn Fn(&str) -> Result<Vec<(String, String)>, CrawlError> + Send + Sync>;

/// One reference file of a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceFile<'a> {
    /// The source that publishes it (the `reference_file_cache` key, with `url`).
    pub source: SourceId,
    /// Where the source publishes it.
    pub url: &'a str,
}

/// A source's file of codes and labels for one dimension of one of its datasets.
#[derive(Clone)]
pub struct CodeList {
    /// Where the source publishes it.
    pub url: String,
    /// Dataset code (as in `data/datasets/{source}.toml`).
    pub dataset: &'static str,
    /// Dimension name within the dataset.
    pub dimension: &'static str,
    /// Its parser, shared by the refresh and the seed recorder.
    pub parse: ParseLabels,
}

impl fmt::Debug for CodeList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CodeList")
            .field("url", &self.url)
            .field("dataset", &self.dataset)
            .field("dimension", &self.dimension)
            .finish_non_exhaustive()
    }
}

fn db_err(source: SourceId, url: &str) -> impl Fn(econ_graph_core::error::AppError) -> CrawlError {
    let what = format!("{source} reference file {url}");
    move |e| CrawlError::Transient(format!("{what}: {e}"))
}

/// Downloads `file` with a conditional GET on its stored `ETag` and, if it changed, hands the
/// body to `apply` and stores the new `ETag` (see the module docs). `apply` gets the body by
/// value so the future it returns borrows nothing from `refresh`.
///
/// Returns whether anything was applied.
pub async fn refresh<'a, F>(
    ctx: &CrawlCtx,
    file: &ReferenceFile<'_>,
    mut apply: F,
) -> Result<bool, CrawlError>
where
    F: FnMut(String) -> Apply<'a>,
{
    let db_err = db_err(file.source, file.url);
    let etag = persist::reference_file_etag(&ctx.pool, file.source, file.url)
        .await
        .map_err(&db_err)?;
    let ConditionalText::Modified { body, etag } = ctx
        .http
        .get_text_conditional(file.source, file.url, etag.as_deref())
        .await?
    else {
        return Ok(false);
    };
    if !apply(body).await? {
        return Ok(false);
    }
    persist::set_reference_file_etag(&ctx.pool, file.source, file.url, etag.as_deref())
        .await
        .map_err(&db_err)?;
    Ok(true)
}

/// [`refresh`] for one code list: a changed file's labels are merged into its dataset's
/// dimension ([`persist::merge_dataset_dimension_codes`]).
pub async fn refresh_code_list(
    ctx: &CrawlCtx,
    source: SourceId,
    list: &CodeList,
) -> Result<bool, CrawlError> {
    let file = ReferenceFile {
        source,
        url: &list.url,
    };
    refresh(ctx, &file, |body| {
        let parse = Arc::clone(&list.parse);
        let db_err = db_err(source, &list.url);
        Box::pin(async move {
            let labels = non_empty(&list.url, parse(&body)?)?;
            persist::merge_dataset_dimension_codes(
                &ctx.pool,
                source,
                list.dataset,
                list.dimension,
                &labels,
            )
            .await
            .map_err(db_err)
        })
    })
    .await
}

/// `labels`, or a `Parse` error if a file parsed to none: an empty code list is a broken
/// download (a header-only or truncated file), never a source that has no codes.
fn non_empty(
    url: &str,
    labels: Vec<(String, String)>,
) -> Result<Vec<(String, String)>, CrawlError> {
    if labels.is_empty() {
        return Err(CrawlError::Parse(format!("{url}: no codes")));
    }
    Ok(labels)
}

/// [`refresh_code_list`] for each of `lists`, all attempted even when one fails; the error names
/// every file that failed.
pub async fn refresh_code_lists(
    ctx: &CrawlCtx,
    source: SourceId,
    lists: &[CodeList],
) -> Result<(), CrawlError> {
    let mut errors = Vec::new();
    for list in lists {
        if let Err(e) = refresh_code_list(ctx, source, list).await {
            errors.push(format!("{}: {e}", list.url));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(CrawlError::Transient(format!(
            "{source} reference data: {}",
            errors.join("; ")
        )))
    }
}

/// One downloaded code list, ready to be written into a seed migration.
#[derive(Debug, Clone, PartialEq)]
pub struct SeedEntry {
    /// Dataset code.
    pub dataset: String,
    /// The dimension as `data/datasets/{source}.toml` declares it, without codes.
    pub dimension: DatasetComponent,
    /// Where the file was downloaded from.
    pub url: String,
    /// The `ETag` it was served with, if any.
    pub etag: Option<String>,
    /// Its codes, sorted by code.
    pub codes: Vec<Code>,
}

/// Downloads each of `lists` (unconditionally), parses it and pairs it with its dimension from
/// `catalog`, for [`seed_migration_sql`].
pub async fn download_seed_entries(
    http: &HttpFetcher,
    source: SourceId,
    lists: &[CodeList],
    catalog: &DatasetCatalog,
) -> Result<Vec<SeedEntry>, CrawlError> {
    let mut out = Vec::with_capacity(lists.len());
    for list in lists {
        let dimension = catalog
            .get(source, list.dataset)
            .and_then(|d| d.dimensions.iter().find(|c| c.name == list.dimension))
            .ok_or_else(|| {
                CrawlError::Permanent(format!(
                    "{source} has no dataset {} with a dimension {}",
                    list.dataset, list.dimension
                ))
            })?;
        let ConditionalText::Modified { body, etag } =
            http.get_text_conditional(source, &list.url, None).await?
        else {
            return Err(CrawlError::Transient(format!(
                "{}: 304 to an unconditional request",
                list.url
            )));
        };
        let mut codes: Vec<Code> = non_empty(&list.url, (list.parse)(&body)?)?
            .into_iter()
            .map(|(code, label)| Code::new(&code, &label))
            .collect();
        codes.sort_by(|a, b| a.code.cmp(&b.code));
        codes.dedup_by(|a, b| a.code == b.code);
        let mut dimension = DatasetComponent::from(dimension);
        dimension.codes = None;
        dimension.codelist = None;
        out.push(SeedEntry {
            dataset: list.dataset.to_string(),
            dimension,
            url: list.url.clone(),
            etag,
            codes,
        });
    }
    Ok(out)
}

/// A SQL string literal.
fn sql_literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn sql_opt(s: Option<&str>) -> String {
    s.map_or_else(|| "NULL".to_string(), sql_literal)
}

/// The `up.sql` and `down.sql` of a seed migration for `source` holding `entries`, recorded at
/// `recorded_at`. `up.sql` makes sure the data source row exists and calls
/// `seed_reference_codes` once per entry; `down.sql` forgets the seeded `ETag`s (the labels stay,
/// as a crawl would have left them).
pub fn seed_migration_sql(
    source: SourceId,
    recorded_at: DateTime<Utc>,
    entries: &[SeedEntry],
) -> Result<(String, String), CrawlError> {
    let ds = persist::data_source_template(source);
    let mut up = format!(
        "-- Initial {source} reference codes for a new database, recorded {} by\n\
         -- `crawler record-reference-seeds --source {source}` from the source's own files. Do not\n\
         -- edit: re-record instead. Each call does nothing when this database already has the\n\
         -- file (see backend/crates/econ-graph-crawler/src/reference_file.rs); the crawl\n\
         -- refreshes it from there.\n\n\
         -- The literals below are standard SQL strings: backslashes in labels are literal.\n\
         SET LOCAL standard_conforming_strings = on;\n\n\
         INSERT INTO data_sources (name, description, base_url, api_key_required,\n    \
         rate_limit_per_minute, is_visible, is_enabled, requires_admin_approval,\n    \
         crawl_frequency_hours, api_documentation_url, api_key_name)\n\
         VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})\n\
         ON CONFLICT (name) DO NOTHING;\n",
        recorded_at.format("%Y-%m-%dT%H:%M:%SZ"),
        sql_literal(&ds.name),
        sql_opt(ds.description.as_deref()),
        sql_literal(&ds.base_url),
        ds.api_key_required,
        ds.rate_limit_per_minute,
        ds.is_visible,
        ds.is_enabled,
        ds.requires_admin_approval,
        ds.crawl_frequency_hours,
        sql_opt(ds.api_documentation_url.as_deref()),
        sql_opt(ds.api_key_name.as_deref()),
    );
    let mut down = String::from(
        "-- Forgets the seeded ETags, so the next crawl downloads these files again.\n",
    );
    for e in entries {
        let json_err = |err: serde_json::Error| CrawlError::Permanent(format!("{}: {err}", e.url));
        let dimension = serde_json::to_string(&e.dimension).map_err(json_err)?;
        let codes = serde_json::to_string(&e.codes).map_err(json_err)?;
        up.push_str(&format!(
            "\nSELECT seed_reference_codes(\n    {},\n    {},\n    {}::jsonb,\n    {},\n    {},\n    {}::jsonb\n);\n",
            sql_literal(&ds.name),
            sql_literal(&e.dataset),
            sql_literal(&dimension),
            sql_literal(&e.url),
            sql_opt(e.etag.as_deref()),
            sql_literal(&codes),
        ));
        down.push_str(&format!(
            "DELETE FROM reference_file_cache\nWHERE url = {} AND etag IS NOT DISTINCT FROM {}\n    \
             AND source_id = (SELECT id FROM data_sources WHERE name = {});\n",
            sql_literal(&e.url),
            sql_opt(e.etag.as_deref()),
            sql_literal(&ds.name),
        ));
    }
    Ok((up, down))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::SourceAdapter as _;
    use crate::persist::stable_id_tests::{database_url, FreshDb};
    use crate::sources::census::{CensusAdapter, STATES_URL};
    use crate::testkit::{test_ctx, MockSource, Reply, Route};
    use diesel_async::SimpleAsyncConnection;

    const STATES: &str = include_str!("../tests/fixtures/census/state.txt");
    const URL_PATH: &str = "/codes.txt";

    fn census(mock: &MockSource) -> CensusAdapter {
        CensusAdapter::new(mock.base_url()).with_states_url(mock.url(URL_PATH))
    }

    fn catalog(adapter: &dyn crate::adapter::SourceAdapter) -> DatasetCatalog {
        let mut catalog = DatasetCatalog::empty();
        catalog.load_adapter(adapter).unwrap();
        catalog
    }

    /// `(code, label)` of the `bds` dataset's `state` dimension, `None` without a row.
    async fn state_codes(pool: &econ_graph_core::DatabasePool) -> Option<Vec<(String, String)>> {
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::datasets::dsl;
        let mut conn = pool.get().await.unwrap();
        let dims: Option<econ_graph_core::models::DatasetComponents> = dsl::datasets
            .filter(dsl::code.eq("bds"))
            .select(dsl::dimensions)
            .first(&mut conn)
            .await
            .optional()
            .unwrap();
        let dim = dims?.0.into_iter().find(|d| d.name == "state")?;
        Some(
            dim.codes
                .unwrap_or_default()
                .into_iter()
                .map(|c| (c.code, c.label))
                .collect(),
        )
    }

    /// Runs `sql` in a transaction, as Diesel runs a migration.
    async fn execute(pool: &econ_graph_core::DatabasePool, sql: &str) {
        pool.get()
            .await
            .unwrap()
            .batch_execute(&format!("BEGIN;\n{sql}\nCOMMIT;"))
            .await
            .unwrap();
    }

    #[test]
    fn sql_literals_escape_quotes() {
        assert_eq!(sql_literal("O'Brien"), "'O''Brien'");
        assert_eq!(sql_opt(None), "NULL");
        assert_eq!(sql_opt(Some("\"e\"")), "'\"e\"'");
    }

    #[test]
    fn seed_migration_names_the_source_and_each_file() {
        let entry = SeedEntry {
            dataset: "bds".into(),
            dimension: DatasetComponent {
                name: "state".into(),
                label: "State".into(),
                component_type: econ_graph_core::models::ComponentType::String,
                unit: None,
                codes: None,
                codelist: None,
            },
            url: STATES_URL.into(),
            etag: Some("\"abc\"".into()),
            codes: vec![Code::new("01", "Alabama")],
        };
        let at = "2026-10-02T03:00:00Z".parse().unwrap();
        let (up, down) = seed_migration_sql(SourceId::Census, at, &[entry]).unwrap();
        assert!(up.contains("recorded 2026-10-02T03:00:00Z"), "{up}");
        assert!(up.contains("ON CONFLICT (name) DO NOTHING"), "{up}");
        assert!(up.contains("SELECT seed_reference_codes("), "{up}");
        assert!(up.contains(&format!("'{STATES_URL}'")), "{up}");
        assert!(up.contains("'\"abc\"'"), "{up}");
        assert!(up.contains(r#"[{"code":"01","label":"Alabama"}]"#), "{up}");
        assert!(down.contains("DELETE FROM reference_file_cache"), "{down}");
    }

    /// The whole path: record a seed from a download, apply it to a new database, sync the
    /// datasets, then crawl. The seed's labels and ETag land; a second application and an
    /// already-crawled database are left alone; `sync_datasets` keeps the seeded codes; the
    /// crawl's first refresh sends the seeded ETag and a `304` changes nothing.
    #[tokio::test]
    async fn recorded_seed_loads_once_and_the_crawl_refreshes_from_it() {
        let Some(admin_url) = database_url() else {
            return;
        };
        let db = FreshDb::create(&admin_url, "econgraph_reference_seed").await;
        let mock = MockSource::start().await;
        let adapter = census(&mock);
        let catalog = catalog(&adapter);
        mock.mount(
            &Route::get(URL_PATH),
            Reply::text(STATES).header("ETag", "\"v1\""),
        )
        .await;

        let dir = std::env::temp_dir().join(format!("econgraph-seed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let at = "2026-10-02T03:00:00Z".parse().unwrap();
        let out =
            crate::cli::record_reference_seeds(&adapter, &test_ctx().http, &catalog, &dir, at)
                .await
                .unwrap();
        assert!(out.contains("bds.state: 51 codes"), "{out}");
        // Recording again replaces the earlier migration rather than adding a second one,
        // whose seed would never load behind the first.
        let later = "2026-10-02T04:00:00Z".parse().unwrap();
        let out =
            crate::cli::record_reference_seeds(&adapter, &test_ctx().http, &catalog, &dir, later)
                .await
                .unwrap();
        assert!(out.contains("replaced"), "{out}");
        let dirs: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(dirs, ["2026-10-02-040000_seed_census_reference_codes"]);
        let migration = dir.join(&dirs[0]);
        let up = std::fs::read_to_string(migration.join("up.sql")).unwrap();
        let down = std::fs::read_to_string(migration.join("down.sql")).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();

        // DB init: no dataset row yet. The seed creates it with the state labels.
        assert_eq!(state_codes(&db.pool).await, None);
        execute(&db.pool, &up).await;
        let seeded = state_codes(&db.pool).await.unwrap();
        assert_eq!(seeded.len(), 51);
        assert!(seeded.contains(&("06".into(), "California".into())));
        let url = mock.url(URL_PATH);
        assert_eq!(
            persist::reference_file_etag(&db.pool, SourceId::Census, &url)
                .await
                .unwrap()
                .as_deref(),
            Some("\"v1\"")
        );

        // Applying it again does nothing, even over labels the crawl changed since.
        persist::merge_dataset_dimension_codes(
            &db.pool,
            SourceId::Census,
            "bds",
            "state",
            &[("06".into(), "California (crawled)".into())],
        )
        .await
        .unwrap();
        execute(&db.pool, &up).await;
        assert!(state_codes(&db.pool)
            .await
            .unwrap()
            .contains(&("06".into(), "California (crawled)".into())));

        // The worker's dataset sync fills in the rest and keeps the codes.
        persist::sync_datasets(&db.pool, &catalog).await.unwrap();
        assert_eq!(state_codes(&db.pool).await.unwrap().len(), 51);

        // The first crawl sends the seeded ETag; unchanged, so nothing is applied.
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path(URL_PATH))
            .and(wiremock::matchers::header("If-None-Match", "\"v1\""))
            .respond_with(wiremock::ResponseTemplate::new(304))
            .with_priority(1)
            .mount(mock.server())
            .await;
        let mut ctx = test_ctx();
        ctx.pool = db.pool.clone();
        let list = &adapter.code_lists()[0];
        assert!(!refresh_code_list(&ctx, SourceId::Census, list)
            .await
            .unwrap());

        // down.sql forgets the seeded ETag, so the next crawl downloads the file again.
        execute(&db.pool, &down).await;
        assert_eq!(
            persist::reference_file_etag(&db.pool, SourceId::Census, &url)
                .await
                .unwrap(),
            None
        );
        assert!(refresh_code_list(&ctx, SourceId::Census, list)
            .await
            .unwrap());

        db.drop().await;
    }

    /// A seed into a dataset that already has the dimension adds only the codes it lacks.
    #[tokio::test]
    async fn seed_adds_missing_codes_to_an_existing_dimension() {
        let Some(admin_url) = database_url() else {
            return;
        };
        let db = FreshDb::create(&admin_url, "econgraph_reference_seed_merge").await;
        let mock = MockSource::start().await;
        let adapter = census(&mock);
        persist::sync_datasets(&db.pool, &catalog(&adapter))
            .await
            .unwrap();
        persist::merge_dataset_dimension_codes(
            &db.pool,
            SourceId::Census,
            "bds",
            "state",
            &[("06".into(), "Kept".into())],
        )
        .await
        .unwrap();
        execute(
            &db.pool,
            r#"SELECT seed_reference_codes('U.S. Census Bureau', 'bds',
                '{"name":"state","label":"State","type":"string"}'::jsonb,
                'https://example.test/state.txt', NULL,
                '[{"code":"01","label":"Alabama"},{"code":"06","label":"California"}]'::jsonb)"#,
        )
        .await;
        let codes = state_codes(&db.pool).await.unwrap();
        assert!(codes.contains(&("06".into(), "Kept".into())), "{codes:?}");
        assert!(
            codes.contains(&("01".into(), "Alabama".into())),
            "{codes:?}"
        );
        assert_eq!(codes.len(), 2);
        db.drop().await;
    }

    /// `apply` failing or storing nothing (`Ok(false)`) stores no ETag, so the file is downloaded
    /// again next time; a stored ETag makes the next request conditional.
    #[tokio::test]
    async fn etag_is_stored_only_after_the_body_was_stored() {
        let Some(admin_url) = database_url() else {
            return;
        };
        let db = FreshDb::create(&admin_url, "econgraph_reference_refresh").await;
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get(URL_PATH),
            Reply::text("LIVE").header("ETag", "\"live\""),
        )
        .await;
        let url = mock.url(URL_PATH);
        let file = ReferenceFile {
            source: SourceId::Census,
            url: &url,
        };
        let mut ctx = test_ctx();
        ctx.pool = db.pool.clone();
        let etag = || persist::reference_file_etag(&db.pool, SourceId::Census, &url);

        let e = refresh(&ctx, &file, |_| {
            Box::pin(async { Err(CrawlError::Parse("bad".into())) })
        })
        .await
        .unwrap_err();
        assert_eq!(e.kind(), "parse");
        assert_eq!(etag().await.unwrap(), None);

        assert!(!refresh(&ctx, &file, |_| Box::pin(async { Ok(false) }))
            .await
            .unwrap());
        assert_eq!(etag().await.unwrap(), None);

        let mut seen = Vec::new();
        assert!(refresh(&ctx, &file, |body| {
            seen.push(body);
            Box::pin(async { Ok(true) })
        })
        .await
        .unwrap());
        assert_eq!(seen, ["LIVE"]);
        assert_eq!(etag().await.unwrap().as_deref(), Some("\"live\""));

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path(URL_PATH))
            .and(wiremock::matchers::header("If-None-Match", "\"live\""))
            .respond_with(wiremock::ResponseTemplate::new(304))
            .with_priority(1)
            .mount(mock.server())
            .await;
        assert!(!refresh(&ctx, &file, |_| Box::pin(async { Ok(true) }))
            .await
            .unwrap());
        db.drop().await;
    }

    /// Labels with quotes, backslashes and dollar signs survive the generated SQL unchanged.
    #[tokio::test]
    async fn seed_sql_keeps_awkward_labels_intact() {
        let Some(admin_url) = database_url() else {
            return;
        };
        let db = FreshDb::create(&admin_url, "econgraph_reference_seed_quoting").await;
        let awkward = r#"O'Brien "quoted" back\slash $$ end \n tab	ü"#;
        let entry = SeedEntry {
            dataset: "bds".into(),
            dimension: DatasetComponent {
                name: "state".into(),
                label: "State".into(),
                component_type: econ_graph_core::models::ComponentType::String,
                unit: None,
                codes: None,
                codelist: None,
            },
            url: "https://example.test/it's.txt".into(),
            etag: Some(r#"W/"a'b""#.into()),
            codes: vec![Code::new("01", awkward)],
        };
        let at = "2026-10-02T03:00:00Z".parse().unwrap();
        let (up, _) = seed_migration_sql(SourceId::Census, at, &[entry]).unwrap();
        execute(&db.pool, &up).await;
        assert_eq!(
            state_codes(&db.pool).await.unwrap(),
            [("01".to_string(), awkward.to_string())]
        );
        assert_eq!(
            persist::reference_file_etag(
                &db.pool,
                SourceId::Census,
                "https://example.test/it's.txt"
            )
            .await
            .unwrap()
            .as_deref(),
            Some(r#"W/"a'b""#)
        );
        db.drop().await;
    }

    /// A code list that parses to nothing is a broken download: refused, and no ETag stored.
    #[tokio::test]
    async fn an_empty_code_list_is_refused() {
        let Some(admin_url) = database_url() else {
            return;
        };
        let db = FreshDb::create(&admin_url, "econgraph_reference_empty").await;
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get(URL_PATH),
            Reply::text("code\tlabel\n").header("ETag", "\"e\""),
        )
        .await;
        let list = CodeList {
            url: mock.url(URL_PATH),
            dataset: "bds",
            dimension: "state",
            parse: Arc::new(|_| Ok(Vec::new())),
        };
        let mut ctx = test_ctx();
        ctx.pool = db.pool.clone();
        let e = refresh_code_list(&ctx, SourceId::Census, &list)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "parse", "{e}");
        assert_eq!(
            persist::reference_file_etag(&db.pool, SourceId::Census, &list.url)
                .await
                .unwrap(),
            None
        );
        let e = download_seed_entries(
            &ctx.http,
            SourceId::Census,
            &[list],
            &catalog(&census(&mock)),
        )
        .await
        .unwrap_err();
        assert_eq!(e.kind(), "parse", "{e}");
        db.drop().await;
    }
}
