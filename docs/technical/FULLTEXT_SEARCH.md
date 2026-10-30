# Series Search

How the backend finds economic series by text: `searchSeries` and `searchSuggestions` in the
GraphQL API, both served by
[`SearchService`](../../backend/crates/econ-graph-services/src/services/search_service.rs).
Series metadata lives in Postgres, so search runs there against indexes on `economic_series`.

## What it matches

- **Words anywhere in the title or description**, stemmed with the `english` configuration, so
  "prices" finds "price" and "unemployed" finds "unemployment".
- **External ids** such as `UNRATE`, tokenized with the `simple` configuration (no stemming).
- **Typos in the title**, by trigram similarity: `unemploymnt` finds "Unemployment Rate".
- **Web-search syntax** through `websearch_to_tsquery`: `"quoted phrases"`, `or`, and `-excluded`
  words.

Both search and suggestions require `end_date IS NOT NULL`. This is a metadata eligibility
check; it does not verify that observation rows exist.

GraphQL `searchSeries` returns active series and accepts `source` and `frequency` filters. It
pages with `first` and `after`: `first` sets the page size, and `after` is the number of results
already read, encoded as a string. `totalCount` counts all matching series, including those
outside the current page. See [search pagination](../api/GRAPHQL_API.md#search-with-pagination)
and the [GraphQL resolvers](../../backend/crates/econ-graph-graphql/src/graphql/query.rs).

The service uses
[`SearchParams`](../../backend/crates/econ-graph-core/src/models/search.rs), with `limit` and
`offset` for paging. Internal callers can also include inactive series or change the similarity
threshold; GraphQL fixes those settings to active series only and 0.3, respectively.

## Schema

The [v4.0.0 data baseline](../../backend/migrations/data/2026-10-21-000100_v4_0_baseline/up.sql)
defines the search column and indexes:

```sql
CREATE EXTENSION IF NOT EXISTS pg_trgm;

ALTER TABLE economic_series
    ADD COLUMN search_vector TSVECTOR NOT NULL GENERATED ALWAYS AS (
        setweight(to_tsvector('simple', coalesce(external_id, '')), 'A') ||
        setweight(to_tsvector('english', coalesce(title, '')), 'A') ||
        setweight(to_tsvector('english', coalesce(description, '')), 'B')
    ) STORED;

CREATE INDEX idx_economic_series_search_vector ON economic_series USING GIN (search_vector);
CREATE INDEX idx_economic_series_title_trgm ON economic_series USING GIN (title gin_trgm_ops);
```

- `search_vector` is a stored generated column, so Postgres keeps it current on every insert and
  update. There is no trigger or backfill job.
- Title and external id carry weight A and description weight B, giving title and ID terms
  greater weight in full-text relevance. Final ordering also uses exact-ID priority and title
  similarity.
- The trigram index supports the `%` similarity predicate for typo tolerance and `ILIKE` on
  titles.

## Queries

`search_series_page` returns a page and the total match count; `search_series` returns only
the page. The SQL below abbreviates the filters and selected columns. `$1` is the original
query and `$7` is the normalized full-text input: whitespace-separated tokens made entirely of
`-` are removed before parsing. Thus `Number of establishments - United States` does not exclude
its own area, while an attached exclusion such as `-rate` keeps its meaning. Similarity and the
comparison with the whole external ID still use the original query. The expanded input is
NULL when no abbreviation is expanded, so `coalesce` repeats an existing disjunct.

```sql
WITH q AS (SELECT tsq, (''::tsvector @@ tsq) AS vacuous FROM (
    SELECT websearch_to_tsquery('english', $7) || websearch_to_tsquery('simple', $7)
        || coalesce(websearch_to_tsquery('english', <expanded full-text input>),
                    websearch_to_tsquery('simple', $7)) AS tsq
) parsed)
SELECT ...,
       (ts_rank_cd(es.search_vector, q.tsq, 32) + similarity(es.title, $1))::float4 AS rank,
       similarity(es.title, $1)::float4 AS similarity_score,
       count(*) OVER () AS total_count
FROM economic_series es, q
WHERE NOT q.vacuous
  AND (es.search_vector @@ q.tsq
       OR (strpos(q.tsq::text, '!') = 0 AND es.title % $1))
  AND es.end_date IS NOT NULL
  AND <source, frequency and active filters>
ORDER BY lower(es.external_id) IN (lower(btrim($1)), lower(<one-word term>)) DESC,
         rank DESC, es.title ASC, es.id ASC
LIMIT $5 OFFSET $6
```

- Both match predicates have supporting GIN indexes. Postgres chooses the execution plan for
  the query and data.
- The query is parsed with both configurations: `english` for stemmed title and description words,
  `simple` for external ids, which are indexed unstemmed.
- The trigram fallback is off when the parsed query contains a NOT (`-rate`, `(-rate)`,
  `"phrase"-rate`), so a near-miss title can't bring back what the exclusion removed. The check
  looks for `!` in the parsed query's text. Apart from the NOT operator, `!` survives only inside a
  quoted URL-like phrase (`"x.com/a!b"`), where it just turns typo matching off.
- Abbreviations listed in the
  [abbreviation CSV](../../backend/crates/econ-graph-core/data/search_abbreviations.csv) are also searched
  spelled out as quoted phrases, so `gdp` finds a title that only says "gross domestic product".
  Quoted, excluded (`-gdp`) and punctuated words in a longer query are left as typed.
- A series whose external id is the whole query ranks first (FRED's `GDP` for `gdp`).
- For a one-word query, the service strips surrounding quotes and brackets and trailing sentence
  punctuation before abbreviation expansion and exact-ID prioritization. Quotes may be ASCII
  or typographic, single or double; brackets may be `()[]{}`. The wrappers need not be paired.
  Thus `"gdp"`, `“gdp”`, `'gdp'`, `(gdp)`, `GDP.` and `gdp*` expand and receive ID priority as
  `gdp` does. Trailing punctuation is `. , ; : ! ? *`. Other symbols remain part of the term
  (`C++` stays `C++`), and a leading `-` is kept. If stripping would leave fewer than two
  characters (`A.`, `...`), the query is used as typed. The original query still supplies
  trigram similarity and the whole-query exact-ID comparison.
- `es.id` breaks remaining ties, giving an unchanged result set a deterministic order. Changes
  to matching data between page requests can still shift offsets.
- A query with no positive requirement (`-rate`, `-a -b`, `a OR -b`) is satisfied by an empty
  document, so it's rejected outright (`vacuous`) instead of matching every series that happens to
  lack the excluded word.
- `rank` combines full-text relevance (normalization 32 scales it into 0..1) with title
  similarity, so exact word matches and near-miss spellings share one ordering.
- `SearchParams.similarity_threshold` (default 0.3) is applied by setting
  `pg_trgm.similarity_threshold` for the query's transaction only.

`get_suggestions` returns up to 20 distinct active titles with non-null `end_date`. Titles starting
with the typed text come first, as completions, followed by the closest titles by trigram similarity,
as spelling corrections. Similarity is reported as the suggestion's confidence. `%`, `_` and `\`
in the typed text are escaped, so they match literally in the prefix `ILIKE`.

## Tests

The [search service tests](../../backend/crates/econ-graph-services/src/services/search_service.rs)
cover word, description, external-ID, typo, excluded-word and vacuous-query (all-NOT) matches,
standalone dashes, missing `end_date`, and suggestion completions, corrections and literal `%`/`_`.
They apply migrations and load fixtures in the database supplied by `DATABASE_URL`, removing
fixture rows when they finish. Use a disposable PostgreSQL 18-or-newer database; see
[database testing](../testing/DATABASE_TESTING.md). The tests skip when `DATABASE_URL` is unset.
With it set, migration or connection setup failures fail the tests.

## Limitations

Abbreviation expansion supports listed terms such as GDP; search does not perform general
synonym matching or semantic (vector) matching. The
[synonym list](../../backend/migrations/data/economic_synonyms.syn) is not used by any text search
configuration. Search does not use `unaccent` or `fuzzystrmatch`.

The search vector covers external IDs, titles and descriptions, excluding units. Trigram
matching covers titles only. The `english` and `simple` parses are OR'ed as whole queries against
the same vector. A mixed query can therefore fail to match by full text when an ID requires the
`simple` parse and another word requires English stemming: neither whole-query parse supplies
both required lexemes. Mixing IDs and words can work when one parse matches all required terms.

`SearchParams.sort_by` does not change the ordering: results always use the exact-ID priority,
relevance, title and ID ordering shown above.

The paged GraphQL `series` listing uses
[`series_service::list_series`](../../backend/crates/econ-graph-services/src/services/series_service.rs).
Its optional `query` filters title and description with `ILIKE`, independently of full-text search.
