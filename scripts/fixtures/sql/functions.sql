-- Helpers for scripts/fixtures/build-sanitized-fixture.sh, created in the
-- schema fixture_sanitize of each scratch database before the sanitize steps
-- run, and dropped before the database is dumped. Nothing here is ever run
-- against a live database.
--
-- The pseudonym key (fixture.salt) is set by the caller for the session. With
-- it, the same real address always gives the same pseudonym, in every database
-- and on every run that uses the same key, and an address can't be confirmed
-- by hashing a guess without the key.

CREATE SCHEMA fixture_sanitize;

-- 16 hex digits of sha256(key, newline, the value lower-cased and trimmed).
CREATE FUNCTION fixture_sanitize.h16(v text) RETURNS text
LANGUAGE sql STABLE STRICT AS $$
    SELECT left(encode(sha256(convert_to(
        current_setting('fixture.salt') || E'\n' || lower(btrim(v)), 'UTF8')), 'hex'), 16)
$$;

-- The pseudonym of an e-mail address (a %40-encoded one is read as '@').
CREATE FUNCTION fixture_sanitize.pseudo_email(e text) RETURNS text
LANGUAGE sql STABLE STRICT AS $$
    SELECT 'user-' || fixture_sanitize.h16(replace(lower(e), '%40', '@')) || '@example.invalid'
$$;

-- The display name for the person with this e-mail address (or other key).
CREATE FUNCTION fixture_sanitize.pseudo_name(k text) RETURNS text
LANGUAGE sql STABLE STRICT AS $$
    SELECT 'User ' || fixture_sanitize.h16(k)
$$;

-- An IPv4 address in 10.0.0.0/8 standing in for this one (or for any string
-- stored as one), the same for the same input.
CREATE FUNCTION fixture_sanitize.pseudo_ip(ip text) RETURNS text
LANGUAGE sql STABLE STRICT AS $$
    SELECT '10.' || get_byte(h, 0) || '.' || get_byte(h, 1) || '.' || get_byte(h, 2)
    FROM (SELECT sha256(convert_to(current_setting('fixture.salt') || E'\nip:' || btrim(ip), 'UTF8')) AS h) s
$$;

-- What counts as an e-mail address: plain, and %40-encoded inside URLs.
CREATE FUNCTION fixture_sanitize.email_re() RETURNS text
LANGUAGE sql IMMUTABLE AS $$
    SELECT '[A-Za-z0-9._%+-]+(?:@|%40)[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}'
$$;

-- Every address in v that isn't already @example.invalid, replaced by its
-- pseudonym. Longest first, so that a@x.com never rewrites part of ba@x.com
-- (a pseudonym can't contain a real address: its domain is example.invalid).
CREATE FUNCTION fixture_sanitize.scrub_emails(v text) RETURNS text
LANGUAGE plpgsql STABLE STRICT AS $$
DECLARE
    m text;
    r text := v;
BEGIN
    FOR m IN
        SELECT y.e FROM (
            SELECT DISTINCT x[1] AS e FROM regexp_matches(v, fixture_sanitize.email_re(), 'g') AS x
        ) y
        ORDER BY length(y.e) DESC, y.e
    LOOP
        IF replace(lower(m), '%40', '@') !~ '@example\.invalid$' THEN
            -- An address %40-encoded in a URL stays encoded.
            r := replace(r, m, CASE WHEN strpos(m, '@') = 0
                                    THEN replace(fixture_sanitize.pseudo_email(m), '@', '%40')
                                    ELSE fixture_sanitize.pseudo_email(m) END);
        END IF;
    END LOOP;
    RETURN r;
END
$$;

-- True if v holds an address outside example.invalid.
CREATE FUNCTION fixture_sanitize.has_real_email(v text) RETURNS boolean
LANGUAGE sql STABLE STRICT AS $$
    SELECT EXISTS (
        SELECT 1 FROM regexp_matches(v, fixture_sanitize.email_re(), 'g') AS x
        WHERE replace(lower(x[1]), '%40', '@') !~ '@example\.invalid$'
    )
$$;

-- JSON Web Tokens: a base64url header and payload that both start with '{"'.
CREATE FUNCTION fixture_sanitize.jwt_re() RETURNS text
LANGUAGE sql IMMUTABLE AS $$
    SELECT 'eyJ[A-Za-z0-9_-]{5,}\.eyJ[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]*'
$$;

CREATE FUNCTION fixture_sanitize.scrub_jwts(v text) RETURNS text
LANGUAGE sql IMMUTABLE STRICT AS $$
    SELECT regexp_replace(v, fixture_sanitize.jwt_re(), 'redacted.jwt.fixture', 'g')
$$;

-- Query parameters that carry credentials (?api_key=, &token=, BEA's UserID=,
-- Census's key=, ...): the value is replaced by REDACTED.
CREATE FUNCTION fixture_sanitize.url_secret_re() RETURNS text
LANGUAGE sql IMMUTABLE AS $$
    SELECT '([?&;](?:api[_-]?key|apikey|key|access[_-]?token|refresh[_-]?token|id[_-]?token|token'
        || '|client[_-]?secret|secret|registrationkey|userid|password|passwd|pwd|sig|signature|auth)=)'
        || '[^&\s"''#<>]+'
$$;

CREATE FUNCTION fixture_sanitize.scrub_url_secrets(v text) RETURNS text
LANGUAGE sql IMMUTABLE STRICT AS $$
    SELECT regexp_replace(v, fixture_sanitize.url_secret_re(), '\1REDACTED', 'gi')
$$;

-- Secret-shaped values, as the verification scan sees them. Each returns true
-- if v holds at least one:
--   token:  a run of 32+ base64url characters with upper case, lower case and digits;
--   base64: a run of 44+ base64 characters with upper case, lower case, digits and a '+';
--   hex:    a run of 32+ hex digits with both letters and digits;
--   pem:    a PEM block header (-----BEGIN).
CREATE FUNCTION fixture_sanitize.has_token(v text) RETURNS boolean
LANGUAGE sql IMMUTABLE STRICT AS $$
    SELECT EXISTS (
        SELECT 1 FROM regexp_matches(v, '[A-Za-z0-9_-]{32,}', 'g') AS x
        WHERE x[1] ~ '[A-Z]' AND x[1] ~ '[a-z]' AND x[1] ~ '[0-9]'
    )
$$;

CREATE FUNCTION fixture_sanitize.has_base64(v text) RETURNS boolean
LANGUAGE sql IMMUTABLE STRICT AS $$
    SELECT EXISTS (
        SELECT 1 FROM regexp_matches(v, '[A-Za-z0-9+/]{44,}', 'g') AS x
        WHERE x[1] ~ '[A-Z]' AND x[1] ~ '[a-z]' AND x[1] ~ '[0-9]' AND strpos(x[1], '+') > 0
    )
$$;

CREATE FUNCTION fixture_sanitize.has_hex(v text) RETURNS boolean
LANGUAGE sql IMMUTABLE STRICT AS $$
    SELECT EXISTS (
        SELECT 1 FROM regexp_matches(v, '[0-9A-Fa-f]{32,}', 'g') AS x
        WHERE x[1] ~ '[A-Fa-f]' AND x[1] ~ '[0-9]'
    )
$$;

-- Columns the scan and the generic steps leave alone, with the reason.
--   kinds: 'all' (neither rewritten nor scanned) or 'secret' (scanned for
--   e-mail addresses only).
CREATE TABLE fixture_sanitize.exempt (
    table_re  text NOT NULL,
    column_re text NOT NULL,
    kinds     text NOT NULL CHECK (kinds IN ('all', 'secret')),
    reason    text NOT NULL
);
INSERT INTO fixture_sanitize.exempt VALUES
    -- Liquibase identifies Keycloak's applied changesets by id, author (an
    -- e-mail address of a Keycloak maintainer) and file; rewriting them would
    -- make Keycloak re-run its migrations. md5sum is the changeset checksum.
    ('^public\.databasechangelog(lock)?$', '.*', 'all',
     'Keycloak''s Liquibase bookkeeping: maintainer addresses and checksums'),
    -- Content hashes and HTTP validators of public files: hex by design.
    ('.*', '^(etag|content_sha256|file_hash|xbrl_file_hash)$', 'secret',
     'hash of public content or an HTTP validator');

-- Every column of an ordinary table outside the system schemas whose values
-- are text, JSON, or arrays of them (domains included), with the type to cast
-- a rewritten value back to. Generated columns are left out: they can't be
-- written, and are recomputed from the columns that are.
CREATE FUNCTION fixture_sanitize.text_columns(kind text)
RETURNS TABLE (tbl text, schema_name name, table_name name, column_name name, type_sql text)
LANGUAGE sql STABLE AS $$
    SELECT format('%I.%I', n.nspname, c.relname), n.nspname, c.relname, a.attname,
           format_type(a.atttypid, a.atttypmod)
    FROM pg_attribute a
    JOIN pg_class c ON c.oid = a.attrelid
    JOIN pg_namespace n ON n.oid = c.relnamespace
    JOIN pg_type t ON t.oid = a.atttypid
    LEFT JOIN pg_type b ON b.oid = CASE WHEN t.typtype = 'd' THEN t.typbasetype ELSE t.oid END
    LEFT JOIN pg_type e ON e.oid = b.typelem AND b.typcategory = 'A'
    WHERE c.relkind = 'r'
      AND n.nspname NOT IN ('pg_catalog', 'information_schema', 'fixture_sanitize')
      AND n.nspname NOT LIKE 'pg\_%'
      AND a.attnum > 0 AND NOT a.attisdropped AND a.attgenerated = ''
      AND (b.typcategory = 'S' OR b.typname IN ('json', 'jsonb')
           OR e.typcategory = 'S' OR e.typname IN ('json', 'jsonb'))
      AND NOT EXISTS (
          SELECT 1 FROM fixture_sanitize.exempt x
          WHERE (x.kinds = 'all' OR x.kinds = kind)
            AND n.nspname || '.' || c.relname ~ x.table_re
            AND a.attname ~ x.column_re)
    ORDER BY 1, 4
$$;

-- Rewrite every text column (see text_columns) with fn, only in the rows
-- where it matches the regular expression guard. Prints one line per column
-- it changed.
CREATE FUNCTION fixture_sanitize.rewrite_all(fn text, guard text, flags text DEFAULT '')
RETURNS void LANGUAGE plpgsql AS $$
DECLARE
    col record;
    n bigint;
    op text := CASE WHEN flags = 'i' THEN '~*' ELSE '~' END;
BEGIN
    FOR col IN SELECT * FROM fixture_sanitize.text_columns('email') LOOP
        EXECUTE format('UPDATE %s SET %I = (fixture_sanitize.%s(%I::text))::%s WHERE %I::text %s %L',
                       col.tbl, col.column_name, fn, col.column_name, col.type_sql,
                       col.column_name, op, guard);
        GET DIAGNOSTICS n = ROW_COUNT;
        IF n > 0 THEN
            RAISE NOTICE '%: %.% rows rewritten: %', fn, col.tbl, col.column_name, n;
        END IF;
    END LOOP;
END
$$;

-- The verification scan: one row per column and kind of finding, with the
-- number of rows. The values themselves are never returned.
CREATE FUNCTION fixture_sanitize.scan()
RETURNS TABLE (finding text, tbl text, column_name text, n bigint)
LANGUAGE plpgsql AS $$
DECLARE
    col record;
    r record;
    secret boolean;
BEGIN
    FOR col IN SELECT * FROM fixture_sanitize.text_columns('email') LOOP
        secret := EXISTS (SELECT 1 FROM fixture_sanitize.text_columns('secret') s
                          WHERE s.tbl = col.tbl AND s.column_name = col.column_name);
        EXECUTE format($q$
            SELECT count(*) FILTER (WHERE v ~ '@|%%40' AND fixture_sanitize.has_real_email(v)) AS email,
                   count(*) FILTER (WHERE %2$s AND v ~ fixture_sanitize.jwt_re()) AS jwt,
                   count(*) FILTER (WHERE %2$s AND strpos(v, '-----BEGIN') > 0) AS pem,
                   count(*) FILTER (WHERE %2$s AND fixture_sanitize.has_token(v)) AS token,
                   count(*) FILTER (WHERE %2$s AND fixture_sanitize.has_base64(v)) AS base64,
                   count(*) FILTER (WHERE %2$s AND fixture_sanitize.has_hex(v)) AS hex
            FROM (SELECT %1$I::text AS v FROM %3$s WHERE %1$I IS NOT NULL) s
        $q$, col.column_name, CASE WHEN secret THEN 'true' ELSE 'false' END, col.tbl) INTO r;
        IF r.email > 0 THEN finding := 'email'; tbl := col.tbl; column_name := col.column_name; n := r.email; RETURN NEXT; END IF;
        IF r.jwt > 0 THEN finding := 'jwt'; tbl := col.tbl; column_name := col.column_name; n := r.jwt; RETURN NEXT; END IF;
        IF r.pem > 0 THEN finding := 'pem'; tbl := col.tbl; column_name := col.column_name; n := r.pem; RETURN NEXT; END IF;
        IF r.token > 0 THEN finding := 'token'; tbl := col.tbl; column_name := col.column_name; n := r.token; RETURN NEXT; END IF;
        IF r.base64 > 0 THEN finding := 'base64'; tbl := col.tbl; column_name := col.column_name; n := r.base64; RETURN NEXT; END IF;
        IF r.hex > 0 THEN finding := 'hex'; tbl := col.tbl; column_name := col.column_name; n := r.hex; RETURN NEXT; END IF;
    END LOOP;
END
$$;

-- Exact row count of every ordinary table outside the system schemas.
CREATE FUNCTION fixture_sanitize.row_counts()
RETURNS TABLE (tbl text, n bigint)
LANGUAGE plpgsql AS $$
DECLARE
    t record;
BEGIN
    FOR t IN
        SELECT format('%I.%I', n2.nspname, c.relname) AS name
        FROM pg_class c JOIN pg_namespace n2 ON n2.oid = c.relnamespace
        WHERE c.relkind = 'r'
          AND n2.nspname NOT IN ('pg_catalog', 'information_schema', 'fixture_sanitize')
          AND n2.nspname NOT LIKE 'pg\_%'
        ORDER BY 1
    LOOP
        tbl := t.name;
        EXECUTE format('SELECT count(*) FROM %s', t.name) INTO n;
        RETURN NEXT;
    END LOOP;
END
$$;

-- True if the table exists (schema-qualified name).
CREATE FUNCTION fixture_sanitize.has_table(name text) RETURNS boolean
LANGUAGE sql STABLE AS $$
    SELECT to_regclass(name) IS NOT NULL
$$;

-- Delete every row of each of these public tables that exists (the list
-- covers several Keycloak versions). Prints one line per table emptied.
CREATE FUNCTION fixture_sanitize.empty_tables(tables text[]) RETURNS void
LANGUAGE plpgsql AS $$
DECLARE
    t text;
    n bigint;
BEGIN
    FOREACH t IN ARRAY tables LOOP
        IF fixture_sanitize.has_table(format('public.%I', t)) THEN
            EXECUTE format('DELETE FROM public.%I', t);
            GET DIAGNOSTICS n = ROW_COUNT;
            IF n > 0 THEN
                RAISE NOTICE 'emptied public.%: % rows', t, n;
            END IF;
        END IF;
    END LOOP;
END
$$;

-- The rows of these public tables that exist, in total (for the checks).
CREATE FUNCTION fixture_sanitize.count_rows(tables text[]) RETURNS bigint
LANGUAGE plpgsql AS $$
DECLARE
    t text;
    n bigint;
    total bigint := 0;
BEGIN
    FOREACH t IN ARRAY tables LOOP
        IF fixture_sanitize.has_table(format('public.%I', t)) THEN
            EXECUTE format('SELECT count(*) FROM public.%I', t) INTO n;
            total := total + n;
        END IF;
    END LOOP;
    RETURN total;
END
$$;

-- Header names that suggest credentials.
CREATE FUNCTION fixture_sanitize.secret_header_re() RETURNS text
LANGUAGE sql IMMUTABLE AS $$
    SELECT '(auth|key|token|cookie|secret|session|signature|password)'
$$;

-- A JSON object of HTTP headers without the ones whose names suggest
-- credentials; anything else is returned as it is.
CREATE FUNCTION fixture_sanitize.strip_headers(h jsonb) RETURNS jsonb
LANGUAGE sql STABLE STRICT AS $$
    SELECT CASE WHEN jsonb_typeof(h) <> 'object' THEN h ELSE (
        SELECT coalesce(jsonb_object_agg(k, v), '{}'::jsonb)
        FROM jsonb_each(h) AS e(k, v)
        WHERE lower(k) !~ fixture_sanitize.secret_header_re()) END
$$;

-- True if a JSON object of headers has one whose name suggests credentials.
CREATE FUNCTION fixture_sanitize.has_secret_header(h jsonb) RETURNS boolean
LANGUAGE sql STABLE STRICT AS $$
    SELECT jsonb_typeof(h) = 'object' AND EXISTS (
        SELECT 1 FROM jsonb_object_keys(h) AS k WHERE lower(k) ~ fixture_sanitize.secret_header_re())
$$;
