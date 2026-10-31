# SEC companyfacts evidence and research identity

This decision settles ECO-602 for Train 5. Company research keeps source facts in
dedicated data-plane tables. It does not migrate SEC facts into generic datasets,
series, or Iceberg, and it does not change the existing raw filing/XBRL tables.

## Why dedicated facts

An SEC fact has a duration or instant, a taxonomy concept, a unit, and a filing
revision. A single observation date cannot distinguish a quarterly duration from
a year-to-date duration ending on the same day. Filing dates and accessions also
have different meanings: several filings may report the same economic subject,
including unchanged comparative facts. Keeping these distinctions explicitly
avoids silently overwriting restatements or retargeting notes.

`CompanyFactSubject` identifies whole-entity companyfacts evidence by ten-digit
CIK, source taxonomy family, source concept, exact period and source unit token.
An absent start means an instant; a present start means a duration, including
equal start/end dates. The model preserves reported numeric sign independently
of taxonomy balance. Values use unconstrained PostgreSQL `NUMERIC` and Rust
`BigDecimal`; wire values are decimal strings.

`CompanyFact` identifies a revision by subject plus the SEC accession in its
10-2-6 digit form. Its stable UUID is UUID v5 over the versioned canonical JSON
revision key. Filed date, fiscal focus (`fy`/`fp`), calendar frame, label and
description are metadata rather than identity. In particular, a comparative
fact's `fy`/`fp` do not identify that fact's economic period. Repeated source rows
with the same revision key and equal values may coalesce metadata; different
values or incompatible filed/form provenance must fail validation rather than
replace a pinned revision. An omitted fact in a later bulk download does not
authorize deleting retained facts or note targets.

## Taxonomy and unit boundaries

Companyfacts reports taxonomy families such as `us-gaap` and units such as
`USD/shares`. It does not supply an authoritative filing-specific taxonomy
namespace URI/version, raw context IDs, all segment contexts or linked footnotes.
Store its source tokens losslessly; do not invent any of those missing fields.

The adapter into the shared Train 3 `FactSubject` uses explicit logical source
namespaces, `urn:econgraph:sec-companyfacts:taxonomy:<encoded-family>` and
`urn:econgraph:sec-companyfacts:unit:<encoded-unit>`. Tokens use UTF-8 percent
encoding. These URNs are application identifiers and never claim to be the
original XBRL namespace or expanded XML unit name. Dimensions are empty because
these are whole-entity companyfacts facts. Train 6 raw-parser facts keep their
own authoritative namespace, units and dimensions; any equivalence with these
logical subjects requires an explicit validated mapping. A future mapping must
preserve existing source keys and notes through an alias/lookup rather than
rewriting their targets.

## Storage and consumers

`company_facts` has a unique subject/accession key with null-safe period-start
comparison. `company_fact_filings` retains submissions metadata separately from
`financial_statements`: inserting placeholder statements would cause the
existing raw filing crawler to skip downloads. Neither table has an app-plane
foreign key, so removing data cannot cascade into user-authored notes. Existing
companies and financial statements retain their UUIDs and legacy notes continue
using their existing compatibility adapters.

Consumers select original, latest, an explicit accession, or a filed-date cutoff
explicitly. Latest/original selection orders by filed date and then accession
for a deterministic same-day tie. Restatement visibility requires retaining
earlier revisions; an unchanged comparative value is additional provenance,
not evidence of a value change. Derived Q4/TTM values carry their input revisions
and are never inserted as fabricated filed facts. Calculations must match
periods, concepts and units and report missing/incompatible inputs explicitly.

`company_fact_load_runs` records each bulk attempt and its completion status,
counts, source digests and failure. Its `source_mode` distinguishes upstream
`downloaded` archives from operator `staged` files. Successful downloaded runs
must retain both upstream Last-Modified timestamps, each within 24 hours before
completion and at most five minutes in the future. Running or failed attempts
and all staged imports are ineligible freshness observations. Existing attempts
without source provenance migrate to staged conservatively. This evidence
supports the release requirement for seven consecutive nightly successes;
implementing the ledger does not by itself satisfy that operational gate.

Executable examples in `models/company_fact.rs` verify exact decimal round trips,
economic-period/unit/taxonomy separation, amendment identities and shared note
references. Database checks verify that instant duplicates conflict, durations
remain distinct, and source numeric precision survives storage.

## API access during rollout

Company research reads require the explicit `catalog.sec:read` capability and the
`financial_components` build flag. The source's exact catalog name is `SEC EDGAR`.
A fresh realm grants SEC reads to signed-in users, while anonymous access remains
ungranted. Existing realm imports preserve live catalog grants deliberately; they
do not automatically expand a live user's privileges. After acceptance, an
operator must explicitly grant `catalog.sec:read` to the intended existing realm
role using the normal Keycloak catalog-grant tooling. Release builds keep the
feature disabled until the full company-research acceptance gate passes.

The same schema snapshot describes development and release builds. A disabled
company resolver returns `FEATURE_DISABLED` before opening a pool; an enabled
resolver checks the SEC catalog grant before reading the data database. Source
facts, companies and filings use the data plane. Local authored notes use the app
plane and validate new attachments through an authorized data-plane target check.
The Train 4 operation classifier must admit all shipped company query roots before
this surface can pass the independent HTTP data-plane boundary.

Company fact queries select a revision before pagination. Clients may explicitly
request `includeRevisions: true` to inspect all retained filings under the query's
as-of/accession filter. Statement calculations retain all source revisions before
selecting operands, allowing changed-value restatements to remain visible. Period
and unit filters never interpret SEC fiscal focus or calendar-frame labels as the
economic period's identity. SIC peer comparisons use the identical requested dates
and expose missing inputs rather than substituting a nearby calendar period.
