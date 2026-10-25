# Failure-behavior coverage matrix

ECO-264's release gate: every backend integration point reachable in the release build, the failure cases
that apply to it, and the test that shows each is handled (or why a case doesn't apply). The
sampled browser checks are at the end. Test names are `file::test`; backend paths are under
`backend/crates/`, frontend paths under `frontend/`.

DB-backed Rust tests need `DATABASE_URL` and skip without it (CI sets it). Upstream HTTP is faked
with wiremock (`econ-graph-crawler/src/testkit`), Keycloak with `econ-graph-auth`'s `TestIssuer`;
nothing calls a live API.

Status: **covered** (a test exercises it), **n/a** (with the reason), **gap** (known, ticketed or
`test.fixme`), **covered by its own PR** (a separate PR implements and tests it).

## 1. Source HTTP client (`econ-graph-crawler/src/http.rs`)

Every source adapter fetches through `HttpFetcher`, so its classification and retry rules hold
for all six sources.

| Case | Status | Evidence |
|---|---|---|
| Timeout | covered | `http.rs::timeout_is_transient` (retried, then `Transient`) |
| Network failure (connection refused) | covered | `http.rs::api_key_is_redacted_from_transport_errors` (`Transient`, key redacted) |
| HTTP 401 / 403 | covered | `http.rs::auth_statuses_are_not_retried` (one request, `Auth`, not retryable); `error.rs::status_classification` |
| HTTP 404 / 410 | covered | `http.rs::not_found_is_not_retried`; `error.rs::status_classification` |
| HTTP 429, short and long `Retry-After` | covered | `http.rs::short_retry_after_is_honoured_in_process`, `long_retry_after_returns_rate_limited_immediately`; `retry_after_parsing` |
| HTTP 5xx, transient then persistent | covered | `http.rs::server_errors_then_success`, `persistent_server_error_gives_up_after_three_attempts` (bounded at three attempts) |
| 503 with `Retry-After` is a rate limit | covered | `error.rs::status_classification` |
| Backoff is bounded, jittered | covered | `http.rs::backoff_is_jittered_exponential`; `policy.rs::backoff_doubles_and_caps` |
| Malformed body | covered | `http.rs::malformed_json_is_parse_error` |
| Empty or incomplete body | covered | `http.rs::empty_and_incomplete_bodies_are_parse_errors` |
| Cancellation (caller drops the request) | covered | `http.rs::a_cancelled_request_frees_its_concurrency_slot` |
| Per-source rate limit and concurrency | covered | `rate_limit.rs` (13 tests); `http.rs::concurrency_is_limited_per_source` |
| Secrets never in errors | covered | `http.rs::api_key_is_redacted_from_{status,parse,transport}_errors`, `snippet_*` |
| Error metrics | covered | `http.rs::record_response_error_counts_it`, `endpoint_labels_are_low_cardinality` |
| Conditional GETs (304 handling) | covered | `http.rs::get_text_conditional_*`, `get_text_if_changed_*`, `a_304_to_an_unconditional_request_is_an_error` |

## 2. Source adapters: discovery and fetch (`econ-graph-crawler/src/sources/`)

Source-specific failure shapes on top of section 1 (in-body errors, missing keys, partial
batches, paging).

| Source | Failure cases covered |
|---|---|
| FRED (`fred.rs`) | missing key (`missing_key_is_auth_error_without_requests`), in-body 400s (`other_400_is_permanent`, `unknown_series_400_is_not_found`), empty response (`empty_seriess_is_not_found`, `empty_page_ends_paging`), paging bound (`observation_paging_is_bounded`), discovery partial failure, rate limit and auth (`discover_ids_*`) |
| BLS (`bls.rs`) | in-body statuses (`status_mapping`, `not_processed_threshold_is_rate_limited`, `series_does_not_exist_is_not_found`, `invalid_key_is_auth_and_key_never_in_error`), empty data (`empty_data_is_not_found_without_since_ok_with_since`), later-window and batch failures (`later_window_error_fails_the_group`, `batch_request_failure_fails_the_whole_batch`, `keyless_chunk_failure_applies_to_its_series_only`, `keyless_rate_limit_stops_further_requests`), omitted or duplicate series in a batch |
| BEA (`bea.rs`) | missing key, in-body errors (`in_body_errors_are_classified_and_key_redacted`, `classify_bea_error_by_description`), HTTP errors (`http_errors_map_by_status_and_redact_key`), empty tables (`discover_fails_on_a_table_with_no_rows`, `..._regional_table_with_no_areas`), batch group failure, stale tables keep points |
| Census BDS (`census.rs`) | missing key, malformed metadata (`parse_variables_rejects_*`, `parse_geographies_rejects_*`, `parse_state_file_rejects_bad_files`), missing reference files, `census_specific_errors` |
| FHFA (`fhfa.rs`) | file-level errors, short rows, unusable ids, row errors failing only their series, HTTP errors failing the batch, download contention |
| World Bank (`world_bank/tests.rs`) | failed request fails every id of its batch, paging bound (`page_count_is_bounded`), response shapes (`response_shapes`), discovery rate limit, auth and all-failed (`discover_aborts_on_rate_limit_and_auth`, `discover_fails_when_every_indicator_fails`), incomplete discovery retried (`an_incomplete_discovery_is_retried_in_full`) |

## 3. Queue: leases, retries, persistence (`econ-graph-crawler/src/worker.rs`, `econ-graph-core/src/models/crawl_queue.rs`)

| Case | Status | Evidence |
|---|---|---|
| Error kind decides retry vs fail | covered | `worker/tests.rs::server_error_retries_with_counted_attempt_and_backoff`, `rate_limited_retries_after_retry_after_without_counting`, `not_found_fails_permanently` |
| Retries are bounded | covered | `crawl_queue/tests.rs::test_retry_later_attempt_counting_and_failure`, `test_release_stuck_fails_item_at_max_retries` |
| Lease safety: one claim per item | covered | `crawl_queue/tests.rs::test_two_workers_never_claim_same_item`; `worker/tests.rs::two_workers_drain_twenty_items_exactly_once` |
| Lease safety: an expired lease can't write | covered | `worker/tests.rs::lost_lease_is_reported_and_leaves_new_owner_alone`, `expired_fetch_success_cannot_overwrite_newer_claims_observation`, `expired_fetch_failure_cannot_replace_newer_claims_success_status`, `expired_batch_*`, `expired_discovery_cannot_overwrite_or_insert_stale_metadata` |
| Crashed worker's items are released | covered | `worker/tests.rs::release_stuck_runs_on_start`; `crawl_queue/tests.rs::test_release_stuck_clears_locks` |
| Idempotency: duplicate enqueue, unchanged refetch | covered | `crawl_queue/tests.rs::test_enqueue_duplicate_while_active_returns_none`; `worker/tests.rs::refetching_unchanged_points_writes_nothing`, `unchanged_discovery_writes_and_retires_nothing` |
| Panicking adapter | covered | `worker/tests.rs::adapter_panic_is_retried_and_worker_survives` |
| Circuit breaker on repeated 429 / auth | covered | `worker/tests.rs::circuit_breaker_pauses_throttled_source_only`, `per_series_rate_limit_in_a_batch_counts_for_the_breaker`, `breaker_counts_one_result_per_batch_call` |
| Partial batch failure | covered | `worker/tests.rs::one_bad_series_fails_only_its_own_job`, `whole_batch_error_applies_to_every_job_with_usual_retries` |
| A failed fetch never overwrites stored data or reports success | covered | `worker/tests.rs::a_failed_refresh_keeps_the_stored_data_and_does_not_count_as_fresh` (5xx, 401, non-JSON and incomplete bodies: points, end date and last successful crawl unchanged, status `failed`, attempt recorded with its kind, coverage unchanged) |
| An empty response never deletes stored data | covered | `worker/tests.rs::an_empty_refresh_keeps_the_stored_points` |
| Cancellation: shutdown mid-job | covered | `worker/tests.rs::shutdown_mid_request_finishes_the_job_and_claims_no_more` |
| Legacy queue service | covered | `econ-graph-services/src/services/queue_service.rs` (17 tests, lease-guarded transitions) |

## 4. Database: transactions and pagination

| Case | Status | Evidence |
|---|---|---|
| Result and queue transition roll back together | covered | `worker/tests.rs::result_transaction_rolls_back_if_queue_completion_fails` |
| DB errors while persisting | covered | `worker/tests.rs::db_error_fails_unsynced_datasets_permanently_and_retries_the_rest`, `db_error_fails_dataset_key_collisions_permanently` |
| Cancelled read doesn't leak a transaction | covered | `status.rs::cancelled_status_does_not_leak_transaction_to_pool` |
| Unreachable database | covered | `database.rs::readiness_check_fails_when_the_database_is_unreachable`; `main.rs::readyz_answers_503_when_the_database_is_unreachable`, `livez_answers_200_even_when_the_database_is_unreachable` |
| Schema older than the binary | covered | `database.rs::readiness_check_rejects_a_schema_older_than_this_binarys_minimum` |
| Concurrent migrations | covered | `database.rs::test_migrations_block_while_another_session_holds_the_lock` |
| `seriesData` paging: bad arguments, true totals, transformed pages | covered | `series_data_tests.rs::rejects_bad_paging_arguments`, `long_series_reports_its_true_total_and_pages_to_the_end`, `transformed_pages_match_the_whole_series*` |
| A failed batch load is an error, not null | covered | `latest_observation_tests.rs::test_failed_batch_is_an_error_not_null` |
| Frontend paging: a missing, stalled or failing later page fails the read | covered | `src/hooks/__tests__/useSeriesDataPaging.test.tsx` (`a missing page after the first is an error, not a short series`, `a failing later page fails the whole query`, `a page that promises more without a new cursor is an error, not a loop`, cancellation) |

## 5. GraphQL: validation, authentication, authorization

| Case | Status | Evidence |
|---|---|---|
| Malformed and oversize request bodies | covered | `econ-graph-backend/src/main.rs::malformed_graphql_body_is_a_json_400`, `oversize_graphql_body_is_a_json_413`, `graphql_body_limit_counts_streamed_bytes` |
| Depth and complexity limits | covered | `schema.rs::query_deeper_than_the_limit_is_refused`, `query_one_over_the_complexity_limit_is_refused`; `security/depth_limit.rs`, `security/complexity.rs` |
| Input validation | covered | `security/input_validation.rs`; `mutation.rs::test_trigger_crawl_plan_rejects_invalid_input`, `collaboration_inputs_reject_caller_supplied_user_ids` |
| Invalid, expired, wrongly signed token: 401, never anonymous | covered | `main.rs::graphql_refuses_invalid_tokens_with_401` |
| Token that can't be checked (provider down): no privileges | covered | `main.rs::graphql_drops_a_token_it_cannot_check` (the request runs anonymous, so every write asks for sign-in) |
| Suspended account refused | covered | `main.rs::graphql_signs_in_a_valid_token_and_refuses_a_suspended_account` |
| Authorization fails closed per resolver | covered | `authorization_tests.rs` (14 tests: missing role denied, anonymous asked to sign in, denial before any DB access, no target-existence leak) |
| Annotation privacy | covered | `annotation_visibility_tests.rs` |
| GraphQL request rate limit (429) and per-request timeout | n/a | `security/rate_limit.rs` and `security/timeout.rs` are not on the request path (not wired in `econ-graph-backend`); their unit tests cover the components only |

## 6. Keycloak: discovery, JWKS, token verification (`econ-graph-auth/src/oidc.rs`)

| Case | Status | Evidence |
|---|---|---|
| Misconfiguration fails instead of disabling sign-in | covered | `malformed_settings_fail_instead_of_disabling_sign_in`, `required_without_issuer_is_an_error` |
| Discovery naming another issuer | covered | `discovery_must_name_the_configured_issuer` |
| Unreachable provider is "unavailable", not "invalid" | covered | `an_unreachable_provider_is_unavailable_not_invalid`, `a_new_key_the_provider_cannot_confirm_is_unavailable_not_invalid` |
| Key rotation and refetch throttling | covered | `a_rotated_key_is_picked_up`, `unknown_key_ids_do_not_refetch_more_than_the_interval_allows`, `a_key_rotated_while_refetches_are_throttled_is_not_called_invalid`, `a_stale_cache_keeps_answering_and_refreshes` |
| Wrong issuer, audience, times, algorithm, key, token type | covered | `wrong_issuer_audience_or_times_are_rejected`, `only_rs256_signed_by_a_published_key_is_accepted`, `only_access_tokens_are_accepted`, `encryption_and_non_rsa_keys_are_not_signing_keys` |
| Browser session: refused vs transient renewal, stale callback, unreachable Keycloak | covered | `src/contexts/__tests__/AuthContext.test.tsx` (28 tests); `src/pages/__tests__/AuthCallback.test.tsx` |

## 7. Coverage, refresh metrics and alerts

| Case | Status | Evidence |
|---|---|---|
| Coverage counts, not-found exclusion | covered | `coverage.rs::coverage_from_seeded_rows`, `not_found_series_excluded_unless_it_has_data` |
| A failed refresh doesn't count as fresh | covered | `worker/tests.rs::a_failed_refresh_keeps_the_stored_data_and_does_not_count_as_fresh` |
| Failed series back off, not hammered | covered | `scheduler.rs::failed_series_back_off_twice_the_interval`, `not_found_series_back_off_ten_times_the_interval`, `failed_discovered_series_back_off_twice_the_interval` |
| Metrics endpoint and gauges | covered | `econ-graph-crawler-worker/src/metrics.rs` (`healthz_follows_alive_flag_and_unknown_path_is_404`, `queue_gauges_from_seeded_rows`, `coverage_gauges_set_and_removed`) |
| Coverage alerts (low, not refreshed, crawl failed, stale) | covered | `k8s/monitoring/tests/crawler-coverage.test.yaml` (promtool) |
| Failure alerts (worker down, metrics missing, no completions, source failing, error rate) | covered | `k8s/monitoring/tests/crawler-failures.test.yaml` (promtool) |
| Alert routing | n/a | no Alertmanager is deployed (`prometheus-rules-crawler.yaml`, KNOWN GAP) |

## 8. Sampled browser checks (release e2e, `tests/e2e/release/failures/`)

Run against the seeded backend with chosen requests failed in the browser (`failures/faults.ts`).
Component tests hold the detailed permutations (`src/pages/__tests__/SeriesDetail*.test.tsx`,
`SeriesExplorer*.test.tsx`, `src/components/global/__tests__/WorldMapTab.test.tsx`,
`AuthCallback.test.tsx`, `AuthContext.test.tsx`).

| Acceptance case | Status | Spec |
|---|---|---|
| Search failure is actionable and searching again recovers | covered | `series-failures.spec.ts`: a failed search says so, and searching again recovers |
| Series and observations load failure, Retry recovers | covered | `series-failures.spec.ts`: a series that fails to load offers Retry; observations that fail to load show no values |
| Empty result is distinct from an error, no stand-in values | covered | `series-failures.spec.ts`: a search that matches nothing is an empty result |
| Transformation failure never labels levels as transformed | covered | `series-failures.spec.ts`: a failed transformation never labels the levels as transformed |
| Annotation save/delete failure, accurate state, retry without duplicates | covered | `annotation-failures.spec.ts`: a failed save keeps the form open; a failed delete keeps the row |
| Sign-in, callback and session failure: signed out with a recovery path | covered | `sign-in-failures.spec.ts` (unreachable sign-in service, failed callback, session ended at Keycloak) |
| Map loading failure, Retry recovers (sampled once) | covered | `map-failures.spec.ts`; the outline's own failure is `world-map/world-atlas.spec.ts` |
| Failed source listing (browse, no query) has a Retry | **gap** | `series-failures.spec.ts`, `test.fixme`: the explore page's error has no Retry and the Search button needs a query, so only a reload recovers |
| Saving an annotation again after a lost response | covered | `annotation-failures.spec.ts`: the backend stored the first request and the page reported a failure; the second Save reuses the create's idempotency key and stores nothing new |
