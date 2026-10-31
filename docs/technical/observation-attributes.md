# Observation attributes and historical absence (ECO-333)

Observations retain provider status flags and footnotes alongside their exact nullable
value and revision identity. `DataPoint`, `NewDataPoint` and the storage-neutral
`Observation` expose `attributes: Option<serde_json::Value>`. PostgreSQL stores a
nullable JSONB object; the migration has no default and does not backfill history.
Unknown provider keys survive DTO conversion and serialization. Storage adapters must
preserve the complete object, not reserialize only fields known to a helper struct.
The Iceberg adapter uses its optional attributes field for the same contract.

Before this migration, `data_points` persisted observation UUID, series UUID, date,
nullable arbitrary-precision value, revision date, original-release flag, and creation
and update timestamps. Source/dataset/dimension provenance exists on the series and
catalog; crawl attempts record receipt/attempt timing at series level. No persisted
per-observation footnotes, status flags, preliminary or estimated markers exist in
that history. BLS parsed and logged footnotes but discarded them. Original-release
flags do **not** establish preliminary status. Backfill and restore must retain old
rows with null attributes; they must not synthesize historical flags from current
provider responses, values, dates or original-release status.

The typed producer helpers `ObservationAttributes` and `ObservationFootnote` describe
current BLS captures. Readers may inspect them, but the raw object remains the lossless
transport contract. Current keys are:

| Key | Meaning |
| --- | --- |
| `source` | Adapter source identifier (`BLS`) |
| `raw_value` | Verbatim provider value string, including missing-value markers |
| `footnotes` | Ordered code/text objects with any provider extension fields retained |
| `status_flags` | Verbatim footnote codes, in provider order |
| `preliminary` | Whether a captured footnote explicitly contains the BLS `P` marker |
| `estimated` | Whether a captured footnote explicitly says `Estimated` or `Estimate` (case insensitive, optional trailing full stop) |

SQL NULL/the outer JSON null means attributes were not captured. An empty object is
distinct from that unknown state. Within a captured BLS object, an omitted or null
provider footnote array gives null `footnotes`, `status_flags`, `preliminary` and
`estimated`. A supplied empty array or `[{}]` gives empty arrays and false marker
booleans. `{}` is only the provider's placeholder; supplied code/text strings, even
empty strings or unusual Unicode/whitespace, and extension fields are retained.
False means **not explicitly marked in the captured array**, not a claim that the
observation is final or was never estimated. Unknown survey-specific codes are
preserved verbatim and are not guessed to mean estimated. BLS documents the `P`
marker in its [API signatures](https://www.bls.gov/developers/api_signature_v2.htm).

A metadata-only change is persisted and counted as an updated existing point.
Identical value/attribute captures do not rewrite the row. A metadata-unaware fetch
with an unchanged value keeps existing captured metadata. If that fetch changes
the value, attributes become unknown rather than attaching the old value's flags
to the new value. All writes remain in the caller's existing transaction, including
lease-fenced worker commits. A provider with real vintages can append a revision
for a status-only change. BLS currently publishes no vintage history through this
adapter: its existing synthetic `revision_date = date` policy overwrites the current
row. Captured current metadata does not reconstruct past BLS preliminary states;
`asOf` retains the existing synthetic-vintage limitation.

GraphQL `DataPoint.attributes` and `LatestObservation.attributes` are nullable JSON.
Series pages, direct series reads, transformations and latest-observation summaries
retain metadata from the selected stored revision. Attribute flags describe the
source observation, not any derived transformation. `All`, `Original`, `Latest`
and `AsOf` select the same rows and metadata together. Catalog authorization and
remote routing remain the existing request-scoped policy.

The populated migration test verifies pre-migration IDs, values, revision flags and
timestamps survive unchanged with null attributes. Provider fixture tests cover
unknown and captured-empty metadata, missing numeric values, preliminary/estimated
markers and literal footnote extensions. PostgreSQL persistence tests exercise
metadata-only writes, deduplication, transaction rollback and unknown-state handling;
shared reader/API cases check exact metadata across revision modes and separated
or combined databases. Recovery/backfill parity must include null-versus-object
state, footnote ordering, extensions and literal strings in addition to exact values.
