-- Chunked catalog discovery: runs, their chunks, and two crawl_queue job kinds.
--
-- An adapter that opts into chunked discovery lists its work units in its discover_catalog
-- job, which records one discovery_runs row and one discovery_run_chunks row per chunk of units
-- and enqueues a discover_chunk job per chunk. Each chunk job stores its result on its chunk
-- row. When every chunk has succeeded, exactly one finalize_discovery job commits the run's
-- results the way a single-job discovery does (backend/crates/econ-graph-crawler/README.md,
-- "Chunked catalog discovery").
--
-- discovery_runs.state:
--   enumerating  the discover_catalog job is listing units
--   running      chunk jobs are queued or in progress
--   finalizing   every chunk succeeded; the finalize_discovery job is queued or in progress
--   complete     the run's results were committed (or the catalog was unchanged)
--   failed       enumeration, a chunk or finalize failed; nothing was committed
--   superseded   a newer run of the same source started first; nothing was committed
-- At most one run per source is not final (enumerating, running or finalizing).
--
-- New tables, a wider crawl_queue kind constraint and a partial crawl_queue index: no existing
-- row changes.
--
-- Locking: replacing the kind constraint takes ACCESS EXCLUSIVE on crawl_queue until the
-- migration commits. lock_timeout and statement_timeout bound the wait at about 10 seconds, so
-- a long transaction on crawl_queue fails the migration with nothing changed rather than
-- stalling the worker; statement_timeout is set to 0 (no limit) once the lock is held.

SET LOCAL lock_timeout = '10s';
SET LOCAL statement_timeout = '10s';

LOCK TABLE crawl_queue IN ACCESS EXCLUSIVE MODE;

SET LOCAL statement_timeout = 0;

ALTER TABLE crawl_queue DROP CONSTRAINT check_crawl_queue_kind;

ALTER TABLE crawl_queue ADD CONSTRAINT check_crawl_queue_kind
    CHECK (kind IN ('fetch_series', 'discover_catalog', 'fetch_filing', 'refetch_series',
                    'discover_chunk', 'finalize_discovery'));

-- A run's chunk and finalize jobs, found by run id (the series_id up to the first '/') when the
-- run is superseded or failed, without scanning the source's other queue rows. Partial, so it
-- holds only those jobs; building it reads crawl_queue once, under the lock above.
CREATE INDEX idx_crawl_queue_discovery_run
    ON crawl_queue (split_part(series_id, '/', 1), status)
    WHERE kind IN ('discover_chunk', 'finalize_discovery');

CREATE TABLE discovery_runs (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    -- crawl_queue.source of the run's jobs (e.g. FRED).
    source VARCHAR(50) NOT NULL,
    state VARCHAR(20) NOT NULL DEFAULT 'enumerating',
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Set when the run reaches complete, failed or superseded.
    finished_at TIMESTAMPTZ,
    -- Set when enumeration's result committed (the discover_catalog job completed). The
    -- scheduler counts a failed run as a failed discovery only if it got this far; a failed
    -- enumeration fails its discover_catalog job instead.
    enumerated_at TIMESTAMPTZ,
    chunks_total INTEGER NOT NULL DEFAULT 0,
    chunks_done INTEGER NOT NULL DEFAULT 0,
    chunks_failed INTEGER NOT NULL DEFAULT 0,
    -- The adapter's fingerprint of its catalog selection when the run started, if it has one.
    -- Finalize fails the run if the adapter's fingerprint has changed since.
    selection_fingerprint TEXT,
    -- crawl_queue.id of the discover_catalog job that started the run (no foreign key: finished
    -- queue rows are purged). The scheduler takes that job's outcome from the run instead of
    -- from the job, which completes as soon as enumeration does.
    discovery_job_id UUID,
    -- Queue priority of the discover_catalog job that started the run; its chunk and finalize
    -- jobs use the same priority.
    priority INTEGER NOT NULL DEFAULT 5,
    -- The catalog validator enumeration returned, stored with the discovered series only when
    -- the run finalizes: {"url", "etag", "last_modified", "content_sha256", "version"}.
    validator JSONB,
    -- Curated entries enumeration found missing upstream (a JSON array of strings).
    curated_missing JSONB NOT NULL DEFAULT '[]'::jsonb,
    -- Enumeration found the catalog unchanged: the run completed without chunks.
    catalog_unchanged BOOLEAN NOT NULL DEFAULT FALSE,
    error_message TEXT,

    CONSTRAINT check_discovery_runs_state
        CHECK (state IN ('enumerating', 'running', 'finalizing', 'complete', 'failed', 'superseded')),
    CONSTRAINT check_discovery_runs_chunk_counts
        CHECK (chunks_total >= 0 AND chunks_done >= 0 AND chunks_failed >= 0
               AND chunks_done + chunks_failed <= chunks_total),
    CONSTRAINT check_discovery_runs_finished
        CHECK ((state IN ('complete', 'failed', 'superseded')) = (finished_at IS NOT NULL))
);

-- Only the newest run of a source may finalize: starting a run supersedes the older one first.
CREATE UNIQUE INDEX uq_discovery_runs_one_active_per_source
    ON discovery_runs (source)
    WHERE state IN ('enumerating', 'running', 'finalizing');

CREATE INDEX idx_discovery_runs_source_started
    ON discovery_runs (source, started_at DESC);

CREATE INDEX idx_discovery_runs_discovery_job
    ON discovery_runs (discovery_job_id);

COMMENT ON TABLE discovery_runs IS
    'One chunked catalog discovery of a source: enumerate, chunk jobs, then one finalize';

CREATE TABLE discovery_run_chunks (
    run_id UUID NOT NULL REFERENCES discovery_runs(id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL,
    state VARCHAR(20) NOT NULL DEFAULT 'pending',
    -- The chunk's work units, as enumeration listed them (a JSON array).
    units JSONB NOT NULL,
    -- The chunk's result once done: {"found": [...], "missing": [...]}.
    result JSONB,
    error_message TEXT,
    finished_at TIMESTAMPTZ,

    PRIMARY KEY (run_id, chunk_index),
    CONSTRAINT check_discovery_run_chunks_state
        CHECK (state IN ('pending', 'done', 'failed')),
    CONSTRAINT check_discovery_run_chunks_result
        CHECK ((state = 'done') = (result IS NOT NULL))
);

COMMENT ON TABLE discovery_run_chunks IS
    'Work units and stored results of one chunk of a discovery run; deleted when the run completes';
