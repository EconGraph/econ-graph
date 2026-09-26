# Series Search

How the backend finds economic series by text: `searchSeries` and `searchSuggestions` in the
GraphQL API, both served by `SearchService`
(`backend/crates/econ-graph-services/src/services/search_service.rs`). Series metadata lives in
Postgres, so search runs there against indexes on `economic_series`.

## What it matches

- **Words anywhere in the title or description**, stemmed with the `english` configuration, so
  "prices" finds "price" and "unemployed" finds "unemployment".
- **External ids** such as `UNRATE`, tokenized with the `simple` configuration (no stemming).
- **Typos in the title**, by trigram similarity: `unemploymnt` finds "Unemployment Rate".
- **Web-search syntax** through `websearch_to_tsquery`: `"quoted phrases"`, `or`, and `-excluded`
  words.

Results can be filtered by source, frequency and active status, and are paged with `limit` and
`offset`.

## Schema

Migration `backend/migrations/2026-09-26-000004_series_search`:

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
- Title and external id carry weight A and description weight B, so a title hit outranks a
  description hit.
- The trigram index serves `similarity()` / `%` for typo tolerance, and `ILIKE` on titles.

## Queries

`search_series`:

```sql
WITH q AS (SELECT websearch_to_tsquery('english', $1) AS tsq)
SELECT ...,
       (ts_rank_cd(es.search_vector, q.tsq, 32) + similarity(es.title, $1))::float4 AS rank,
       similarity(es.title, $1)::float4 AS similarity_score
FROM economic_series es, q
WHERE (es.search_vector @@ q.tsq OR es.title % $1)
  AND <source, frequency and active filters>
ORDER BY rank DESC, es.title ASC
LIMIT $5 OFFSET $6
```

- The `WHERE` runs as a bitmap OR of the two GIN indexes.
- `rank` combines full-text relevance (normalization 32 scales it into 0..1) with title
  similarity, so exact word matches and near-miss spellings share one ordering.
- `SearchParams.similarity_threshold` (default 0.3) is applied by setting
  `pg_trgm.similarity_threshold` for the query's transaction only.

`get_suggestions` returns up to 20 distinct active titles: titles starting with the typed text
first, as completions, then the closest titles by trigram similarity, as spelling corrections. The
similarity is reported as the suggestion's confidence.

## Tests

`search_service.rs` has DB-backed tests covering word, description, external-id and typo matches,
and completions and corrections. They run when `DATABASE_URL` points at a Postgres 18 database and
are skipped otherwise.

## Not built

An earlier version of this document described a larger design. These parts of it were never
implemented:

- **Synonyms** ("GDP" finding "Gross Domestic Product", "jobless" finding "unemployment"). A
  synonym list exists at `backend/migrations/economic_synonyms.syn`, but no text search
  configuration uses it. Wiring it in needs the file installed into the server's
  `tsearch_data` directory, which managed Postgres services generally don't allow; a synonym
  table expanded in the query is the more portable route.
- **`unaccent` and `fuzzystrmatch`**. Series titles are English, so neither has been needed.
- **Units in the search vector**, and trigram matching on descriptions and external ids.
- **`SearchParams.sort_by`**. Results are always ordered by relevance.
- **Semantic (vector) search**, listed there as a future enhancement: embeddings of series
  metadata (for example with pgvector) so a query can match by meaning ("jobless" finding
  "Unemployment Rate"). It would complement this search, not replace it, since exact ids and typos
  still need lexical matching.

The paged `series` listing (`series_service::list_series`) still filters its optional `query` with
`ILIKE` on title and description, not with `search_vector`.
