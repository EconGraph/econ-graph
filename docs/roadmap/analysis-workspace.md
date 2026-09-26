# Roadmap: analysis workspace (multi-series charts, saved charts, export)

Status: accepted for train 1 (2026-09-26). Later phases are proposals.

Train 1 of the [release plan](./releases.md) removes `/analysis` from the build and ships
annotations and a CSV download on the series page. This doc covers what train 1 leaves
out: charts with more than one series, charts a user can save and share, chart
export beyond CSV, and whether a separate analysis page should exist at all.

## Goal

A user can put several series on one correct chart, save it, come back to it, share it
with a colleague or a link, and take it out of the product as data or an image. Every
number on the chart comes from the backend, and the chart looks the same for everyone
who opens it.

The long-term direction is collaborative analysis (Joe, 2026-09-26): people discuss
specific details, such as one data point, a date range or one line of a financial
statement, not only a chart as a whole. Real-time collaboration comes later, but the
data model for notes on details comes first, so later phases add features rather than
migrate.

## Where things stand today (main at `140fadf`)

### The `/analysis` page

`pages/ProfessionalAnalysis.tsx` (584 lines, route `/analysis/:id?`) runs entirely on
data typed into the file: `SAMPLE_SERIES` (GDPC1, UNRATE and CPIAUCSL, 16 points each),
`MOCK_COLLABORATORS` (four made-up people) and three mock annotations with mock comments.
It ignores its `:id` route parameter. It draws with `components/charts/ProfessionalChart.tsx`
and shows collaboration through `components/charts/ChartCollaboration.tsx`, which is
local state only.

`ProfessionalChart` has three problems beyond the sample data:

- **Secondary series are plotted by array index, not by date.** The x-axis is a
  `category` axis whose labels are the primary series' dates
  (`ProfessionalChart.tsx:223`), and each secondary series is drawn as
  `series.data.map(point => point.value)` (`:246`). A monthly series against a
  quarterly one puts month 5 at quarter 5. Any real multi-series chart would be wrong.
  The series page chart (`InteractiveChartWithCollaboration.tsx:547`) already uses a
  `time` axis, so the fix is known.
- **The technical analysis is stock-trading tooling.** SMA, EMA, Bollinger bands, RSI and
  rate of change (`utils/technicalAnalysis.ts`) assume a daily price series. On monthly
  or quarterly macro data a 20-period Bollinger band spans five years and RSI means
  nothing. "Economic cycles" is a peak finder, and the event overlay is 7 events
  hard-coded in the same file.
- **Export does nothing.** `exportChart` (`:507`) is an empty callback behind a visible
  button.

### Charts and sharing in the backend

There is no `charts` table. Two tables refer to one anyway:

- `chart_annotations.chart_id` is a nullable UUID with no foreign key ("for custom chart
  groupings"). Annotations are created with `chart_id = NULL` and a `series_id` stored
  as `VARCHAR(255)`, although series ids are UUIDs.
- `chart_collaborators.chart_id` is a non-null UUID with no foreign key. `role` is a
  `VARCHAR(20)` and `permissions` a JSONB blob that no permission check reads.

The collaboration API in `econ-graph-graphql` (`createAnnotation`, `addComment`,
`shareChart`, `deleteAnnotation`, `annotationsForSeries`, `commentsForAnnotation`,
`chartCollaborators`) is not safe to put in front of users:

- **Every mutation takes the acting user from its input, not from the token.**
  `CreateAnnotationInput.user_id`, `AddCommentInput.user_id`,
  `DeleteAnnotationInput.user_id` and `ShareChartInput.owner_user_id`
  (`graphql/mutation.rs:66-160`, `graphql/types.rs:900-952`). Anyone can annotate,
  comment, delete or share as anyone else, without signing in.
- **`annotationsForSeries(userId)` takes the viewer from an argument**
  (`graphql/query.rs:266`), so passing another user's id returns their private
  annotations.
- **Sharing grants itself.** `check_admin_permission` returns `true` when the caller has
  no row for the chart (`collaboration_service.rs:354-379`), so anyone can share any
  chart id, including with themselves as admin.
- **`commentsForAnnotation` checks nothing**, so private annotations' comments are
  readable by id.
- **`chartCollaborators(chartId)` checks nothing** (`graphql/query.rs:310-325`), so
  anyone can list any chart's collaborators.
- **"Public" and "hidden" are one column.** `createAnnotation` stores `is_public` in
  `is_visible`, which the UI also uses to hide an annotation from view.

### The series page's collaboration panel

`SeriesDetail` uses `InteractiveChartWithCollaboration`, which opens
`ChartCollaborationConnectedQuery`. That panel can't work either:

- Its chart id is a string built from the series id, transformation and dates
  (`InteractiveChartWithCollaboration.tsx:310`). The backend parses chart ids as UUIDs,
  so `chartCollaborators` always fails.
- It queries `annotationsForChart(chartId)` (`utils/graphql.ts:314`), which the backend
  doesn't have.
- Its share mutation is a stub that reports success without calling the backend
  (`ChartCollaborationConnectedQuery.tsx:279-285`).

Five chart components add up to about 3,300 lines: `ProfessionalChart`,
`InteractiveChart`, `InteractiveChartWithCollaboration`, `ChartCollaboration` and
`ChartCollaborationConnectedQuery`.

## Decisions

### 1. Is there a separate analysis page?

| Option | For | Against |
|---|---|---|
| A. Bring `/analysis` back as a workspace page beside the series page | Matches the original "professional" pitch | Two chart views of the same data, which is how today's page drifted into mock data. No job for it that a chart can't do |
| **B. One chart view. A series page is a chart with one series; "Add series" turns it into a chart (recommended)** | One component to keep correct. The user's path is search, open, compare, which is how FRED graphs work | The series page's metadata panel has to give way once a second series is added |
| C. A notebook-style workspace (cells of charts, notes and formulas) | Powerful for research | A large product of its own, and Python notebooks with the Flight endpoint (federation phase 6) serve the users who want it |

With B, the routes are `/series/:id` (one series plus its metadata, as today) and
`/chart` (any number of series). An unsaved chart keeps its whole state in the URL,
for example `/chart?s=<uuid>:yoy,<uuid>&from=2000-01-01`, so it can be bookmarked and
pasted without an account. A saved chart lives at `/chart/:chartId`. "Workspace" means
the "My charts" list, not a separate page.

### 2. What a multi-series chart does

- **Time axis, always.** Each series is drawn as `{x: date, y: value}` points on a
  Chart.js `time` axis, at its own frequency. Nothing is aligned by index.
- **Transformations stay per series and server side.** The existing
  `DataTransformationType` (none, YoY, QoQ, MoM, percent change, log difference) is
  applied by `seriesData` for each series. The browser receives plain JSON arrays (Joe's
  decision) and does no math.
- **Units decide the axes.** Series with the same unit share an axis. Two units get a
  left and a right axis. A third unit is refused, with a suggestion to switch to
  "index to 100 at date X" or percent change. Two axes are the most a reader can follow.
- **Up to about eight series.** Beyond that the chart is unreadable and the query is
  large. The limit belongs in the GraphQL complexity budget (release item 4), not only
  in the UI.
- **Recession shading instead of an event list.** US recession bars come from real data
  (FRED's `USREC` series), not a hard-coded list. Other events come from the curated
  events file in [global-analysis.md](./global-analysis.md) decision 5, so there is one
  list, not two.
- **Technical analysis is compiled out, not deleted.** The indicator math (SMA, EMA,
  Bollinger bands, RSI, rate of change, the peak finder) isn't known to be wrong, only
  a poor fit for monthly and quarterly data. Joe prefers compiling code out to deleting
  it unless it's known bad. So `utils/technicalAnalysis.ts` stays in the tree. Once
  `ProfessionalChart` is deleted nothing imports it, so it is already absent from the
  bundle. A build flag `technical_analysis` in the flag file (`kind: build`, `remove_by:
  unscheduled`, off in every profile), compiled in through the flags area's build-time define, following
  [feature-flags.md](./feature-flags.md), gates any future indicator overlay on the one
  chart component. The overlay would be computed on the transformed series the server
  returns. The file has no tests today, and the flag PR adds them. The same file's
  `calculateCorrelation` and `calculateStandardDeviation` don't come back on the
  client: correlation is server work (decision 5). If users ask for smoothing, a
  centered moving average is also a candidate for a server-side transformation.
- **Formulas and frequency conversion come later.** "Series A divided by series B" and
  "monthly to quarterly average" need the series aligned to one frequency, which is
  server work: an aggregation step before the transformation. That is decision 5.

### 3. What a saved chart is

A new app-side table (it belongs to the app plane in [federation.md](./federation.md),
and refers to series by id only). `uuidv7()` is PostgreSQL 18's built-in, which `main`
requires since the uuidv7 shim was dropped:

```sql
CREATE TYPE chart_visibility AS ENUM ('private', 'link', 'public');

CREATE TABLE charts (
    id            UUID PRIMARY KEY DEFAULT uuidv7(),
    owner_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title         VARCHAR(200) NOT NULL,
    description   TEXT,
    spec          JSONB NOT NULL,          -- versioned, see below
    visibility    chart_visibility NOT NULL DEFAULT 'private',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

`spec` holds `{ "version": 1, "series": [{ "id", "transformation", "axis", "color" }],
"from", "to", "indexDate" }`. It is the same state the `/chart` URL carries, so saving
is "write the URL state to a row". The spec is validated by one Rust type on write, and
a `version` field lets it change later without a migration. Series ids in the spec are
checked to exist when the chart is saved; a series deleted later shows as "no longer
available" rather than breaking the chart.

The same PR fixes the tables that point at charts:

- `chart_annotations.chart_id` and `chart_collaborators.chart_id` get foreign keys to
  `charts(id)` with `ON DELETE CASCADE`.
- `chart_annotations.series_id` becomes a UUID (no production data, so no migration of
  rows).
- `chart_collaborators.role` becomes a native enum (`viewer`, `commenter`, `editor`),
  per Joe's rule on enum columns. The unused `permissions` JSONB is dropped. Ownership
  lives in `charts.owner_id`, not in a collaborator row.
- `chart_annotations.is_visible` becomes a native `visibility` enum (private or public)
  in train 1, in the auth area's work. Phase 5 adds `is_hidden` (the author's own view
  toggle).

**Workspaces are not a new object.** "My charts" (charts I own) and "Shared with me"
(charts with a collaborator row for me) cover the need. Folders, tags and dashboards
(a grid of charts) wait until someone has enough charts to need them.

### 4. Sharing and permissions

- **The acting user always comes from the verified token.** The `user_id` and
  `owner_user_id` fields leave every input, and `annotationsForSeries` loses its
  `userId` argument. This is a security fix, not a feature. See train placement below:
  it can't wait for saved charts.
- **Global capability from the token, object access from the database.** This follows
  [auth-plans-permissions.md](./auth-plans-permissions.md): the token's fine-grained
  roles say whether a user may create charts (`chart:create`), share them
  (`chart:share`) or export images (`export:image`). The `charts` and
  `chart_collaborators` rows say whether they may do it to this chart.
- **Access rules.** The owner can do anything. An `editor` can change the spec and
  annotations. A `commenter` can comment. A `viewer` can read. `link` visibility lets
  anyone with the URL read the chart, signed in or not. `public` also lists it (later,
  for a gallery or search). Sharing by email with someone who has no account stores a
  pending invite that resolves on first sign-in through Keycloak.
- **Annotations attach to details, not only to charts.** An annotation has one anchor:
  - `data_point`: a series and a date (what `chart_annotations` stores today);
  - `range`: a series and a start and end date, for a recession or a policy period;
  - `chart`: a saved chart as a whole.

  The SEC financial data roadmap (#193) defines its own anchors: `company_fact` (a CIK,
  concept, unit, period and dimensions, optionally pinned to one filing's accession
  number so a note stays on the reported version of a restated value) and `filing` (a
  CIK and an accession number). They join the same table only if the shared-table
  option below is chosen.

  Data point and range annotations follow the series. They show on the series page and
  on every chart containing that series, for viewers who can see them: public ones for
  everyone, private ones only for the author. A chart annotation follows that chart's
  sharing. A CHECK requires the columns of exactly one anchor kind to be set.
- **Anchors use natural keys, never data-side row ids.** A data point is the series'
  stable id plus a date, not a `data_points` row id, with an optional `revision_date`
  to name the vintage the note is about. Annotations live on the app plane in
  [federation.md](./federation.md), and observations move to Iceberg there, so a row id
  would break at the plane split or at the next data reload. Natural keys survive both.
- **One annotation model or two: deferred.** The SEC side has its own tables:
  `financial_annotations` and `annotation_replies`, with threads and a status (the
  Rust models also carry mentions and highlights that the schema lacks). The chart
  side has `chart_annotations` and `annotation_comments`.
  Both describe a threaded note on a detail. One proposal is a single `annotations`
  table with a native `anchor_kind` enum and one comments table, shared with the SEC
  roadmap (#193), which would keep its `highlights` JSON in its own anchor columns.
  The SEC roadmap records it as the leading candidate, not a commitment. Joe deferred
  the choice (2026-09-26): it is settled when saved charts (phase 5) or SEC phase 6
  start, whichever comes first. Until then each side keeps its own tables,
  and both follow the natural-key rule above, so either design stays open.
- **Collaboration is asynchronous first.** Comments, replies, mentions and resolved
  threads come with phase 5. Live presence (online dots, "who's viewing", real-time
  cursors, as in `CollaborativePresence` and the mock's `isOnline`) is the later step
  toward collaborative analysis. It needs a subscription transport the backend doesn't
  have yet, so it is phase 7.

### 5. Frequency alignment and formulas

A later GraphQL field, `chartData(spec)`, returns every series aligned to one frequency
(the lowest in the chart, or one the user picks) with an aggregation per series
(average, end of period, sum), and accepts formula series such as `a / b * 100` over
the aligned inputs. It runs on the server so the browser still gets plain arrays, and
it moves onto the `TimeSeriesStore` trait (federation phase 1) with no API change.
Correlation between two series (a number shown on the chart) is the same aligned read
plus one calculation, on changes rather than levels, matching
[global-analysis.md](./global-analysis.md) decision 4.

### 6. Export

| Format | How | When |
|---|---|---|
| CSV, one series | `seriesData` as a file, with source, units, transformation and retrieval date in header rows | Train 1 (release item 11) |
| CSV, a chart | One row per date, one column per series, blanks where a series has no value. With frequency alignment (decision 5), one aligned table | With saved charts |
| PNG | Chart.js `toBase64Image()` in the browser, with the title, the sources and the URL drawn into the image | With saved charts |
| Embed and image URL | A server-rendered PNG at `/chart/:id.png`, for pasting into documents and slides | Later, only if asked for. It needs a headless Chart.js renderer on the server |
| SVG, PDF, Excel | Not planned | The PNG and the CSV cover the uses at a fraction of the cost |

Every export carries source attribution. Some sources' terms of use ask for it (to be
checked per source in the data source docs), and a chart without its source is the
first thing a reader of a report asks about.

## Phases

1. **Remove what can't work** (train 1, release item 17). Code that is known to be
   wrong is deleted:
   - `ProfessionalAnalysis` and the `/analysis` route. The page shows only typed-in
     sample data and ignores its `:id`.
   - `ProfessionalChart`. It plots secondary series by index, and its export button
     does nothing.
   - `ChartCollaboration`. Its collaborators, annotations and comments are local mock
     state.

   Code that works but doesn't fit train 1 stays, unimported and so out of the bundle:
   the indicator functions in `utils/technicalAnalysis.ts`. A follow-up PR registers
   the `technical_analysis` build flag for any future overlay and adds the file's first
   tests. The hard-coded event list moves into the global analysis events file if
   that lands first; otherwise it stays in the same file.
2. **Make the collaboration API safe** (train 1, before release item 14). Take the
   acting user from the token in all four mutations and `annotationsForSeries`. Remove
   the "no row means admin" fallback. Check annotation visibility in
   `commentsForAnnotation`, and restrict `chartCollaborators` to the chart's
   collaborators. Tests for each, including one that a request with another
   user's id in the body is refused. It is its own PR (#194), release item 3,
   alongside the other security items 1 and 2. With that fix,
   `shareChart` refuses every call: no chart records an owner, so nobody can prove
   the right to share. Sharing stays off until phase 5.
3. **One series chart component** (train 1, with release items 7 and 14). Turn
   `InteractiveChartWithCollaboration` into one `SeriesChart` on a time axis, and
   delete `InteractiveChart`, which only its own test imports. Replace
   `ChartCollaborationConnectedQuery` with an annotation panel that calls
   `annotationsForSeries`. The panel's sharing tab is removed until phase 5. Train 1
   offers no hide toggle. The auth area replaces `is_visible` with a `visibility`
   enum in train 1, and phase 5 adds a separate hide flag.
4. **Multi-series chart on the URL** (train 2). `/chart?s=...` with the rules in
   decision 2, "Add series" on the series page, recession shading from `USREC`. No
   account needed. Schema check (release item 18) covers the new operations.
5. **Saved charts and sharing** (train 2). The `charts` table, whose `owner_id` column
   is the prerequisite for turning sharing back on, and the fixes to the two
   existing tables, `saveChart`, `chart`, `myCharts`, `sharedWithMe`, `shareChart` with
   invites, and the "My charts" page. Data point, range and chart anchors with
   threaded replies, mentions and resolved status, after settling whether they share
   one table with the SEC annotations. Chart CSV and PNG export.
6. **Aligned data and formulas** (train 3, after federation phase 1). `chartData` with
   frequency conversion and formula series, the aligned chart CSV, and the correlation
   figure.
7. **Later, on demand.** Live collaboration (presence and live updates through GraphQL
   subscriptions), server-rendered image URLs and embeds, a public chart gallery,
   folders or dashboards.

## Placement on the release trains

| Train | Phases | Why there |
|---|---|---|
| 1 (`v4.0.0`) | 1, 2, 3 | Removing `/analysis` is already train 1 item 17. Phase 2 is a precondition of item 14: sign-in makes the collaboration API reachable, and today it trusts user ids from the request body |
| 2 (`v0.3.0`) | 4, 5 | The release plan already lists saved charts in train 2. Both need Keycloak accounts (train 1) and nothing else |
| 3 (`v0.4.0`) | 6 | Frequency conversion is a store-level read, so it lands on the `TimeSeriesStore` trait (federation phase 1), and it shares its correlation code with the global analysis comparison (also train 3) |

## Where this challenged the release plan

The release plan has since taken the first two points in.

- **Annotations on "the existing backend".** That backend let any caller act as any
  user, and sharing granted itself. Phase 2 now lands first, as release item 3.
- **Train 1 had nothing to share.** Sharing in today's schema is per chart id, and
  there is no chart table; the series page invents a string id the backend rejects.
  Sharing moved to train 2 (release item 14).
- **The original `/analysis` pitch was the wrong target.** "Bloomberg Terminal-level"
  technical analysis is for traders reading daily prices. The users this data serves
  compare macro series across frequencies, transform them and cite them. Correct
  alignment, transformations, recession shading and clean export matter more than
  indicators, so the indicators are kept out of the build until someone asks for them.

## Not planned

- Trading indicators (SMA, EMA, Bollinger bands, RSI) and the peak finder in the
  default build. They stay in the tree, unimported, behind the `technical_analysis` build flag.
- A second chart library. Series charts stay on Chart.js (Joe's decision).
- Client-side data math. The browser gets arrays and draws them.

## Decisions taken

Joe asked on 2026-09-26 for release 1 to proceed autonomously on the recommended options.
These were the open questions, and the option taken:

1. **Sharing in train 1: no.** Train 1 ships private and public annotations only, and
   sharing moves to train 2 with saved charts. The release plan's item 14 already says
   so. The other options were pulling a minimal `charts` table into train 1, or dropping
   annotations from train 1 too.
2. **No separate analysis page** (decision 1, option B).
3. **The trading indicators stay in the tree, unimported, with the `technical_analysis`
   flag off by default.** The page,
   `ProfessionalChart` and `ChartCollaboration` are deleted because each is known to be
   wrong (see phase 1).

## Train 1 pull requests

Phases 1 and 3 are these PRs. Phase 2 is #194 in the security work. The same list
covers the other series-page and dashboard items in the release plan (7, 8, 9, 11).
Each PR updates its row here when it merges or changes scope.

| Id | Change | Phase or release item | PR | Status (2026-09-26) |
|---|---|---|---|---|
| UI-1 | Delete `/analysis`, `ProfessionalChart`, `ChartCollaboration` | Phase 1, item 17 | #199 | Ready |
| UI-2 | `technical_analysis` build flag and first indicator tests | Phase 1 | | Waits for UI-1 and the frontend flags PR |
| UI-3 | `seriesByExternalId(sourceName, externalId)` (exact `data_sources.name`) and `latestObservation` (newest date at its newest revision; null value kept), batched with a non-cached dataloader | Item 8 | #200 | Merged |
| UI-4 | Series page on real data | Item 7 | #204 | Ready |
| UI-5 | One `SeriesChart` component | Phase 3 | #209 | Draft |
| UI-6 | CSV download of the shown points | Item 7, export | #223 | Draft, waits for UI-5 |
| UI-7 | Dashboard on real latest values; drops the dashboard's other fake panels (recent releases, collaboration badges, system status) and the no-op category buttons | Item 8 | #212 | Ready, merges after #200 |
| UI-8 | Public annotations on the series chart, read-only | Phase 3, item 14 | | In progress, merges after #194 |
| UI-9 | Explore page cleanup: drops the random search time, made-up dates, fake relevance scores and dead controls; fixes `useSeriesSearch` against `searchSeries`; source filter from `dataSources`; results capped at 100 with a message. Hiding empty series is DATA-11 | Item 9 | #201 | Draft |
| UI-10 | Release e2e specs for the series pages | Item 11 | | After UI-4 to UI-8 |
| UI-11 | `seriesData` returns every point; real log difference | Item 7 | | In progress, merges after #184 |
| UI-12 | Series page pages through all data | Item 7 | | After UI-11 |

UI-11 and UI-12 were added after UI-4 found that `seriesData` returns at most 10,000
rows with `totalCount` equal to the rows returned, so long daily series were cut short
without notice, and that log difference was computed as `ratio - 1`.
