DROP TABLE series_fetch_validators;
ALTER TABLE reference_file_cache
    DROP COLUMN last_modified,
    DROP COLUMN content_sha256,
    DROP COLUMN version;
