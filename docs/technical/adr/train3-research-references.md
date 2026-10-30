# Durable research references (Train 3, ECO-791)

Company research and workspace consumers use `econ_graph_core::models::research_reference`.
The `AnnotationAnchor` envelope serializes version as the string `"1"`; unsupported
versions and unknown fields are rejected. At trust boundaries use `from_json`, which
also validates identities and periods. These references confer no access rights.

A series reference uses its **stored** UUID, including a pre-derived legacy UUID.
Never recompute a stored note target from source/external ID. A company uses its
canonical ten-digit SEC CIK, rather than a ticker or database row UUID. A filing
uses issuer CIK and SEC accession in its dashed 10-2-6 form. The accession prefix
may belong to a filing agent, so it need not equal the issuer CIK. Amendments have
distinct accessions and never replace an original filing reference.

An economic fact subject consists of issuer, expanded taxonomy namespace and
concept, period, unit, and dimensions. Instant and duration periods are distinct;
a duration includes both endpoints. Names are case-sensitive. Namespace URIs,
expanded unit measures, and expanded dimension axes/members must be supplied by
the parser; document-local prefixes and context labels are not portable identities.
Typed dimension values require canonical typed-member content. Dimensions are a
sorted map. Composite units must be represented as canonical expanded products and
divisors by the producer; this baseline does not parse XBRL or invent missing data.
`canonical_key()` validates and serializes the structured identity with a version
prefix, avoiding ambiguous delimiter concatenation. It is an external lookup key,
not a mandate to rewrite existing database IDs.

`Fact` deliberately identifies the economic subject across filings. `FiledFact`
also pins a filing; notes using it never follow a later revision. `Statement`
adds a canonical statement key within one filing. Existing financial notes lack
portable period/namespace/context metadata, so their adapter preserves the exact
statement and optional line-item UUID in `LegacyFinancial`. It must resolve by
stored UUID, never by choosing the newest filing or guessing a matching concept.
The chart adapter preserves its stored series UUID, chart association, date and exact nullable decimal position.
No shared-annotation table migration or cross-plane foreign key is required.

SEC bytea ingestion now treats a repeated company/accession as idempotent only
when its content hash and identifying metadata agree. It returns the existing row
UUID, including legacy random UUIDs. Changed content under one accession fails
without updating the old row. A distinct amendment accession creates a separate
row. This conservative rule preserves filed-note meaning until a later ingest
policy explicitly models within-filing corrections. Copies must preserve row IDs;
semantic references alone cannot reconstruct an old random UUID after deletion.

Resolution is request-authorized. Internally distinguish found, missing,
tombstoned, inaccessible and unavailable; expose missing/tombstoned/inaccessible
as the same `not_found` status. An outage is `unavailable` and must fail closed.
Keep the original reference and app-owned content in every unresolved state; data
removal or revoked access must not delete, rewrite or silently retarget notes.

Executable examples live in `research_reference_contract` and its versioned JSON
fixture. They cover transport agreement, legacy adapters, restatements, dimensions,
periods, units, namespaces and privacy status. The `filing_identity_contract` SEC storage integration
test additionally checks repeat-ingest IDs, changed-content refusal and amendments.
