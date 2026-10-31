//! One consumer suite exercised by two implementations. Run Postgres explicitly with
//! DATA_DATABASE_URL=... cargo test -p econ-graph-services --test observation_contract -- --include-ignored.
use std::str::FromStr;

use bigdecimal::{BigDecimal, Zero};
use chrono::{NaiveDate, Utc};
use econ_graph_core::{error::AppResult, models::DataQueryParams};
use econ_graph_services::observation_store::{
    Observation, ObservationPage, ObservationRequest, PageContext, PostgresStore, RevisionMode,
    TimeSeriesStore, MAX_SERIES_DATA_PAGE,
};
use uuid::Uuid;

const EXACT: &str = "12345678901234567890123456789012345678901234567890.1234567890123456789";
const TINY: &str = "0.000000000000000000000000000000000000000000000000000000000001";
fn day(month: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2020, month, 1).unwrap()
}
fn request(series_id: Uuid, revisions: RevisionMode) -> ObservationRequest {
    ObservationRequest {
        series_id,
        revisions,
        start_date: None,
        end_date: None,
        limit: None,
        offset: None,
        context: None,
    }
}
fn observation_id(series_id: Uuid, i: u128) -> Uuid {
    Uuid::new_v5(&series_id, &i.to_be_bytes())
}
fn observations(series_id: Uuid) -> Vec<Observation> {
    [
        (1, 1, true, Some("999")),
        (1, 2, true, Some(EXACT)),
        (1, 3, false, None),
        (2, 2, true, Some("0")),
        (3, 3, true, Some(TINY)),
        (3, 3, false, Some(TINY)),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (date, rev, original, value))| Observation {
        id: observation_id(series_id, i as u128 + 1),
        series_id,
        date: day(date),
        revision_date: day(rev),
        is_original_release: original,
        value: value.map(|s| BigDecimal::from_str(s).unwrap()),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        attributes: match i {
            0 => None,
            1 => Some(serde_json::json!({"source":"fixture","footnotes":[],"preliminary":false})),
            _ => Some(serde_json::json!({"source":"fixture","footnotes":[{"code":"P","text":"literal\n雪"}],"preliminary":true,"vintage":rev})),
        },
    })
    .collect()
}

/// Deliberately has no SQL or DataPoint dependency: future adapters can implement the same trait.
struct MemoryStore {
    series_id: Uuid,
    dataset_id: Uuid,
    points: Vec<Observation>,
}
impl TimeSeriesStore for MemoryStore {
    async fn read_page(&self, req: ObservationRequest) -> AppResult<ObservationPage> {
        req.validate()?;
        let original = matches!(
            req.revisions,
            RevisionMode::Original | RevisionMode::OriginalLatest | RevisionMode::OriginalAsOf(_)
        );
        let as_of = match req.revisions {
            RevisionMode::AsOf(d) | RevisionMode::OriginalAsOf(d) => Some(d),
            _ => None,
        };
        let pick_one = matches!(
            req.revisions,
            RevisionMode::Latest
                | RevisionMode::AsOf(_)
                | RevisionMode::OriginalLatest
                | RevisionMode::OriginalAsOf(_)
        );
        let mut points: Vec<_> = self
            .points
            .iter()
            .filter(|p| {
                p.series_id == req.series_id
                    && req.start_date.is_none_or(|d| p.date >= d)
                    && req.end_date.is_none_or(|d| p.date <= d)
                    && (!original || p.is_original_release)
                    && as_of.is_none_or(|d| p.revision_date <= d)
                    && (req.revisions == RevisionMode::All
                        || !(p.is_original_release
                            && p.revision_date == p.date
                            && self.points.iter().any(|o| {
                                o.series_id == p.series_id
                                    && o.date == p.date
                                    && o.is_original_release
                                    && o.revision_date != p.date
                            })))
            })
            .cloned()
            .collect();
        if pick_one {
            points = points
                .iter()
                .filter(|p| {
                    !points.iter().any(|o| {
                        o.date == p.date
                            && (o.revision_date, !o.is_original_release, o.id)
                                > (p.revision_date, !p.is_original_release, p.id)
                    })
                })
                .cloned()
                .collect();
        }
        points.sort_by_key(|p| (p.date, p.revision_date, p.id));
        let total_count = points.len() as i64;
        let offset = req.offset.unwrap_or(0).max(0);
        let page: Vec<_> = points
            .iter()
            .skip(offset as usize)
            .take(
                req.limit
                    .unwrap_or(MAX_SERIES_DATA_PAGE)
                    .clamp(0, MAX_SERIES_DATA_PAGE) as usize,
            )
            .cloned()
            .collect();
        let context = if let (Some(context), Some(first)) = (req.context, page.first()) {
            let earlier: Vec<_> = points.iter().take(offset as usize).cloned().collect();
            match context {
                PageContext::PreviousPoint => earlier.into_iter().rev().take(1).collect(),
                PageContext::FirstPoint => earlier
                    .into_iter()
                    .filter(|p| p.value.as_ref().is_some_and(|v| !v.is_zero()))
                    .take(1)
                    .collect(),
                PageContext::Days(n) => earlier
                    .into_iter()
                    .filter(|p| {
                        p.date
                            >= first
                                .date
                                .checked_sub_signed(chrono::Duration::days(n))
                                .unwrap_or(NaiveDate::MIN)
                    })
                    .collect(),
            }
        } else {
            vec![]
        };
        let revised = page
            .iter()
            .filter_map(|point| {
                let original = self
                    .points
                    .iter()
                    .filter(|p| {
                        p.series_id == point.series_id
                            && p.date == point.date
                            && p.is_original_release
                            && !(p.revision_date == p.date
                                && self.points.iter().any(|other| {
                                    other.series_id == p.series_id
                                        && other.date == p.date
                                        && other.is_original_release
                                        && other.revision_date != p.date
                                }))
                    })
                    .min_by_key(|p| (p.revision_date, p.id))?;
                if original.id == point.id {
                    Some((point.id, false))
                } else if original.revision_date > point.revision_date {
                    None
                } else {
                    Some((point.id, point.value != original.value))
                }
            })
            .collect();
        Ok(ObservationPage {
            series_id: req.series_id,
            dataset_id: (req.series_id == self.series_id).then_some(self.dataset_id),
            points: page,
            context,
            revised,
            total_count,
            offset,
        })
    }
}

async fn contract(store: &impl TimeSeriesStore, series: Uuid, dataset: Uuid) {
    for days in [-1, i64::MIN, i64::MAX] {
        let mut req = request(series, RevisionMode::All);
        req.context = Some(PageContext::Days(days));
        assert!(matches!(
            store.read_page(req).await,
            Err(econ_graph_core::error::AppError::BadRequest(_))
        ));
    }
    let all = store
        .read_page(request(series, RevisionMode::All))
        .await
        .unwrap();
    assert_eq!(all.series_id, series);
    assert_eq!(all.dataset_id, Some(dataset));
    assert_eq!(all.total_count, 6);
    assert_eq!(all.points.len(), 6);
    assert_eq!(
        all.points[1].value,
        Some(BigDecimal::from_str(EXACT).unwrap())
    );
    assert_eq!(
        all.points[4].value,
        Some(BigDecimal::from_str(TINY).unwrap())
    );
    assert_eq!(all.points[2].value, None);
    assert_eq!(
        all.points.iter().map(|p| &p.attributes).collect::<Vec<_>>(),
        observations(series)
            .iter()
            .map(|p| &p.attributes)
            .collect::<Vec<_>>()
    );
    assert!(!all.has_next_page());
    assert_eq!(all.revised.get(&observation_id(series, 1)), None);
    assert_eq!(all.revised.get(&observation_id(series, 2)), Some(&false));
    assert_eq!(all.revised.get(&observation_id(series, 3)), Some(&true));
    assert_eq!(all.revised.get(&observation_id(series, 6)), Some(&false));
    for (mode, count, first_value) in [
        (RevisionMode::Latest, 3, None),
        (RevisionMode::AsOf(day(2)), 2, Some(EXACT)),
        (RevisionMode::Original, 3, Some(EXACT)),
        (RevisionMode::OriginalLatest, 3, Some(EXACT)),
        (RevisionMode::OriginalAsOf(day(2)), 2, Some(EXACT)),
    ] {
        let page = store.read_page(request(series, mode)).await.unwrap();
        assert_eq!(page.total_count, count, "{mode:?}");
        let expected = observations(series);
        for point in &page.points {
            assert_eq!(
                point.attributes,
                expected
                    .iter()
                    .find(|p| p.id == point.id)
                    .unwrap()
                    .attributes,
                "{mode:?}"
            );
        }
        if mode == RevisionMode::Latest {
            assert_eq!(
                page.points[2].id,
                observation_id(series, 6),
                "same-vintage revision beats original regardless of UUID"
            );
        }

        assert_eq!(
            page.points[0].value,
            first_value.map(|v| BigDecimal::from_str(v).unwrap()),
            "{mode:?}"
        );
        assert_eq!(
            page.points[0].id,
            if mode == RevisionMode::Latest {
                observation_id(series, 3)
            } else {
                observation_id(series, 2)
            }
        );
    }
    for context in [
        PageContext::PreviousPoint,
        PageContext::Days(32),
        PageContext::FirstPoint,
    ] {
        let mut req = request(series, RevisionMode::All);
        req.limit = Some(1);
        req.offset = Some(3);
        req.context = Some(context);
        let page = store.read_page(req).await.unwrap();
        assert_eq!(page.total_count, 6);
        assert_eq!(page.offset, 3);
        assert!(page.has_next_page());
        assert_eq!(page.points[0].id, all.points[3].id);
        let expected: Vec<_> = match context {
            PageContext::PreviousPoint => vec![3],
            PageContext::Days(_) => vec![1, 2, 3],
            PageContext::FirstPoint => vec![1],
        };
        assert_eq!(
            page.context.iter().map(|p| p.id).collect::<Vec<_>>(),
            expected
                .into_iter()
                .map(|i| observation_id(series, i))
                .collect::<Vec<_>>()
        );
    }
    let mut bounds = request(series, RevisionMode::All);
    bounds.start_date = Some(day(2));
    bounds.end_date = Some(day(2));
    let bounded = store.read_page(bounds).await.unwrap();
    assert_eq!(bounded.total_count, 1);
    assert_eq!(bounded.points[0].date, day(2));
    let mut empty = request(series, RevisionMode::All);
    empty.limit = Some(0);
    let empty = store.read_page(empty).await.unwrap();
    assert!(empty.points.is_empty());
    assert_eq!(empty.total_count, 6);
    let mut negative = request(series, RevisionMode::All);
    negative.offset = Some(-100);
    negative.limit = Some(-1);
    let negative = store.read_page(negative).await.unwrap();
    assert_eq!(negative.offset, 0);
    assert!(negative.points.is_empty());
    let mut beyond = request(series, RevisionMode::Latest);
    beyond.offset = Some(100);
    beyond.context = Some(PageContext::PreviousPoint);
    let beyond = store.read_page(beyond).await.unwrap();
    assert_eq!(beyond.total_count, 3);
    assert!(beyond.points.is_empty());
    assert!(beyond.context.is_empty());
    assert!(!beyond.has_next_page());
    let missing = store
        .read_page(request(Uuid::nil(), RevisionMode::Latest))
        .await
        .unwrap();
    assert_eq!(missing.dataset_id, None);
    assert_eq!(missing.total_count, 0);
    assert!(missing.points.is_empty());
}

#[tokio::test]
async fn alternate_adapter_satisfies_consumer_contract() {
    let series_id = Uuid::from_u128(100);
    let dataset_id = Uuid::from_u128(101);
    contract(
        &MemoryStore {
            series_id,
            dataset_id,
            points: observations(series_id),
        },
        series_id,
        dataset_id,
    )
    .await;
}

// Exercise the revised map independently of revision selection: equal decimal scales,
// nulls, reversion, multiple originals and unavailable originals.
fn revised_observations(series_id: Uuid) -> Vec<Observation> {
    let rows = [
        (1, 2, true, Some("3.0")),
        (1, 3, false, Some("3.00")),
        (1, 4, false, None),
        (1, 5, false, Some("3")),
        (2, 3, true, None),
        (2, 4, false, None),
        (2, 5, false, Some("0")),
        (3, 4, false, Some("9")), // no original
        (4, 5, true, Some("8")),
        (4, 4, false, Some("7")), // original not published yet
        (5, 6, true, Some("11")),
        (5, 7, true, Some("12")),
    ];
    rows.into_iter()
        .enumerate()
        .map(|(i, (date, rev, original, value))| Observation {
            id: Uuid::from_u128((series_id.as_u128() & !0xffff) | (1000 + i as u128)),
            series_id,
            date: day(date),
            revision_date: day(rev),
            is_original_release: original,
            value: value.map(|v| BigDecimal::from_str(v).unwrap()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            attributes: BTreeMap::new(),
        })
        .collect()
}

async fn revised_contract(store: &impl TimeSeriesStore, series: Uuid) {
    let page = store
        .read_page(request(series, RevisionMode::All))
        .await
        .unwrap();
    let expected = [
        Some(false),
        Some(false),
        Some(true),
        Some(false),
        Some(false),
        Some(false),
        Some(true),
        None,
        Some(false),
        None,
        Some(false),
        Some(true),
    ];
    for (i, expected) in expected.into_iter().enumerate() {
        assert_eq!(
            page.revised
                .get(&Uuid::from_u128(
                    (series.as_u128() & !0xffff) | (1000 + i as u128)
                ))
                .copied(),
            expected,
            "revised flag for fixture row {i}"
        );
    }
    // Paging and vintage bounds cannot hide an original used for the comparison.
    let mut req = request(series, RevisionMode::AsOf(day(4)));
    req.start_date = Some(day(1));
    req.end_date = Some(day(1));
    let page = store.read_page(req).await.unwrap();
    assert_eq!(page.points.len(), 1);
    assert_eq!(
        page.revised
            .get(&Uuid::from_u128((series.as_u128() & !0xffff) | 1002)),
        Some(&true)
    );
}

#[tokio::test]
async fn alternate_adapter_preserves_revised_flags() {
    let series_id = Uuid::new_v4();
    revised_contract(
        &MemoryStore {
            series_id,
            dataset_id: Uuid::new_v4(),
            points: revised_observations(series_id),
        },
        series_id,
    )
    .await;
}

#[test]
fn exact_nullable_transport_and_legacy_modes() {
    for p in observations(Uuid::nil()) {
        let json = serde_json::to_value(&p).unwrap();
        if p.value.is_some() {
            assert!(json["value"].is_string());
        } else {
            assert!(json["value"].is_null());
        }
        let decoded: Observation = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, p);
        let row: econ_graph_core::models::DataPoint = decoded.into();
        assert_eq!(row.value, p.value);
        assert_eq!(row.id, p.id);
    }
    for original in [None, Some(false), Some(true)] {
        for latest in [None, Some(false), Some(true)] {
            for as_of in [None, Some(day(2))] {
                let p = DataQueryParams {
                    series_id: Uuid::nil(),
                    start_date: None,
                    end_date: None,
                    original_only: original,
                    latest_revision_only: latest,
                    as_of,
                    limit: None,
                    offset: None,
                };
                let req = ObservationRequest::from_legacy(p, None);
                let back = req.legacy_params();
                assert_eq!(back.original_only, Some(original.unwrap_or(false)));
                assert_eq!(back.as_of, as_of);
                if as_of.is_none() {
                    assert_eq!(back.latest_revision_only, Some(latest.unwrap_or(false)));
                }
            }
        }
    }
}

#[tokio::test]
#[ignore = "requires migrated disposable DATA_DATABASE_URL; required Core CI includes this explicitly"]
async fn postgres_satisfies_same_consumer_contract() {
    use diesel_async::{RunQueryDsl, SimpleAsyncConnection};
    let url = std::env::var("DATA_DATABASE_URL")
        .expect("DATA_DATABASE_URL for explicit Postgres contract test");
    let pool = econ_graph_core::database::create_pool(&url).await.unwrap();
    let source = Uuid::new_v4();
    let dataset = Uuid::new_v4();
    let series = Uuid::new_v4();
    let mut conn = pool.get().await.unwrap();
    conn.batch_execute(&format!("INSERT INTO data_sources (id,name,description,base_url) VALUES ('{source}','store contract {source}','test','https://example.test'); INSERT INTO datasets (id,source_id,code,name) VALUES ('{dataset}','{source}','store-contract','Store contract'); INSERT INTO economic_series (id,source_id,external_id,title,frequency,dataset_id) VALUES ('{series}','{source}','store-contract','Store contract','monthly','{dataset}')"))
        .await.unwrap();
    for point in observations(series) {
        let row: econ_graph_core::models::DataPoint = point.into();
        // UUIDs derive from this series, so concurrent contract runs cannot collide.
        diesel::insert_into(econ_graph_core::schema::data_points::table)
            .values((
                econ_graph_core::schema::data_points::id.eq(row.id),
                econ_graph_core::schema::data_points::series_id.eq(row.series_id),
                econ_graph_core::schema::data_points::date.eq(row.date),
                econ_graph_core::schema::data_points::value.eq(row.value),
                econ_graph_core::schema::data_points::attributes.eq(row.attributes),
                econ_graph_core::schema::data_points::revision_date.eq(row.revision_date),
                econ_graph_core::schema::data_points::is_original_release
                    .eq(row.is_original_release),
            ))
            .execute(&mut conn)
            .await
            .unwrap();
    }
    drop(conn);
    contract(&PostgresStore::new(&pool), series, dataset).await;

    let legacy = econ_graph_services::series_service::get_series_data(
        &pool,
        request(series, RevisionMode::All).legacy_params(),
        None,
    )
    .await
    .unwrap();
    let neutral = PostgresStore::new(&pool)
        .read_page(request(series, RevisionMode::All))
        .await
        .unwrap();
    assert_eq!(legacy.total_count, neutral.total_count);
    assert_eq!(legacy.revised, neutral.revised);
    assert_eq!(
        legacy
            .points
            .iter()
            .map(|p| (p.id, &p.value))
            .collect::<Vec<_>>(),
        neutral
            .points
            .iter()
            .map(|p| (p.id, &p.value))
            .collect::<Vec<_>>()
    );

    // A writer changes the observation population between the store's count/page/context
    // statements. Every result must still describe one snapshot, never a mixed population.
    let writer_pool = pool.clone();
    let changing_id = observation_id(series, 7);
    let writer = tokio::spawn(async move {
        let mut conn = writer_pool.get().await.unwrap();
        for _ in 0..60 {
            diesel::sql_query(format!("INSERT INTO data_points (id,series_id,date,value,revision_date,is_original_release) VALUES ('{changing_id}','{series}','2020-04-01',7,'2020-04-01',true)"))
                .execute(&mut conn).await.unwrap();
            tokio::task::yield_now().await;
            diesel::sql_query(format!("DELETE FROM data_points WHERE id='{changing_id}'"))
                .execute(&mut conn)
                .await
                .unwrap();
        }
    });
    for _ in 0..60 {
        let mut req = request(series, RevisionMode::All);
        req.offset = Some(1);
        req.context = Some(PageContext::Days(366));
        let page = PostgresStore::new(&pool).read_page(req).await.unwrap();
        assert_eq!(page.context.len(), 1);
        assert_eq!(
            page.total_count as usize,
            page.points.len() + page.context.len()
        );
        // If the page saw this transient original, the flag lookup must see it too.
        if page.points.iter().any(|point| point.id == changing_id) {
            assert_eq!(page.revised.get(&changing_id), Some(&false));
        } else {
            assert!(!page.revised.contains_key(&changing_id));
        }
    }
    writer.await.unwrap();
    let mut conn = pool.get().await.unwrap();
    diesel::delete(
        econ_graph_core::schema::data_points::table
            .filter(econ_graph_core::schema::data_points::series_id.eq(series)),
    )
    .execute(&mut conn)
    .await
    .unwrap();
    for point in revised_observations(series) {
        let row: econ_graph_core::models::DataPoint = point.into();
        diesel::insert_into(econ_graph_core::schema::data_points::table)
            .values((
                econ_graph_core::schema::data_points::id.eq(row.id),
                econ_graph_core::schema::data_points::series_id.eq(row.series_id),
                econ_graph_core::schema::data_points::date.eq(row.date),
                econ_graph_core::schema::data_points::value.eq(row.value),
                econ_graph_core::schema::data_points::revision_date.eq(row.revision_date),
                econ_graph_core::schema::data_points::is_original_release
                    .eq(row.is_original_release),
            ))
            .execute(&mut conn)
            .await
            .unwrap();
    }
    drop(conn);
    revised_contract(&PostgresStore::new(&pool), series).await;
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query(format!("DELETE FROM data_sources WHERE id='{source}'"))
        .execute(&mut conn)
        .await
        .unwrap();
}
use diesel::{ExpressionMethods, QueryDsl};

/// Forecasts can be published before their observation date. A synthetic row stamped with
/// that future observation date must not defeat the actual vintage during latest selection.
fn projection_observations(series_id: Uuid) -> Vec<Observation> {
    let date = NaiveDate::from_ymd_opt(2030, 1, 1).unwrap();
    let now = Utc::now();
    [
        (date, date, true, "999"),
        (
            date,
            NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
            true,
            "12",
        ),
        (
            date,
            NaiveDate::from_ymd_opt(2024, 7, 1).unwrap(),
            false,
            "13",
        ),
        (
            NaiveDate::from_ymd_opt(2030, 2, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 8, 1).unwrap(),
            true,
            "14",
        ),
    ]
    .into_iter()
    .enumerate()
    .map(
        |(i, (date, revision_date, is_original_release, value))| Observation {
            id: observation_id(series_id, 100 + i as u128),
            series_id,
            date,
            revision_date,
            is_original_release,
            value: Some(BigDecimal::from_str(value).unwrap()),
            created_at: now,
            updated_at: now,
            attributes: BTreeMap::new(),
        },
    )
    .collect()
}

async fn projection_contract(store: &impl TimeSeriesStore, series: Uuid, dataset: Uuid) {
    let cutoff = NaiveDate::from_ymd_opt(2031, 1, 1).unwrap();
    for (mode, first_id) in [
        (RevisionMode::Latest, 102),
        (RevisionMode::AsOf(cutoff), 102),
        (RevisionMode::Original, 101),
        (RevisionMode::OriginalLatest, 101),
        (RevisionMode::OriginalAsOf(cutoff), 101),
    ] {
        let page = store.read_page(request(series, mode)).await.unwrap();
        assert_eq!(page.dataset_id, Some(dataset));
        assert_eq!(page.total_count, 2, "{mode:?}");
        assert_eq!(page.points.len(), 2, "{mode:?}");
        assert_eq!(
            page.points[0].id,
            observation_id(series, first_id),
            "{mode:?}"
        );
        assert_eq!(page.points[1].value, Some(BigDecimal::from(14)));
    }
    let all = store
        .read_page(request(series, RevisionMode::All))
        .await
        .unwrap();
    assert_eq!(all.total_count, 4);
    assert!(all
        .points
        .iter()
        .any(|p| p.id == observation_id(series, 100)));
    let mut paged = request(series, RevisionMode::Latest);
    paged.offset = Some(1);
    paged.limit = Some(1);
    paged.context = Some(PageContext::PreviousPoint);
    let page = store.read_page(paged).await.unwrap();
    assert_eq!(page.total_count, 2);
    assert_eq!(page.points[0].id, observation_id(series, 103));
    assert_eq!(page.context[0].id, observation_id(series, 102));
}

#[tokio::test]
async fn alternate_adapter_excludes_future_synthetic_competitors() {
    let series_id = Uuid::new_v4();
    let dataset_id = Uuid::new_v4();
    projection_contract(
        &MemoryStore {
            series_id,
            dataset_id,
            points: projection_observations(series_id),
        },
        series_id,
        dataset_id,
    )
    .await;
}

#[tokio::test]
#[ignore = "requires migrated disposable DATA_DATABASE_URL; required Core CI includes this explicitly"]
async fn postgres_preserves_restricted_visibility_and_projection_vintages() {
    use diesel::{ExpressionMethods, QueryDsl};
    use diesel_async::{RunQueryDsl, SimpleAsyncConnection};
    use econ_graph_core::schema::data_points;
    let url = std::env::var("DATA_DATABASE_URL")
        .expect("DATA_DATABASE_URL for explicit Postgres contract test");
    let pool = econ_graph_core::database::create_pool(&url).await.unwrap();
    let source = Uuid::new_v4();
    let dataset = Uuid::new_v4();
    let series = Uuid::new_v4();
    let mut conn = pool.get().await.unwrap();
    conn.batch_execute(&format!("INSERT INTO data_sources(id,name,description,base_url) VALUES('{source}','projection contract {source}','test','https://example.test'); INSERT INTO datasets(id,source_id,code,name) VALUES('{dataset}','{source}','projection','Projection contract'); INSERT INTO economic_series(id,source_id,external_id,title,frequency,dataset_id) VALUES('{series}','{source}','projection','Projection contract','monthly','{dataset}')")).await.unwrap();
    for point in projection_observations(series) {
        diesel::insert_into(data_points::table)
            .values((
                data_points::id.eq(point.id),
                data_points::series_id.eq(series),
                data_points::date.eq(point.date),
                data_points::value.eq(point.value),
                data_points::revision_date.eq(point.revision_date),
                data_points::is_original_release.eq(point.is_original_release),
            ))
            .execute(&mut conn)
            .await
            .unwrap();
    }
    drop(conn);
    let store = PostgresStore::new(&pool);
    projection_contract(&store, series, dataset).await;
    let legacy = econ_graph_services::series_service::get_series_data(
        &pool,
        request(series, RevisionMode::Latest).legacy_params(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(legacy.total_count, 2);
    assert_eq!(legacy.points[0].id, observation_id(series, 102));
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query("INSERT INTO series_lifecycle(source_id,external_id,status,scope_fingerprint) VALUES($1,'projection','restricted','observation-contract')")
        .bind::<diesel::sql_types::Uuid,_>(source).execute(&mut conn).await.unwrap();
    drop(conn);
    for mode in [
        RevisionMode::All,
        RevisionMode::Latest,
        RevisionMode::Original,
        RevisionMode::OriginalLatest,
        RevisionMode::AsOf(NaiveDate::from_ymd_opt(2031, 1, 1).unwrap()),
        RevisionMode::OriginalAsOf(NaiveDate::from_ymd_opt(2031, 1, 1).unwrap()),
    ] {
        let mut req = request(series, mode);
        req.offset = Some(1);
        req.context = Some(PageContext::FirstPoint);
        let hidden = store.read_page(req.clone()).await.unwrap();
        assert_eq!(
            hidden.dataset_id, None,
            "restricted dataset identity must not leak"
        );
        assert_eq!(hidden.total_count, 0);
        assert!(hidden.points.is_empty());
        assert!(hidden.context.is_empty());
        assert!(hidden.revised.is_empty());
        assert!(!hidden.has_next_page());
        let legacy = econ_graph_services::series_service::get_series_data(
            &pool,
            req.legacy_params(),
            req.context,
        )
        .await
        .unwrap();
        assert_eq!(legacy.total_count, 0);
        assert!(legacy.points.is_empty());
        assert!(legacy.context.is_empty());
        assert!(legacy.revised.is_empty());
    }
    let mut conn = pool.get().await.unwrap();
    let retained = data_points::table
        .filter(data_points::series_id.eq(series))
        .count()
        .get_result::<i64>(&mut conn)
        .await
        .unwrap();
    assert_eq!(
        retained, 4,
        "public restriction must not delete stored history"
    );
    diesel::sql_query("DELETE FROM series_lifecycle WHERE source_id=$1")
        .bind::<diesel::sql_types::Uuid, _>(source)
        .execute(&mut conn)
        .await
        .unwrap();
    drop(conn);
    projection_contract(&store, series, dataset).await;
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query("DELETE FROM data_sources WHERE id=$1")
        .bind::<diesel::sql_types::Uuid, _>(source)
        .execute(&mut conn)
        .await
        .unwrap();
}
