-- Prints a normalized description of everything the migrations create in the public schema:
-- one line per object, sorted, so two databases can be compared with diff.
--
-- Used by scripts/compare_migrations.sh to check that a squashed baseline migration builds the
-- same database as the chain it replaces. Objects owned by extensions (pg_trgm, pgcrypto) are
-- left out; the extension and its version are listed instead.
\pset format unaligned
\pset tuples_only on
\pset pager off

WITH ns AS (
    SELECT oid FROM pg_namespace WHERE nspname = 'public'
),
ext_objects AS (
    SELECT objid FROM pg_depend WHERE deptype = 'e'
),
rels AS (
    SELECT c.oid, c.relname, c.relkind
    FROM pg_class c
    WHERE c.relnamespace = (SELECT oid FROM ns)
        AND c.oid NOT IN (SELECT objid FROM ext_objects)
        -- Diesel's bookkeeping table; its rows differ by design.
        AND c.relname NOT LIKE '\_\_diesel\_schema\_migrations%'
),
lines AS (
    SELECT format('extension %s %s', extname, extversion) AS line
    FROM pg_extension
    WHERE extname <> 'plpgsql'

    UNION ALL
    SELECT format('enum %s (%s)', t.typname,
                  string_agg(quote_literal(e.enumlabel), ', ' ORDER BY e.enumsortorder))
    FROM pg_type t
    JOIN pg_enum e ON e.enumtypid = t.oid
    WHERE t.typnamespace = (SELECT oid FROM ns)
    GROUP BY t.typname

    UNION ALL
    SELECT format('relation %s kind=%s rls=%s', c.relname, c.relkind, c.relrowsecurity)
    FROM pg_class c
    WHERE c.oid IN (SELECT oid FROM rels) AND c.relkind IN ('r', 'v', 'm', 'S', 'p')

    -- Columns are numbered among the live (non-dropped) columns, so a column that was added
    -- and later dropped in the old chain does not shift the rest.
    UNION ALL
    SELECT format('column %s.%s #%s %s%s%s%s%s', r.relname, a.attname,
                  row_number() OVER (PARTITION BY r.oid ORDER BY a.attnum),
                  format_type(a.atttypid, a.atttypmod),
                  CASE WHEN a.attnotnull THEN ' not null' ELSE '' END,
                  CASE WHEN a.attgenerated = 's' THEN ' generated stored ' || pg_get_expr(d.adbin, d.adrelid)
                       WHEN d.adbin IS NOT NULL THEN ' default ' || pg_get_expr(d.adbin, d.adrelid)
                       ELSE '' END,
                  CASE WHEN a.attidentity::text <> '' THEN ' identity ' || a.attidentity::text ELSE '' END,
                  CASE WHEN a.attcollation <> 0 AND a.attcollation <> t.typcollation
                       THEN ' collate ' || a.attcollation::regcollation::text ELSE '' END)
    FROM rels r
    JOIN pg_attribute a ON a.attrelid = r.oid AND a.attnum > 0 AND NOT a.attisdropped
    JOIN pg_type t ON t.oid = a.atttypid
    LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum
    WHERE r.relkind IN ('r', 'v', 'm', 'p')

    UNION ALL
    SELECT format('constraint %s.%s %s%s', r.relname, con.conname, pg_get_constraintdef(con.oid),
                  CASE WHEN con.condeferrable THEN ' deferrable' ||
                       CASE WHEN con.condeferred THEN ' initially deferred' ELSE '' END
                       ELSE '' END)
    FROM pg_constraint con
    JOIN rels r ON r.oid = con.conrelid
    -- Not-null constraints are already on the column lines.
    WHERE con.contype <> 'n'

    UNION ALL
    SELECT format('index %s', pg_get_indexdef(i.indexrelid))
    FROM pg_index i
    JOIN rels r ON r.oid = i.indrelid

    UNION ALL
    SELECT format('trigger %s', pg_get_triggerdef(tg.oid))
    FROM pg_trigger tg
    JOIN rels r ON r.oid = tg.tgrelid
    WHERE NOT tg.tgisinternal

    UNION ALL
    SELECT format('view %s %s', r.relname, regexp_replace(pg_get_viewdef(r.oid), '\s+', ' ', 'g'))
    FROM rels r
    WHERE r.relkind IN ('v', 'm')

    UNION ALL
    SELECT format('sequence %s %s start=%s inc=%s min=%s max=%s cycle=%s', r.relname,
                  format_type(s.seqtypid, NULL), s.seqstart, s.seqincrement, s.seqmin, s.seqmax,
                  s.seqcycle)
    FROM pg_sequence s
    JOIN rels r ON r.oid = s.seqrelid

    UNION ALL
    SELECT format('function %s', regexp_replace(pg_get_functiondef(p.oid), '\s+', ' ', 'g'))
    FROM pg_proc p
    WHERE p.pronamespace = (SELECT oid FROM ns)
        AND p.oid NOT IN (SELECT objid FROM ext_objects)

    UNION ALL
    SELECT format('comment %s %s', r.relname, quote_literal(obj_description(r.oid, 'pg_class')))
    FROM rels r
    WHERE obj_description(r.oid, 'pg_class') IS NOT NULL

    UNION ALL
    SELECT format('comment %s.%s %s', r.relname, a.attname, quote_literal(col_description(r.oid, a.attnum)))
    FROM rels r
    JOIN pg_attribute a ON a.attrelid = r.oid AND a.attnum > 0 AND NOT a.attisdropped
    WHERE col_description(r.oid, a.attnum) IS NOT NULL

    UNION ALL
    SELECT format('comment function %s %s', p.oid::regprocedure, quote_literal(obj_description(p.oid, 'pg_proc')))
    FROM pg_proc p
    WHERE p.pronamespace = (SELECT oid FROM ns)
        AND p.oid NOT IN (SELECT objid FROM ext_objects)
        AND obj_description(p.oid, 'pg_proc') IS NOT NULL

    UNION ALL
    SELECT format('comment type %s %s', t.typname, quote_literal(obj_description(t.oid, 'pg_type')))
    FROM pg_type t
    WHERE t.typnamespace = (SELECT oid FROM ns)
        AND obj_description(t.oid, 'pg_type') IS NOT NULL

    UNION ALL
    SELECT format('acl %s %s', r.relname, c.relacl::text)
    FROM rels r
    JOIN pg_class c ON c.oid = r.oid
    WHERE c.relacl IS NOT NULL
)
SELECT line FROM lines ORDER BY line;
