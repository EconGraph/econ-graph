# Train 5 company research delivery

Train 5 delivers company research from SEC companyfacts: company lookup, three
normalized statements, financial history, CSV, ratios, SIC peers and local notes.
The release is v4.3.0. [ECO-8](https://linear.app/econgraph/issue/ECO-8) and
[ECO-116](https://linear.app/econgraph/issue/ECO-116) define the release scope.
Share prices and OAuth company MCP tools remain outside this train. Original
statement layouts, extension concepts and segments follow in Train 6.

## Dependencies and ownership

```mermaid
flowchart TD
    B[Train 3 accepted main and tag] --> C[ECO-602 fact contract]
    C --> I[ECO-776 nightly ingestion]
    C --> P[ECO-777 periods and standard concepts]
    I --> A[ECO-778 authorized APIs]
    P --> A
    A --> U[ECO-779 company pages and CSV]
    A --> R[ECO-780 ratios and SIC peers]
    A --> N[ECO-781 local notes]
    U --> E[ECO-782 release acceptance]
    R --> E
    N --> E
    T[Train 4 production platform and data access] --> E
    L[Seven consecutive nightly loads and alert evidence] --> E
```

Ingestion and calculations can develop concurrently once their shared fact
contract is reviewed. API design can proceed against that contract; integration
must exercise persisted facts and real calculations. Pages, ratio presentation
and notes join the API contract before the complete research workflow runs.

| Package | Branch | Review focus |
| --- | --- | --- |
| Fact identity and schema | `feat/train5-companyfacts-contract` | Decimal precision, revisions, Train 6 compatibility |
| Bulk ingestion | `feat/train5-companyfacts-ingest` | Idempotency, interruption recovery, archive bounds, freshness |
| Financial calculations | `feat/train5-companyfacts-calculations` | Actual periods, compatible inputs, missing data, provenance |
| Company APIs | `feat/train5-companyfacts-api` | Authorization, bounded queries, schema and data ownership |
| Company pages | `feat/train5-companyfacts-ui` | Reachable controls, attribution, exact CSV, honest states |
| Local notes | `feat/train5-company-notes` | Durable anchors, legacy compatibility, owner permissions |
| Release acceptance | `test/train5-company-acceptance` | Independent fixtures, real workflow, operational evidence |

The lead integrates reviewed commits into `feat/train5`. Each implementation
package has its own worktree. Independent reviewers inspect contracts and final
diffs, and the lead assigns fixes back to the owning package. Reviewers do not
approve their own implementation. All pull requests require passing CI for the
latest commit and disposition of automated review comments. No merge or production
deployment is authorized by this coordination work.

## Baseline and integration gates

Development currently uses a **provisional** combination of current main and
`integration/train3-split-validation`. The seven Train 3 prerequisite PRs
[#772](https://github.com/EconGraph/econ-graph/pull/772) through
[#778](https://github.com/EconGraph/econ-graph/pull/778) remain upstream
dependencies. This is not the accepted branch cut: Train 3 requires accepted main,
its exact-commit validation and the immutable `train-3` tag. Reconcile the feature
branches with that accepted baseline before claiming Train 5 readiness.

Train 4 remains an integration and production-release gate. Consume its delivered
data-service authorization and platform contracts without importing its unfinished
implementation into each company package. Preserve the split between app-owned
notes and data-owned companies, filings and facts; never introduce a cross-plane
foreign key.

## Data and calculation contracts

Companyfacts has dedicated storage. Its subject identity preserves issuer CIK,
source taxonomy family, case-sensitive concept, raw unit and explicit instant or
duration endpoints. Filing accession identifies a revision. Filing fiscal labels
and calendar frames do not define the economic period. Numeric values stay exact;
conflicting values for the same subject and accession must not silently replace
evidence supporting a filed note.

Companyfacts taxonomy families are distinct from the filing-specific namespaces
used by the future parser. Any reference adapter must make that distinction
explicit. New ingestion metadata must not cause the existing raw-filing crawler
to skip a filing it has not downloaded.

Derived Q4 and TTM figures expose their input facts and revision cutoff. Missing
or incompatible periods produce an unavailable result. Per-share and weighted
average share figures cannot use naive subtraction or summation. Standardized
companyfacts statements must not claim to reproduce the original filing layout.

## Release acceptance

The release gate requires a signed-in researcher to find a company, inspect all
three statements and supported periods, chart any line, export exact CSV, inspect
ratios and peers, recognize restatements, and create and retrieve local notes.
Another user must not gain access to private notes. Every retained control must
perform its advertised action.

Acceptance also requires 20 fixed companies checked against their source facts,
the balance-sheet equation holding in at least 99 percent of checked periods,
freshness within a day, and seven consecutive successful nightly loads with
failure-alert evidence. Fixture runs and repeated executions in one day do not
substitute for the nightly evidence. Keep incomplete company pages gated until
the complete release bar is met.
