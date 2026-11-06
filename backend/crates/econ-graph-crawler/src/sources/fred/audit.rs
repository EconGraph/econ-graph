//! Temporary, opt-in evidence exporter. This module is compiled only with `cfg(test)`.
//! No database, observation requests, alternate matcher, or alternate pagination loop.

use super::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const HISTORY_SHA256: &str = "006745a4e19f2faca561cbc8ca8ae295365aa2afbb637a7bec32f5a2a22343a8";
const FIXTURE: &str = include_str!("../../../tests/fixtures/fred/coverage-family-witnesses.json");

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn failed() -> CrawlError {
    // Neither IO errors, provider errors, configuration text nor URLs enter artifacts.
    CrawlError::Permanent("FRED audit evidence could not be recorded safely".into())
}

fn canonical(value: Value) -> Value {
    match value {
        Value::Object(map) => serde_json::to_value(
            map.into_iter()
                .map(|(k, v)| (k, canonical(v)))
                .collect::<BTreeMap<_, _>>(),
        )
        .expect("JSON values serialize"),
        Value::Array(rows) => Value::Array(rows.into_iter().map(canonical).collect()),
        other => other,
    }
}

fn catalog_key(rule: &FredCatalogRule) -> String {
    match rule.release_id {
        Some(id) => format!("release-{id}"),
        None => format!("category-{}", rule.category_id.expect("validated rule")),
    }
}

#[derive(Clone)]
struct Historical {
    id: String,
    family: usize,
}

fn history(bytes: &[u8]) -> Result<Vec<Historical>, CrawlError> {
    if digest(bytes) != HISTORY_SHA256 {
        return Err(failed());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| failed())?;
    let mut family = 0;
    let mut rows = Vec::new();
    for line in text.lines() {
        if line.starts_with("# --- ") {
            family += 1;
        } else if !line.is_empty() && !line.starts_with('#') && line != "series_id" {
            validate_catalog_id(line)?;
            rows.push(Historical {
                id: line.into(),
                family,
            });
        }
    }
    if family != 33
        || rows.len() != 1533
        || rows.iter().map(|r| &r.id).collect::<BTreeSet<_>>().len() != 1533
    {
        return Err(failed());
    }
    Ok(rows)
}

struct State {
    report: Value,
    rules: FredCatalogRules,
    keys: Vec<String>,
    fixture: Value,
    historical: Vec<Historical>,
    seen: BTreeSet<String>,
    bindings: BTreeMap<String, Vec<Value>>,
}

/// Deliberately does not derive Debug: the in-memory credential is never formatted.
pub(super) struct Recorder {
    directory: PathBuf,
    secret: String,
    state: Mutex<State>,
}

impl std::fmt::Debug for Recorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FredAuditRecorder")
    }
}

impl Recorder {
    fn new(
        directory: &Path,
        secret: &str,
        rules: &FredCatalogRules,
        text: &str,
        fixture: Value,
        historical: Vec<Historical>,
        revision: &str,
    ) -> Result<Arc<Self>, CrawlError> {
        if directory.join("manifest.json").exists() {
            return Err(failed());
        }
        std::fs::create_dir_all(directory).map_err(|_| failed())?;
        let keys: Vec<_> = text
            .lines()
            .filter_map(|line| line.strip_prefix("# rule: "))
            .map(|line| line.split(';').next().unwrap_or_default().trim().to_owned())
            .collect();
        let keys = if keys.len() == rules.rules.len() {
            keys
        } else {
            (0..rules.rules.len())
                .map(|i| format!("rule-{i}"))
                .collect()
        };
        let mut catalogs = serde_json::Map::new();
        for rule in &rules.rules {
            catalogs.entry(catalog_key(rule)).or_insert(json!({
                "complete": false, "pages": {}, "ids": []
            }));
        }
        let recorder = Arc::new(Self {
            directory: directory.into(),
            secret: secret.into(),
            state: Mutex::new(State {
                report: json!({
                    "format": 1, "complete": false, "selection_complete": false,
                    "status": "partial_in_progress", "started_at": chrono::Utc::now().to_rfc3339(),
                    "revision": revision, "rules_sha256": digest(text.as_bytes()),
                    "scope_fingerprint": rules.fingerprint(), "fixture_sha256": digest(FIXTURE.as_bytes()),
                    "bounds": {"copied_scan_guard": rules.max_catalog_series, "production_selected_guard": rules.max_series,
                        "selected_hard_ceiling": 10000, "count_uncapped": true, "timeout_seconds": 1800,
                        "requests_per_minute": 30, "ownership_lookup_cap": 2},
                    "historical": {"commit": "4c51c29c20b47113b4574aad6ae44b01beef021f", "sha256": HISTORY_SHA256,
                        "provided": !historical.is_empty(), "runtime_membership": false},
                    "catalogs": catalogs, "rules": keys.iter().enumerate().map(|(i,key)| json!({
                        "index":i,"key":key,"catalog":catalog_key(&rules.rules[i]),
                        "description":rules.rules[i].describe(),"metric_label":rules.rules[i].label(i),
                        "max_matches":rules.rules[i].max_matches,"complete":false,"ids":[]
                    })).collect::<Vec<_>>(),
                    "includes": rules.include.iter().map(|id| (id.clone(), json!({"outcome":"unvisited",
                        "expected_missing":rules.expected_missing.contains(id)}))).collect::<BTreeMap<_,_>>(),
                    "excludes": rules.exclude, "metadata_responses": {}, "ownership_responses": {},
                    "completed_catalog_rows": 0, "selected_union": [], "positive_witnesses": [], "synthetic_boundaries": []
                }),
                rules: rules.clone(),
                keys,
                fixture,
                historical,
                seen: BTreeSet::new(),
                bindings: BTreeMap::new(),
            }),
        });
        recorder.update(|_| {})?;
        Ok(recorder)
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), CrawlError> {
        let text = std::str::from_utf8(bytes).map_err(|_| failed())?;
        if !crate::http::audit_text_is_safe(text, &self.secret) {
            return Err(failed());
        }
        let path = self.directory.join(name);
        let temporary = path.with_extension("pending");
        std::fs::write(&temporary, bytes).map_err(|_| failed())?;
        std::fs::rename(temporary, path).map_err(|_| failed())
    }

    fn update(&self, change: impl FnOnce(&mut State)) -> Result<(), CrawlError> {
        let mut state = self.state.lock().map_err(|_| failed())?;
        change(&mut state);
        self.write(
            "manifest.json",
            &serde_json::to_vec_pretty(&state.report).map_err(|_| failed())?,
        )
    }

    fn capture(&self, name: &str, text: &str) -> Result<Value, CrawlError> {
        let raw_name = format!("{name}.response-text.json");
        self.write(&raw_name, text.as_bytes())?;
        let mut record = json!({"retrieved_at":chrono::Utc::now().to_rfc3339(),
            "raw_representation":"response_text_utf8_not_http_wire", "raw_path":raw_name,
            "raw_sha256":digest(text.as_bytes()), "validation":"downloaded_unvalidated"});
        if let Ok(value) = serde_json::from_str::<Value>(text) {
            let bytes =
                serde_json::to_vec_pretty(&canonical(value.clone())).map_err(|_| failed())?;
            let normalized_name = format!("{name}.normalized.json");
            self.write(&normalized_name, &bytes)?;
            record["normalized_path"] = json!(normalized_name);
            record["normalized_sha256"] = json!(digest(&bytes));
            record["advertised_count"] = value["count"].clone();
            record["response_offset"] = value["offset"].clone();
            record["response_limit"] = value["limit"].clone();
        }
        Ok(record)
    }

    pub(super) fn realtime(&self, realtime: &str) -> Result<(), CrawlError> {
        self.update(|s| s.report["catalog_realtime"] = json!(realtime))
    }

    pub(super) fn page(
        &self,
        rule: &FredCatalogRule,
        offset: usize,
        text: &str,
    ) -> Result<(), CrawlError> {
        let key = catalog_key(rule);
        let mut record = self.capture(&format!("{key}-{offset}"), text)?;
        record["requested_offset"] = json!(offset);
        self.update(|s| s.report["catalogs"][&key]["pages"][offset.to_string()] = record)
    }

    pub(super) fn page_validated(
        &self,
        rule: &FredCatalogRule,
        offset: &str,
    ) -> Result<(), CrawlError> {
        self.update(|s| {
            s.report["catalogs"][catalog_key(rule)]["pages"][offset]["validation"] =
                json!("validated")
        })
    }

    pub(super) fn catalog_complete(
        &self,
        group: &[usize],
        rows: &[CatalogSeries],
        rules: &FredCatalogRules,
    ) -> Result<(), CrawlError> {
        let key = catalog_key(&rules.rules[group[0]]);
        self.update(|s| {
            s.report["catalogs"][&key]["complete"] = json!(true);
            s.report["completed_catalog_rows"] =
                json!(s.report["completed_catalog_rows"].as_u64().unwrap_or(0) + rows.len() as u64);
            s.report["catalogs"][&key]["ids"] =
                json!(rows.iter().map(|r| &r.id).collect::<Vec<_>>());
            for &index in group {
                s.report["rules"][index]["complete"] = json!(true);
                s.report["rules"][index]["ids"] = json!(rows
                    .iter()
                    .filter(
                        |row| rules.rules[index].matches(row) && !rules.exclude.contains(&row.id)
                    )
                    .map(|row| &row.id)
                    .collect::<Vec<_>>());
            }
            for (position, row) in rows.iter().enumerate() {
                s.seen.insert(row.id.clone());
                let offset = position / CATALOG_PAGE_SIZE * CATALOG_PAGE_SIZE;
                let page = s.report["catalogs"][&key]["pages"][offset.to_string()].clone();
                Self::bind(
                    s,
                    row,
                    json!({"kind":"catalog", "catalog":key,
                    "page":page,"realtime":s.report["catalog_realtime"]}),
                    group,
                );
            }
        })
    }

    fn bind(state: &mut State, row: &CatalogSeries, evidence: Value, group: &[usize]) {
        let wanted = state.fixture["families"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|family| {
                family["positive_witnesses"]
                    .as_array()
                    .into_iter()
                    .flatten()
            })
            .any(|w| w["series_id"].as_str() == Some(row.id.as_str()));
        if wanted {
            let matches = group
                .iter()
                .filter(|&&i| {
                    state.rules.rules[i].matches(row) && !state.rules.exclude.contains(&row.id)
                })
                .map(|&i| &state.keys[i])
                .collect::<Vec<_>>();
            state.bindings.entry(row.id.clone()).or_default().push(json!({
                "series_id":row.id,"title":row.metadata.title,"frequency":row.metadata.frequency,
                "matching_rule_keys":matches,"evidence":evidence
            }));
        }
    }

    pub(super) fn metadata(&self, id: &str, text: &str) -> Result<(), CrawlError> {
        let name = format!("series-{}", digest(id.as_bytes()));
        let mut record = self.capture(&name, text)?;
        record["realtime_pinned"] = json!(false);
        let row = serde_json::from_str::<Value>(text)
            .ok()
            .and_then(|v| {
                v["seriess"]
                    .as_array()?
                    .iter()
                    .find(|r| r["id"].as_str() == Some(id))
                    .cloned()
            })
            .and_then(|v| serde_json::from_value::<CatalogSeries>(v).ok());
        record["requested_id_present"] = json!(row.is_some());
        self.update(|s| {
            s.report["metadata_responses"][id] = record.clone();
            if let Some(row) = row {
                Self::bind(
                    s,
                    &row,
                    json!({"kind":"include_metadata","response":record}),
                    &[],
                );
            }
        })
    }

    pub(super) fn include_outcome(&self, id: &str, outcome: &str) -> Result<(), CrawlError> {
        self.update(|s| s.report["includes"][id]["outcome"] = json!(outcome))
    }

    pub(super) fn success(
        &self,
        selection: &CatalogSelection,
        facts: &CatalogLifecycle,
    ) -> Result<(), CrawlError> {
        self.update(|s| {
            s.report["selection_complete"] = json!(true);
            s.report["scanned_catalog_rows"] = json!(selection.scanned);
            let mut memberships: BTreeMap<String, Vec<usize>> = BTreeMap::new();
            for (index, rule) in s.report["rules"].as_array().expect("rules").iter().enumerate() {
                for id in rule["ids"].as_array().expect("rule IDs") {
                    if let Some(id) = id.as_str() { memberships.entry(id.to_owned()).or_default().push(index); }
                }
            }
            s.report["selected_union"] = json!(selection
                .found
                .iter()
                .map(|row| json!({
                    "id":row.external_id,"title":row.title,"frequency":row.frequency,
                    "matching_rule_indices":memberships.get(&row.external_id).cloned().unwrap_or_default(),
                    "included":s.rules.include.contains(&row.external_id),
                    "restricted":facts.restricted_ids.contains(&row.external_id),
                    "discontinued":facts.discontinued_ids.contains(&row.external_id)
                }))
                .collect::<Vec<_>>());
            s.report["confirmed_missing_ids"] = json!(facts.confirmed_missing_ids);
            s.report["rule_counts"] = json!(selection.rule_matches);
        })
    }

    async fn ownership(&self, adapter: &FredAdapter, ctx: &CrawlCtx) -> Result<(), CrawlError> {
        // Only the two include-only positives lack a configured catalog ownership witness.
        // Official endpoint: https://fred.stlouisfed.org/docs/api/fred/series_release.html
        let (ids, realtime) = {
            let state = self.state.lock().map_err(|_| failed())?;
            let ids: BTreeSet<String> = state.fixture["families"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|f| f["positive_witnesses"].as_array().into_iter().flatten())
                .filter(|w| {
                    w["matching_rule_keys"]
                        .as_array()
                        .is_some_and(Vec::is_empty)
                })
                .filter_map(|w| w["series_id"].as_str().map(str::to_owned))
                .collect();
            (
                ids,
                state.report["catalog_realtime"]
                    .as_str()
                    .ok_or_else(failed)?
                    .to_owned(),
            )
        };
        if ids.len() > 2 {
            return Err(failed());
        }
        for id in ids {
            let query = [
                ("series_id", id.as_str()),
                ("api_key", adapter.api_key(ctx)?),
                ("file_type", "json"),
                ("realtime_start", realtime.as_str()),
                ("realtime_end", realtime.as_str()),
            ];
            let value: Value = ctx
                .http
                .get_json_audited(
                    SourceId::Fred,
                    &adapter.url("/series/release"),
                    &query,
                    |text| {
                        let mut record =
                            self.capture(&format!("ownership-{}", digest(id.as_bytes())), text)?;
                        record["realtime_pinned"] = json!(true);
                        self.update(|s| s.report["ownership_responses"][&id] = record)
                    },
                )
                .await?;
            let releases = value["releases"].as_array().ok_or_else(failed)?;
            if releases.is_empty() || releases.iter().any(|r| r["id"].as_u64().is_none()) {
                return Err(failed());
            }
            self.update(|s| {
                s.report["ownership_responses"][&id]["release_ids"] =
                    json!(releases.iter().map(|r| &r["id"]).collect::<Vec<_>>());
                s.report["ownership_responses"][&id]["validation"] = json!("validated");
            })?;
        }
        Ok(())
    }

    fn finish(&self, reason: Option<&str>) -> Result<bool, CrawlError> {
        self.update(|s| {
            let retrieval_complete = reason.is_none() && s.report["selection_complete"] == true;
            s.report["retrieval_complete"] = json!(retrieval_complete);
            s.report["finished_at"] = json!(chrono::Utc::now().to_rfc3339());
            let selected: BTreeSet<String> = s.report["selected_union"].as_array().into_iter().flatten()
                .filter_map(|r| r["id"].as_str().map(str::to_owned)).collect();
            let matched: BTreeSet<String> = s.report["rules"].as_array().into_iter().flatten()
                .flat_map(|rule|rule["ids"].as_array().into_iter().flatten())
                .filter_map(|id|id.as_str().map(str::to_owned)).collect();
            let positives = s.fixture["families"].as_array().into_iter().flatten().flat_map(|family| {
                family["positive_witnesses"].as_array().into_iter().flatten().map(|w| {
                    let id = w["series_id"].as_str().unwrap_or_default();
                    let bindings = s.bindings.get(id).cloned().unwrap_or_default();
                    let expected_keys = &w["matching_rule_keys"];
                    let matched = bindings.iter().any(|b| {
                        let ownership = if expected_keys.as_array().is_some_and(Vec::is_empty) {
                            s.report["ownership_responses"][id]["release_ids"].as_array()
                                .is_some_and(|ids| ids.contains(&w["release_id"]))
                        } else {
                            b["evidence"]["catalog"] == json!(format!("release-{}",w["release_id"]))
                        };
                        ownership && b["title"] == w["title"] && b["frequency"] == w["frequency"]
                            && &b["matching_rule_keys"] == expected_keys
                    });
                    json!({"family":family["family"],"series_id":id,"kind":"authenticated_metadata_binding",
                        "verified":matched && selected.contains(id),"bindings":bindings,
                        "ownership_response":s.report["ownership_responses"][id],
                        "absence_is_not_not_found":true})
                }).collect::<Vec<_>>()
            }).collect::<Vec<_>>();
            let positive_bindings_accepted = positives.iter().all(|p| p["verified"] == true);
            s.report["positive_witnesses"] = json!(positives);
            let negatives = s.fixture["negative_identifier_witnesses"].as_array().into_iter().flatten().map(|n| {
                let index = s.keys.iter().position(|k| Some(k.as_str()) == n["rule_key"].as_str());
                let positive = s.fixture["families"].as_array().into_iter().flatten()
                    .flat_map(|f| f["positive_witnesses"].as_array().into_iter().flatten())
                    .find(|w| w["matching_rule_keys"].as_array().is_some_and(|keys| keys.contains(&n["rule_key"])));
                let rejected = index.zip(positive).is_some_and(|(index,w)| {
                    let mut row: CatalogSeries = serde_json::from_value(json!({"id":w["series_id"],"title":w["title"],"frequency":w["frequency"]})).expect("checked fixture");
                    let initially_matches = s.rules.rules[index].matches(&row);
                    row.id = n["series_id"].as_str().unwrap_or_default().into();
                    initially_matches && !s.rules.rules[index].matches(&row)
                });
                json!({"kind":"synthetic_identifier_boundary","rule_key":n["rule_key"],"series_id":n["series_id"],
                    "predicate_rejected":rejected,"upstream_existence_verified":false,"api_membership_claim":false})
            }).collect::<Vec<_>>();
            let synthetic_boundaries_accepted = negatives.iter().all(|n| n["predicate_rejected"] == true);
            s.report["synthetic_boundaries"] = json!(negatives);
            s.report["witness_bindings_accepted"] = json!(positive_bindings_accepted && synthetic_boundaries_accepted);
            let complete = retrieval_complete && positive_bindings_accepted && synthetic_boundaries_accepted;
            s.report["complete"] = json!(complete);
            s.report["status"] = json!(reason.unwrap_or(if complete {"complete"} else {"witness_mismatch_or_unresolved"}));
            s.report["historical"]["membership"] = json!(s.historical.iter().map(|h| {
                let classification = if selected.contains(&h.id) {"selected"}
                    else if s.rules.exclude.contains(&h.id) {"explicitly_excluded"}
                    else if s.report["includes"][&h.id]["outcome"] == "confirmed_not_found" {"confirmed_not_found"}
                    else if s.seen.contains(&h.id) {
                        if s.report["selection_complete"] != true && matched.contains(&h.id) {"matched_rule_selection_incomplete"}
                        else {"listed_in_complete_catalog_but_not_selected"}
                    }
                    else if s.report["selection_complete"] == true {"not_in_configured_catalogs_unresolved"}
                    else {"unresolved_partial_scan"};
                json!({"id":h.id,"family":h.family,"classification":classification})
            }).collect::<Vec<_>>());
        })?;
        Ok(self.state.lock().map_err(|_| failed())?.report["complete"] == true)
    }
}

fn error_kind(error: &CrawlError) -> &'static str {
    if is_catalog_volume_guard(error) {
        return "volume_guard";
    }
    match error {
        CrawlError::Auth(_) => "authentication",
        CrawlError::Parse(_) => "parse_or_credential_echo",
        CrawlError::Transient(_) => "provider_or_pagination",
        CrawlError::NotFound(_) => "provider_not_found",
        CrawlError::RateLimited { .. } => "rate_limited",
        _ => "selection_failed",
    }
}

fn record_execution_diagnostics(
    recorder: &Arc<Recorder>,
    http: &crate::http::HttpFetcher,
    error: Option<&CrawlError>,
) -> Result<(), CrawlError> {
    let failures = http.take_audit_failures();
    recorder.update(|s| {
        s.report["http_failures"] = json!(failures);
        s.report["selection_error_category"] = json!(error.map(CrawlError::kind));
        s.report["http_diagnostic_policy"] =
            json!("numeric_status_and_static_identifiers_only_no_urls_values_or_provider_text");
    })
}

// Frozen final 84-rule draft scope: this header-only measurement must never silently follow a
// later rule expansion. It makes no production selection or membership decision.
const SCOPE_RULES_SHA256: &str = "c9364a95a019170c7dac9133691387f46db921b66e8d1b5923d1d97dc488885a";

fn scope_groups(rules: &FredCatalogRules) -> Vec<Vec<usize>> {
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (index, rule) in rules.rules.iter().enumerate() {
        if let Some(group) = groups.iter_mut().find(|group| {
            let first = &rules.rules[group[0]];
            (first.release_id, first.category_id) == (rule.release_id, rule.category_id)
        }) {
            group.push(index);
        } else {
            groups.push(vec![index]);
        }
    }
    groups
}

fn scope_recorder(
    directory: &Path,
    secret: &str,
    rules: &FredCatalogRules,
    text: &str,
    revision: &str,
    realtime: &str,
) -> Result<Arc<Recorder>, CrawlError> {
    let groups = scope_groups(rules);
    let recorder = Recorder::new(
        directory,
        secret,
        rules,
        text,
        json!({}),
        Vec::new(),
        revision,
    )?;
    let catalogs: serde_json::Map<String, Value> = groups
        .iter()
        .map(|group| {
            (
                catalog_key(&rules.rules[group[0]]),
                json!({"rule_indices":group,"status":"unvisited"}),
            )
        })
        .collect();
    recorder.update(|state| state.report = json!({
        "format":1,"mode":"scope_counts","revision":revision,"rules_sha256":digest(text.as_bytes()),
        "scope":"FIRST-PAGE projected scan scope only; not membership, selected IDs or vintage cost acceptance",
        "complete":false,"counts_complete":false,"membership_complete":false,
        "status":"partial_in_progress","catalog_realtime":realtime,
        "expected_catalogs":groups.len(),"validated_catalogs":0,
        "projected_catalog_rows":0,"projected_catalog_page_requests":0,
        "production_scan_guard":rules.max_catalog_series,"production_selected_guard":rules.max_series,
        "logical_request_limit":71,"http_attempt_limit":213,"requests_per_minute":30,
        "timeout_seconds":1800,"catalogs":catalogs
    }))?;
    Ok(recorder)
}

async fn scope_headers(
    adapter: &FredAdapter,
    ctx: &CrawlCtx,
    rules: &FredCatalogRules,
    recorder: &Arc<Recorder>,
    realtime: &str,
) -> Result<bool, CrawlError> {
    let groups = scope_groups(rules);
    if groups.len() > 71 {
        return Err(failed());
    }
    let mut all_valid = true;
    for group in groups {
        let rule = &rules.rules[group[0]];
        let key = catalog_key(rule);
        let (path, parameter, id) = if let Some(id) = rule.release_id {
            ("/release/series", "release_id", id)
        } else {
            (
                "/category/series",
                "category_id",
                rule.category_id.ok_or_else(failed)?,
            )
        };
        let id = id.to_string();
        let query = [
            (parameter, id.as_str()),
            ("api_key", adapter.api_key(ctx)?),
            ("file_type", "json"),
            ("limit", "1"),
            ("offset", "0"),
            ("order_by", "series_id"),
            ("sort_order", "asc"),
            ("realtime_start", realtime),
            ("realtime_end", realtime),
        ];
        let result: Result<CatalogPage, CrawlError> = ctx
            .http
            .get_json_audited(SourceId::Fred, &adapter.url(path), &query, |text| {
                let page = recorder.capture(&format!("{key}-0"), text)?;
                recorder.update(|s| s.report["catalogs"][&key]["page"] = page)
            })
            .await
            .map_err(classify_fred_error);
        let page = match result {
            Ok(page)
                if page.offset == 0
                    && page.limit == 1
                    && page.seriess.len() == page.count.min(1)
                    && page.seriess.iter().all(|row| {
                        validate_catalog_id(&row.id).is_ok()
                            && !row.metadata.title.trim().is_empty()
                            && row
                                .metadata
                                .frequency
                                .as_ref()
                                .is_some_and(|f| !f.trim().is_empty())
                    }) =>
            {
                page
            }
            result => {
                all_valid = false;
                let reason = match result {
                    Err(ref error) => error_kind(error),
                    _ => "invalid_header",
                };
                recorder.update(|s| {
                    s.report["catalogs"][&key]["status"] = json!(reason);
                    s.report["catalogs"][&key]["retrieved_at"] =
                        json!(chrono::Utc::now().to_rfc3339());
                })?;
                continue;
            }
        };
        // Checked arithmetic, not a volume guard: no rows beyond the first are
        // fetched even when advertised counts exceed production's scan ceiling.
        let (previous, pages) = {
            let s = recorder.state.lock().map_err(|_| failed())?;
            (
                s.report["projected_catalog_rows"]
                    .as_u64()
                    .ok_or_else(failed)?,
                s.report["projected_catalog_page_requests"]
                    .as_u64()
                    .ok_or_else(failed)?,
            )
        };
        let count = u64::try_from(page.count).map_err(|_| failed())?;
        let total = previous.checked_add(count).ok_or_else(failed)?;
        let requests = count.div_ceil(1000).max(1);
        let page_total = pages.checked_add(requests).ok_or_else(failed)?;
        recorder.update(|s| {
            s.report["catalogs"][&key]["status"] = json!("header_validated");
            s.report["catalogs"][&key]["provider_count"] = json!(count);
            s.report["catalogs"][&key]["production_would_refuse_empty"] = json!(count == 0);
            s.report["catalogs"][&key]["projected_page_requests"] = json!(requests);
            s.report["catalogs"][&key]["retrieved_at"] = json!(chrono::Utc::now().to_rfc3339());
            s.report["projected_catalog_rows"] = json!(total);
            s.report["projected_catalog_page_requests"] = json!(page_total);
            s.report["validated_catalogs"] =
                json!(s.report["validated_catalogs"].as_u64().unwrap_or(0) + 1);
        })?;
    }
    Ok(all_valid)
}

#[tokio::test]
#[ignore = "explicit authenticated first-page scope audit only"]
async fn authenticated_catalog_scope_counts() {
    async fn run() -> Result<(), CrawlError> {
        let directory =
            PathBuf::from(std::env::var_os("FRED_AUDIT_OUTPUT_DIR").ok_or_else(failed)?);
        let revision = std::env::var("FRED_AUDIT_REVISION").map_err(|_| failed())?;
        if !directory.is_absolute()
            || revision.len() != 40
            || !revision.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(failed());
        }
        let secret = std::env::var("FRED_API_KEY").map_err(|_| failed())?;
        if secret.trim().is_empty() {
            return Err(failed());
        }
        let text =
            std::fs::read_to_string(data_dir().join(CATALOG_RULES_FILE)).map_err(|_| failed())?;
        let rules = FredCatalogRules::parse(&text)?;
        if digest(text.as_bytes()) != SCOPE_RULES_SHA256
            || rules.rules.len() != 84
            || rules.include.len() != 30
            || scope_groups(&rules).len() != 71
            || rules.max_catalog_series != 20000
            || rules.max_series != 500
        {
            return Err(failed());
        }
        let realtime = chrono::Utc::now().date_naive().to_string();
        let recorder = scope_recorder(&directory, &secret, &rules, &text, &revision, &realtime)?;
        let checker = crate::check_selection::Checker::live(
            crate::http::HttpConfig::default(),
            std::collections::HashMap::from([(
                SourceId::Fred,
                crate::policy::SourcePolicy::default_for(SourceId::Fred),
            )]),
            25,
            crate::adapter::ApiKeys {
                fred: Some(secret),
                ..Default::default()
            },
            chrono::Utc::now().date_naive(),
        )?;
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(1800),
            scope_headers(
                &FredAdapter::default(),
                &checker.ctx,
                &rules,
                &recorder,
                &realtime,
            ),
        )
        .await;
        let (complete, status) = match result {
            Ok(Ok(true)) => (true, "all_headers_validated"),
            Ok(Ok(false)) => (false, "header_errors"),
            Ok(Err(_)) => (false, "scope_failed"),
            Err(_) => (false, "timeout"),
        };
        record_execution_diagnostics(&recorder, &checker.ctx.http, None)?;
        recorder.update(|s| {
            s.report["counts_complete"] = json!(complete);
            s.report["status"] = json!(status);
            s.report["finished_at"] = json!(chrono::Utc::now().to_rfc3339());
            s.report["projected_rows_exceed_production_scan_guard"] =
                json!(s.report["projected_catalog_rows"].as_u64().unwrap_or(0) > 20000);
        })?;
        if !complete {
            return Err(failed());
        }
        Ok(())
    }
    assert!(
        run().await.is_ok(),
        "FRED scope counts incomplete; inspect safe evidence"
    );
}

// This helper is in the cfg(test) module, not the production parser or selector.
// Parsing always happens before the explicit in-memory measurement override.
fn full_inventory_rules(
    text: &str,
    fixture: &Value,
) -> Result<(FredCatalogRules, FredCatalogRules), CrawlError> {
    let original = FredCatalogRules::parse(text)?;
    let positives = fixture["families"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|family| family["positive_witnesses"].as_array().map_or(0, Vec::len))
        .sum::<usize>();
    if digest(text.as_bytes()) != SCOPE_RULES_SHA256
        || digest(FIXTURE.as_bytes())
            != "0b723859ecb703c75158d59013b561b241a037cdac9a1647cbbb6c639e0f3dd7"
        || original.rules.len() != 84
        || original.include.len() != 30
        || scope_groups(&original).len() != 71
        || original.max_series != 500
        || original.max_catalog_series != 20000
        || positives != 40
        || fixture["families"].as_array().map_or(0, Vec::len) != 33
        || fixture["negative_identifier_witnesses"]
            .as_array()
            .map_or(0, Vec::len)
            != 13
    {
        return Err(failed());
    }
    let mut audit = original.uncapped();
    audit.max_catalog_series = 200000;
    Ok((original, audit))
}

fn full_inventory_manifest(
    recorder: &Arc<Recorder>,
    original: &FredCatalogRules,
    effective: &FredCatalogRules,
) -> Result<(), CrawlError> {
    recorder.update(|s| {
        s.report["mode"] = json!("full_inventory");
        s.report["scope"] = json!("Configured catalog membership evidence only; no observation/vintage cost or production budget acceptance");
        s.report["original_scope_fingerprint"] = json!(original.fingerprint());
        s.report["effective_audit_scope_fingerprint"] = json!(effective.fingerprint());
        s.report["selection_fingerprint_excludes_operational_guards"] = json!(true);
        s.report["audit_execution_fingerprint"] = json!(format!("fred-audit-run-v1:{}", digest(
            canonical(json!({"selection_scope":effective.fingerprint(),"scan_ceiling":200000,"count_uncapped":true}))
                .to_string().as_bytes())));
        s.report["expected_positive_witnesses"] = json!(40);
        s.report["production_budget_accepted"] = json!(false);
        s.report["bounds"] = json!({
            "production_config_scan_guard":20000,"production_config_selected_guard":500,
            "production_parser_scan_ceiling":100000,"production_parser_selected_ceiling":10000,
            "test_only_scan_ceiling":200000,"test_only_selected_count_uncapped":true,
            "conservative_catalog_page_ceiling":270,"include_lookup_ceiling":30,"ownership_lookup_ceiling":2,
            "conservative_logical_request_ceiling":302,"conservative_http_attempt_ceiling":906,
            "timeout_seconds":1800,"wrapper_timeout_seconds":1830,"requests_per_minute":30
        });
        // Old authenticated evidence remains bound to its own 83-rule revision.
        // The final84 rules share physical catalogs; this is topology reuse,
        // never a claim that the old run evaluated the new predicates/witnesses.
        s.report["dated_scope_baseline"] = json!({
            "revision":"68b69191bef12408daf719112d1deec5ea1619af",
            "rules_sha256":"825a4f835488b775a79dcd520eb6c4ad8a39bbc71e0ccdce8bca5fdcdc29f955",
            "rules":83,"physical_catalogs":71,"catalog_realtime":"2026-11-05",
            "projected_catalog_rows":149899,"projected_catalog_pages":204,
            "projected_logical_requests_including_max_overrides":236,
            "kind":"dated_physical_scope_estimate_not_current_membership_evidence",
            "final84_rule_api_membership_verified_by_baseline":false
        });
    })
}

/// Runtime interface (none read by compilation/offline tests): FRED_AUDIT_OUTPUT_DIR,
/// FRED_AUDIT_HISTORICAL_CSV, FRED_AUDIT_REVISION, CRAWLER_DATA_DIR, FRED_API_KEY.
/// The runner copies the original 20k/500 config unchanged. Only this ignored
/// test applies the explicit authorized 200k scan override in memory.
#[tokio::test]
#[ignore = "explicit authenticated audit only; never part of ordinary test execution"]
async fn authenticated_catalog_evidence() {
    async fn run() -> Result<(), CrawlError> {
        let output = std::env::var_os("FRED_AUDIT_OUTPUT_DIR").ok_or_else(failed)?;
        if !Path::new(&output).is_absolute() {
            return Err(failed());
        }
        let historical = std::env::var_os("FRED_AUDIT_HISTORICAL_CSV").ok_or_else(failed)?;
        let revision = std::env::var("FRED_AUDIT_REVISION").map_err(|_| failed())?;
        if revision.len() != 40 || !revision.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(failed());
        }
        let secret = std::env::var("FRED_API_KEY").map_err(|_| failed())?;
        if secret.trim().is_empty() {
            return Err(failed());
        }
        let text =
            std::fs::read_to_string(data_dir().join(CATALOG_RULES_FILE)).map_err(|_| failed())?;
        let fixture: Value = serde_json::from_str(FIXTURE).map_err(|_| failed())?;
        let (rules, audit_rules) = full_inventory_rules(&text, &fixture)?;
        let historical = history(&std::fs::read(historical).map_err(|_| failed())?)?;
        let recorder = Recorder::new(
            Path::new(&output),
            &secret,
            &rules,
            &text,
            fixture,
            historical,
            &revision,
        )?;
        full_inventory_manifest(&recorder, &rules, &audit_rules)?;
        let keys = crate::adapter::ApiKeys {
            fred: Some(secret),
            ..Default::default()
        };
        let checker = crate::check_selection::Checker::live(
            crate::http::HttpConfig::default(),
            std::collections::HashMap::from([(
                SourceId::Fred,
                crate::policy::SourcePolicy::default_for(SourceId::Fred),
            )]),
            25,
            keys,
            chrono::Utc::now().date_naive(),
        )?;
        let adapter = FredAdapter {
            audit: Some(recorder.clone()),
            ..FredAdapter::default()
        };
        let result = tokio::time::timeout(std::time::Duration::from_secs(1800), async {
            adapter.select_catalog(&checker.ctx, &audit_rules).await?;
            recorder.ownership(&adapter, &checker.ctx).await
        })
        .await;
        let reason = match &result {
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(error_kind(error)),
            Err(_) => Some("timeout"),
        };
        record_execution_diagnostics(
            &recorder,
            &checker.ctx.http,
            match &result {
                Ok(Err(error)) => Some(error),
                _ => None,
            },
        )?;
        if !recorder.finish(reason)? {
            return Err(failed());
        }
        Ok(())
    }
    assert!(
        run().await.is_ok(),
        "FRED audit incomplete; inspect the credential-free manifest"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{test_ctx, MockSource, Reply, Route, TEST_API_KEY};

    #[tokio::test]
    async fn audit_http_diagnostics_keep_numeric_status_and_static_parameter_enums_only() {
        for (status, category, message, parameter) in [
            (
                400,
                "permanent",
                "Bad Request. The value for variable realtime_end is invalid. DO_NOT_PERSIST",
                Some("realtime_end"),
            ),
            (403, "auth", "DO_NOT_PERSIST access refused", None),
            (
                400,
                "permanent",
                "Bad Request. Variable unknown_private_field is invalid. DO_NOT_PERSIST",
                None,
            ),
        ] {
            let mock = MockSource::start().await;
            mock.mount_expect(
                &Route::get("/release/series"),
                Reply::json(json!({"error_code":status,"error_message":message}))
                    .with_status(status),
                1,
            )
            .await;
            let directory = tempfile::tempdir().unwrap();
            let (mut adapter, rules, recorder) = setup(directory.path(), rules_text());
            adapter.base_url = mock.base_url();
            let ctx = test_ctx();
            let error = adapter.select_catalog(&ctx, &rules).await.unwrap_err();
            record_execution_diagnostics(&recorder, &ctx.http, Some(&error)).unwrap();
            assert!(!recorder.finish(Some(error_kind(&error))).unwrap());
            let report = manifest(directory.path());
            let failures = report["http_failures"].as_array().unwrap();
            assert_eq!(failures.len(), 1);
            assert_eq!(failures[0]["http_status"], status);
            assert_eq!(failures[0]["error_category"], category);
            assert_eq!(failures[0]["endpoint"], "release_series");
            assert_eq!(failures[0]["provider_named_parameter"], json!(parameter));
            assert_eq!(report["selection_error_category"], category);
            assert!(failures[0]["request_parameter_names"]
                .as_array()
                .unwrap()
                .contains(&json!("api_key")));
            let bytes = std::fs::read(directory.path().join("manifest.json")).unwrap();
            let text = String::from_utf8(bytes).unwrap();
            assert!(!text.contains(TEST_API_KEY));
            assert!(!text.contains("DO_NOT_PERSIST"));
            assert!(!text.contains("http://"));
            assert!(!text.contains("unknown_private_field"));
            assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
            assert_eq!(mock.received_requests().await.len(), 1);
        }
    }

    #[tokio::test]
    async fn audit_http_diagnostics_omit_encoded_credential_error_bodies_before_any_artifact() {
        let unicode = TEST_API_KEY
            .chars()
            .map(|c| format!("\\u{:04x}", c as u32))
            .collect::<String>();
        let percent = TEST_API_KEY
            .bytes()
            .map(|b| format!("%{b:02X}"))
            .collect::<String>();
        for status in [400, 403] {
            for spelling in [
                TEST_API_KEY.to_owned(),
                unicode.clone(),
                percent.clone(),
                unicode.replace('\\', "\\\\"),
            ] {
                let mock = MockSource::start().await;
                mock.mount_expect(&Route::get("/release/series"),
                    Reply::json(json!({"error_code":status,"error_message":format!("DO_NOT_PERSIST {spelling}")}))
                        .with_status(status),1).await;
                let directory = tempfile::tempdir().unwrap();
                let (mut adapter, rules, recorder) = setup(directory.path(), rules_text());
                adapter.base_url = mock.base_url();
                let ctx = test_ctx();
                let error = adapter.select_catalog(&ctx, &rules).await.unwrap_err();
                record_execution_diagnostics(&recorder, &ctx.http, Some(&error)).unwrap();
                assert!(!recorder.finish(Some(error_kind(&error))).unwrap());
                let report = manifest(directory.path());
                assert_eq!(report["http_failures"][0]["http_status"], status);
                assert_eq!(
                    report["http_failures"][0]["reason"],
                    "credential_echo_body_omitted"
                );
                assert!(report["http_failures"][0]["provider_named_parameter"].is_null());
                assert_eq!(report["complete"], false);
                let text = std::fs::read_to_string(directory.path().join("manifest.json")).unwrap();
                assert!(crate::http::audit_text_is_safe(&text, TEST_API_KEY));
                assert!(!text.contains("DO_NOT_PERSIST"));
                assert!(!text.contains(&spelling));
                assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
                assert_eq!(mock.received_requests().await.len(), 1);
            }
        }
    }

    #[test]
    fn full_inventory_override_is_test_only_and_preserves_production_guards_and_all40_witnesses() {
        let text = include_str!("../../../data/catalog_selection/fred.toml");
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        let (original, effective) = full_inventory_rules(text, &fixture).unwrap();
        assert_eq!(original.max_catalog_series, 20000);
        assert_eq!(original.max_series, 500);
        assert_eq!(effective.max_catalog_series, 200000);
        assert_eq!(effective.max_series, usize::MAX);
        assert_eq!(
            serde_json::to_value(&original.rules).unwrap(),
            serde_json::to_value(&effective.rules).unwrap()
        );
        assert!(FredCatalogRules::parse(
            &text.replace("max_catalog_series = 20000", "max_catalog_series = 200000")
        )
        .is_err());
        assert!(
            FredCatalogRules::parse(&text.replace("max_series = 500", "max_series = 10001"))
                .is_err()
        );
        assert_eq!(digest(text.as_bytes()), SCOPE_RULES_SHA256);
        let directory = tempfile::tempdir().unwrap();
        let recorder = Recorder::new(
            directory.path(),
            TEST_API_KEY,
            &original,
            text,
            fixture,
            Vec::new(),
            "offline",
        )
        .unwrap();
        full_inventory_manifest(&recorder, &original, &effective).unwrap();
        let report = manifest(directory.path());
        assert_eq!(report["expected_positive_witnesses"], 40);
        assert_eq!(report["bounds"]["production_config_scan_guard"], 20000);
        assert_eq!(report["bounds"]["production_parser_scan_ceiling"], 100000);
        assert_eq!(report["bounds"]["test_only_scan_ceiling"], 200000);
        assert_eq!(
            report["bounds"]["conservative_logical_request_ceiling"],
            302
        );
        assert_eq!(report["bounds"]["conservative_http_attempt_ceiling"], 906);
        assert_eq!(
            report["original_scope_fingerprint"],
            report["effective_audit_scope_fingerprint"]
        );
        assert_eq!(
            report["scope_fingerprint"],
            report["original_scope_fingerprint"]
        );
        assert_eq!(report["dated_scope_baseline"]["rules"], 83);
        assert_eq!(
            report["dated_scope_baseline"]["final84_rule_api_membership_verified_by_baseline"],
            false
        );
        assert_eq!(report["complete"], false);
        assert_eq!(report["production_budget_accepted"], false);
        assert!(!recorder.finish(Some("offline_partial")).unwrap());
        let partial = manifest(directory.path());
        assert_eq!(partial["positive_witnesses"].as_array().unwrap().len(), 40);
        assert!(partial["positive_witnesses"]
            .as_array()
            .unwrap()
            .iter()
            .all(|w| w["verified"] == false));
    }

    #[tokio::test]
    async fn full_inventory_in_memory_ceiling_crosses_parser_limit_but_retains_page_validation() {
        let mock = MockSource::start().await;
        let text = include_str!("../../../data/catalog_selection/fred.toml");
        let (original, effective) =
            full_inventory_rules(text, &serde_json::from_str(FIXTURE).unwrap()).unwrap();
        let first = (0..1000)
            .map(|i| row(&format!("A{i:04}")))
            .collect::<Vec<_>>();
        mock.mount_expect(
            &Route::get("/release/series").query("offset", "0"),
            Reply::json(json!({"count":100001,"offset":0,"limit":1000,"seriess":first})),
            2,
        )
        .await;
        mock.mount_expect(
            &Route::get("/release/series").query("offset", "1000"),
            Reply::json(json!({"count":100001,"offset":1001,"limit":1000,"seriess":[]})),
            1,
        )
        .await;
        let adapter = FredAdapter {
            base_url: mock.base_url(),
            ..Default::default()
        };
        let rule = &original.rules[0];
        let guarded = adapter
            .catalog_rows(&test_ctx(), TEST_API_KEY, rule, &original, "2026-11-06", 0)
            .await
            .unwrap_err();
        assert!(is_catalog_volume_guard(&guarded));
        let incomplete = adapter
            .catalog_rows(&test_ctx(), TEST_API_KEY, rule, &effective, "2026-11-06", 0)
            .await
            .unwrap_err();
        assert!(!is_catalog_volume_guard(&incomplete));
        assert!(matches!(incomplete, CrawlError::Transient(_)));
        assert_eq!(mock.received_requests().await.len(), 3);
        assert_eq!(original.max_catalog_series, 20000);
        assert_eq!(original.max_series, 500);
    }

    #[tokio::test]
    async fn scope_counts_requests_one_row_per_group_even_above_scan_guard() {
        let mock = MockSource::start().await;
        let directory = tempfile::tempdir().unwrap();
        let text = "max_series=1\nmax_catalog_series=10\n[[rules]]\nrelease_id=53\n[[rules]]\nrelease_id=53\nid_prefix='Z'\n[[rules]]\nrelease_id=54";
        let rules = FredCatalogRules::parse(text).unwrap();
        for (id, count) in [(53, 100858), (54, 200000)] {
            mock.mount_expect(
                &Route::get("/release/series")
                    .query("release_id", id.to_string())
                    .query("limit", "1")
                    .query("offset", "0")
                    .query("realtime_start", "2026-11-05")
                    .query("realtime_end", "2026-11-05")
                    .query("order_by", "series_id")
                    .query("sort_order", "asc"),
                Reply::json(json!({"count":count,"offset":0,"limit":1,"seriess":[row("PUBLIC")]})),
                1,
            )
            .await;
        }
        let recorder = scope_recorder(
            directory.path(),
            TEST_API_KEY,
            &rules,
            text,
            "offline",
            "2026-11-05",
        )
        .unwrap();
        let adapter = FredAdapter {
            base_url: mock.base_url(),
            ..Default::default()
        };
        assert!(
            scope_headers(&adapter, &test_ctx(), &rules, &recorder, "2026-11-05")
                .await
                .unwrap()
        );
        let report = manifest(directory.path());
        assert_eq!(report["validated_catalogs"], 2);
        assert_eq!(report["projected_catalog_rows"], 300858);
        assert_eq!(report["projected_catalog_page_requests"], 301);
        assert_eq!(
            report["catalogs"]["release-53"]["rule_indices"],
            json!([0, 1])
        );
        assert_eq!(report["membership_complete"], false);
        assert_eq!(report["complete"], false);
        assert_eq!(mock.received_requests().await.len(), 2);
        assert_eq!(report["production_scan_guard"], 10);
        assert_eq!(report["production_selected_guard"], 1);
    }

    #[tokio::test]
    async fn scope_counts_invalid_header_is_not_zero_and_other_catalog_is_measured() {
        let mock = MockSource::start().await;
        let directory = tempfile::tempdir().unwrap();
        let text = "max_series=1\nmax_catalog_series=10\n[[rules]]\nrelease_id=53\n[[rules]]\nrelease_id=54";
        let rules = FredCatalogRules::parse(text).unwrap();
        mock.mount_expect(
            &Route::get("/release/series").query("release_id", "53"),
            Reply::json(json!({"count":1000000,"offset":1,"limit":1,"seriess":[row("PUBLIC")]})),
            1,
        )
        .await;
        mock.mount_expect(
            &Route::get("/release/series").query("release_id", "54"),
            Reply::json(json!({"count":0,"offset":0,"limit":1,"seriess":[]})),
            1,
        )
        .await;
        let recorder = scope_recorder(
            directory.path(),
            TEST_API_KEY,
            &rules,
            text,
            "offline",
            "2026-11-05",
        )
        .unwrap();
        let adapter = FredAdapter {
            base_url: mock.base_url(),
            ..Default::default()
        };
        assert!(
            !scope_headers(&adapter, &test_ctx(), &rules, &recorder, "2026-11-05")
                .await
                .unwrap()
        );
        let report = manifest(directory.path());
        assert_eq!(report["validated_catalogs"], 1);
        assert_eq!(report["projected_catalog_rows"], 0);
        assert_eq!(report["catalogs"]["release-53"]["status"], "invalid_header");
        assert!(report["catalogs"]["release-53"]["provider_count"].is_null());
        assert_eq!(
            report["catalogs"]["release-54"]["production_would_refuse_empty"],
            true
        );
        assert_eq!(report["projected_catalog_page_requests"], 1);
        assert_eq!(mock.received_requests().await.len(), 2);
    }

    fn rules_text() -> &'static str {
        "max_series=50\nmax_catalog_series=10000\ninclude=['EXTRA']\nexclude=['ADROP']\n# rule: first\n[[rules]]\nrelease_id=53\nid_prefix='A'\n# rule: second\n[[rules]]\nrelease_id=53\nid_pattern='A.*'"
    }
    fn row(id: &str) -> Value {
        json!({"id":id,"title":"Example","frequency":"Quarterly"})
    }
    fn page(count: usize, offset: usize, rows: Vec<Value>) -> Reply {
        Reply::json_str(
            json!({"count":count,"offset":offset,"limit":1000,"seriess":rows}).to_string(),
        )
    }
    fn setup(dir: &Path, text: &str) -> (FredAdapter, FredCatalogRules, Arc<Recorder>) {
        let rules = FredCatalogRules::parse(text).unwrap();
        let recorder = Recorder::new(
            dir,
            TEST_API_KEY,
            &rules,
            text,
            json!({"families":[],"negative_identifier_witnesses":[]}),
            Vec::new(),
            "offline",
        )
        .unwrap();
        (
            FredAdapter {
                audit: Some(recorder.clone()),
                ..FredAdapter::default()
            },
            rules,
            recorder,
        )
    }
    fn manifest(dir: &Path) -> Value {
        serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).unwrap()).unwrap()
    }

    #[tokio::test]
    async fn recorder_uses_one_real_walk_keeps_overlap_union_and_requires_finalization() {
        let mock = MockSource::start().await;
        let dir = tempfile::tempdir().unwrap();
        mock.mount_expect(
            &Route::get("/release/series"),
            page(3, 0, vec![row("AA"), row("AB"), row("ADROP")]),
            1,
        )
        .await;
        mock.mount(
            &Route::get("/series").query("series_id", "EXTRA"),
            Reply::json_str(json!({"seriess":[row("EXTRA")]}).to_string()),
        )
        .await;
        let (mut adapter, rules, rec) = setup(dir.path(), rules_text());
        adapter.base_url = mock.base_url();
        let selected = adapter.select_catalog(&test_ctx(), &rules).await.unwrap();
        assert_eq!(selected.rule_matches, vec![2, 2]);
        assert_eq!(selected.scanned, 3);
        let before = manifest(dir.path());
        assert_eq!(before["complete"], false);
        assert_eq!(before["selection_complete"], true);
        rec.finish(None).unwrap();
        let report = manifest(dir.path());
        assert_eq!(report["complete"], true);
        assert_eq!(report["rules"][0]["ids"], json!(["AA", "AB"]));
        assert_eq!(report["rules"][1]["ids"], json!(["AA", "AB"]));
        assert_eq!(report["selected_union"].as_array().unwrap().len(), 3);
        assert_eq!(report["includes"]["EXTRA"]["outcome"], "metadata_found");
        assert_eq!(
            report["metadata_responses"]["EXTRA"]["realtime_pinned"],
            false
        );
        let raw = std::fs::read(dir.path().join("release-53-0.response-text.json")).unwrap();
        assert_eq!(
            report["catalogs"]["release-53"]["pages"]["0"]["raw_sha256"],
            digest(&raw)
        );
        assert_eq!(
            report["catalogs"]["release-53"]["pages"]["0"]["validation"],
            "validated"
        );
        mock.server().verify().await;
    }

    #[tokio::test]
    async fn recorder_preserves_downloaded_pages_but_never_completes_drift_or_guard_failure() {
        for guard in [false, true] {
            let mock = MockSource::start().await;
            let dir = tempfile::tempdir().unwrap();
            let text = if guard {
                "max_series=50\nmax_catalog_series=1\n[[rules]]\nrelease_id=53"
            } else {
                "max_series=50\nmax_catalog_series=2000\n[[rules]]\nrelease_id=53"
            };
            let rows = (0..1000).map(|i| row(&format!("A{i:04}"))).collect();
            mock.mount(
                &Route::get("/release/series").query("offset", "0"),
                page(1001, 0, rows),
            )
            .await;
            mock.mount(
                &Route::get("/release/series").query("offset", "1000"),
                page(1002, 1000, vec![row("Z")]),
            )
            .await;
            let (mut adapter, rules, rec) = setup(dir.path(), text);
            adapter.base_url = mock.base_url();
            let error = adapter
                .select_catalog(&test_ctx(), &rules)
                .await
                .unwrap_err();
            rec.finish(Some(error_kind(&error))).unwrap();
            let report = manifest(dir.path());
            assert_eq!(report["complete"], false);
            assert_eq!(report["selection_complete"], false);
            assert_eq!(report["rules"][0]["complete"], false);
            assert_eq!(report["selected_union"], json!([]));
            assert!(dir.path().join("release-53-0.response-text.json").exists());
            if guard {
                assert_eq!(report["status"], "volume_guard");
                assert_eq!(
                    report["catalogs"]["release-53"]["pages"]["0"]["validation"],
                    "downloaded_unvalidated"
                );
            } else {
                assert!(dir
                    .path()
                    .join("release-53-1000.response-text.json")
                    .exists());
                assert_eq!(
                    report["catalogs"]["release-53"]["pages"]["1000"]["validation"],
                    "downloaded_unvalidated"
                );
            }
        }
    }

    #[tokio::test]
    async fn recorder_omits_encoded_credential_echoes_and_provider_error_messages() {
        for body in [json!({"count":1,"offset":0,"limit":1000,"seriess":[{"id":"AA","title":TEST_API_KEY,"frequency":"Quarterly"}]}).to_string(),
            format!("{{\"count\":1,\"offset\":0,\"limit\":1000,\"seriess\":[{{\"id\":\"AA\",\"title\":\"{}\",\"frequency\":\"Quarterly\"}}]}}",TEST_API_KEY.bytes().map(|b|format!("\\u{b:04x}")).collect::<String>())] {
            let mock=MockSource::start().await;let dir=tempfile::tempdir().unwrap();
            mock.mount(&Route::get("/release/series"),Reply::json_str(body)).await;
            let(mut adapter,rules,rec)=setup(dir.path(),rules_text());adapter.base_url=mock.base_url();
            let error=adapter.select_catalog(&test_ctx(),&rules).await.unwrap_err();rec.finish(Some(error_kind(&error))).unwrap();
            for path in std::fs::read_dir(dir.path()).unwrap(){let text=std::fs::read_to_string(path.unwrap().path()).unwrap();assert!(!text.contains(TEST_API_KEY));assert!(!text.contains("api_key"));}
            assert!(!dir.path().join("release-53-0.response-text.json").exists());assert_eq!(manifest(dir.path())["complete"],false);
        }
    }

    #[tokio::test]
    async fn nested_and_mixed_credential_spellings_never_create_artifact_files() {
        let key = "0123456789abcdef0123456789abcdef";
        let unicode = |slashes: usize| {
            key.chars()
                .map(|c| format!("{}u{:04x}", "\\".repeat(slashes), c as u32))
                .collect::<String>()
        };
        let mixed = key
            .chars()
            .enumerate()
            .map(|(i, c)| match i % 3 {
                0 => c.to_string(),
                1 => format!("%{:02X}", c as u32),
                _ => format!("\\\\\\u{:04X}", c as u32),
            })
            .collect::<String>();
        let mut bodies = vec![unicode(1), unicode(2), unicode(24), mixed,
            serde_json::to_string(&unicode(1)).unwrap()].into_iter().map(|title|
            json!({"count":1,"offset":0,"limit":1000,"seriess":[{"id":"AA","title":title,"frequency":"Quarterly"}]}).to_string()).collect::<Vec<_>>();
        let mut object_key = json!({"count":1,"offset":0,"limit":1000,"seriess":[row("AA")]});
        object_key
            .as_object_mut()
            .unwrap()
            .insert(unicode(3), json!("public value"));
        bodies.push(object_key.to_string());
        for body in bodies {
            let mock = MockSource::start().await;
            let dir = tempfile::tempdir().unwrap();
            mock.mount(&Route::get("/release/series"), Reply::json_str(&body))
                .await;
            let rules = FredCatalogRules::parse(rules_text()).unwrap();
            let recorder = Recorder::new(
                dir.path(),
                key,
                &rules,
                rules_text(),
                json!({"families":[],"negative_identifier_witnesses":[]}),
                Vec::new(),
                "offline",
            )
            .unwrap();
            let adapter = FredAdapter {
                base_url: mock.base_url(),
                audit: Some(recorder.clone()),
                ..FredAdapter::default()
            };
            let mut ctx = test_ctx();
            ctx.keys.fred = Some(key.into());
            let error = adapter.select_catalog(&ctx, &rules).await.unwrap_err();
            recorder.finish(Some(error_kind(&error))).unwrap();
            // Defense at both boundaries: request hook and direct artifact recorder.
            assert!(recorder.capture("direct-echo", &body).is_err());
            let names = std::fs::read_dir(dir.path())
                .unwrap()
                .map(|e| e.unwrap().file_name())
                .collect::<Vec<_>>();
            assert_eq!(
                names,
                vec![std::ffi::OsString::from("manifest.json")],
                "raw, normalized and pending echo artifacts must all be absent"
            );
            assert_eq!(manifest(dir.path())["complete"], false);
        }
    }

    #[test]
    fn targeted_credential_detection_preserves_unrelated_response_text_exactly() {
        let key = "0123456789abcdef0123456789abcdef";
        let safe = r#" { "notes": "public \\u0030 and percent %30 are unrelated", "value": 12345678901234567890.123400, "unicode": "é" } "#;
        assert!(crate::http::audit_text_is_safe(safe, key));
        assert!(!crate::http::audit_text_is_safe(
            r#"{"x":"\\u00e9\\ud83e\\udd80"}"#,
            "é🦀"
        ));
        let dir = tempfile::tempdir().unwrap();
        let rules = FredCatalogRules::parse(rules_text()).unwrap();
        let recorder = Recorder::new(
            dir.path(),
            key,
            &rules,
            rules_text(),
            json!({"families":[],"negative_identifier_witnesses":[]}),
            Vec::new(),
            "offline",
        )
        .unwrap();
        let receipt = recorder.capture("unrelated", safe).unwrap();
        let raw = std::fs::read(dir.path().join("unrelated.response-text.json")).unwrap();
        assert_eq!(raw, safe.as_bytes());
        assert_eq!(receipt["raw_sha256"], digest(safe.as_bytes()));
    }

    #[test]
    fn draft_witnesses_use_real_matcher_and_label_all_thirteen_boundaries_synthetic() {
        let text = include_str!("../../../data/catalog_selection/fred.toml");
        let rules = FredCatalogRules::parse(text).unwrap();
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let rec = Recorder::new(
            dir.path(),
            TEST_API_KEY,
            &rules,
            text,
            fixture,
            Vec::new(),
            "offline",
        )
        .unwrap();
        rec.finish(Some("offline_only")).unwrap();
        let report = manifest(dir.path());
        assert_eq!(report["positive_witnesses"].as_array().unwrap().len(), 40);
        let negatives = report["synthetic_boundaries"].as_array().unwrap();
        assert_eq!(negatives.len(), 13);
        assert!(negatives.iter().all(|n| n["predicate_rejected"] == true
            && n["upstream_existence_verified"] == false
            && n["api_membership_claim"] == false));
        assert!(report["positive_witnesses"]
            .as_array()
            .unwrap()
            .iter()
            .all(|w| w["verified"] == false));
        assert_eq!(report["complete"], false);
    }

    #[tokio::test]
    async fn authenticated_bindings_require_catalog_fields_and_override_ownership() {
        for release in [53, 99, 0] {
            let mock = MockSource::start().await;
            let dir = tempfile::tempdir().unwrap();
            mock.mount(&Route::get("/release/series"), page(1, 0, vec![row("AA")]))
                .await;
            mock.mount(
                &Route::get("/series"),
                Reply::json_str(json!({"seriess":[row("EXTRA")]}).to_string()),
            )
            .await;
            let reply = if release == 0 {
                Reply::text(format!("provider refusal {TEST_API_KEY}")).with_status(403)
            } else {
                Reply::json_str(json!({"releases":[{"id":release}]}).to_string())
            };
            mock.mount(&Route::get("/series/release"), reply).await;
            let rules = FredCatalogRules::parse(rules_text()).unwrap();
            let fixture = json!({"families":[{"family":1,"positive_witnesses":[
                {"series_id":"AA","title":"Example","frequency":"Quarterly","release_id":53,"matching_rule_keys":["first","second"]},
                {"series_id":"EXTRA","title":"Example","frequency":"Quarterly","release_id":53,"matching_rule_keys":[]}
            ]}],"negative_identifier_witnesses":[]});
            let rec = Recorder::new(
                dir.path(),
                TEST_API_KEY,
                &rules,
                rules_text(),
                fixture,
                Vec::new(),
                "offline",
            )
            .unwrap();
            let adapter = FredAdapter {
                base_url: mock.base_url(),
                audit: Some(rec.clone()),
                ..FredAdapter::default()
            };
            adapter.select_catalog(&test_ctx(), &rules).await.unwrap();
            let ownership = rec.ownership(&adapter, &test_ctx()).await;
            let complete = rec
                .finish(ownership.as_ref().err().map(error_kind))
                .unwrap();
            let report = manifest(dir.path());
            assert_eq!(complete, release == 53);
            assert_eq!(report["selection_complete"], true);
            assert_eq!(report["retrieval_complete"], release != 0);
            assert_eq!(report["positive_witnesses"][0]["verified"], true);
            assert_eq!(report["positive_witnesses"][1]["verified"], release == 53);
            assert_eq!(
                report["positive_witnesses"][0]["bindings"][0]["evidence"]["catalog"],
                "release-53"
            );
            if release != 0 {
                assert_eq!(
                    report["ownership_responses"]["EXTRA"]["realtime_pinned"],
                    true
                );
            } else {
                assert_eq!(report["status"], "authentication");
            }
            assert!(!std::fs::read_to_string(dir.path().join("manifest.json"))
                .unwrap()
                .contains(TEST_API_KEY));
        }
    }

    #[tokio::test]
    async fn timeout_keeps_completed_catalog_evidence_and_unresolved_history_partial() {
        let mock = MockSource::start().await;
        let dir = tempfile::tempdir().unwrap();
        let text = "max_series=50\nmax_catalog_series=10000\n[[rules]]\nrelease_id=53\n[[rules]]\nrelease_id=54";
        mock.mount(
            &Route::get("/release/series").query("release_id", "53"),
            page(1, 0, vec![row("AA")]),
        )
        .await;
        mock.mount(
            &Route::get("/release/series").query("release_id", "54"),
            page(1, 0, vec![row("ZZ")]).delay(std::time::Duration::from_secs(1)),
        )
        .await;
        let (mut adapter, rules, rec) = setup(dir.path(), text);
        rec.state.lock().unwrap().historical = vec![
            Historical {
                id: "AA".into(),
                family: 1,
            },
            Historical {
                id: "UNSEEN".into(),
                family: 1,
            },
        ];
        adapter.base_url = mock.base_url();
        assert!(tokio::time::timeout(
            std::time::Duration::from_millis(150),
            adapter.select_catalog(&test_ctx(), &rules)
        )
        .await
        .is_err());
        assert!(!rec.finish(Some("timeout")).unwrap());
        let report = manifest(dir.path());
        assert_eq!(report["catalogs"]["release-53"]["complete"], true);
        assert_eq!(report["catalogs"]["release-54"]["complete"], false);
        assert_eq!(
            report["historical"]["membership"][0]["classification"],
            "matched_rule_selection_incomplete"
        );
        assert_eq!(
            report["historical"]["membership"][1]["classification"],
            "unresolved_partial_scan"
        );
        assert_eq!(report["status"], "timeout");
    }

    #[test]
    fn historical_input_rejects_wrong_provenance_and_decoded_secrets_never_reach_disk() {
        assert!(history(b"series_id\nGDP\n").is_err());
        let dir = tempfile::tempdir().unwrap();
        let (_, _, rec) = setup(dir.path(), rules_text());
        assert!(rec
            .capture("unsafe", &json!({"value":TEST_API_KEY}).to_string())
            .is_err());
        assert!(!dir.path().join("unsafe.response-text.json").exists());
    }
}
