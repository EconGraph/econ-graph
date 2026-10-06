#!/usr/bin/env bash
# Builds one database from the migrations at a git ref and one from the migrations in the working
# tree, then diffs their schemas (scripts/schema_snapshot.sql) and their seed rows.
#
# Used when squashing migrations into a baseline (docs/development/RELEASE_PROCESS.md,
# "Fold migrations into the release baseline"): the squash is right when this prints no diff.
#
# Usage: scripts/compare_migrations.sh <old-ref> [new-migrations-dir]
#   ADMIN_URL  a PostgreSQL 18 superuser connection to an existing database, used to create and
#              drop the two scratch databases (default postgres://postgres@127.0.0.1:5432/postgres)
#
# Each up.sql runs in its own transaction, in version order, as Diesel runs them. In seed rows,
# random ids (UUID v4/v7) are replaced by the table and name of the row they identify, and
# timestamps within the last day are masked; fixed ids such as the UUIDv5 series ids are compared
# as they are.
set -euo pipefail

old_ref=${1:?usage: compare_migrations.sh <old-ref> [new-migrations-dir]}
script_dir=$(cd "$(dirname "$0")" && pwd)
backend_dir=$(dirname "$script_dir")
new_dir=${2:-$backend_dir/migrations}
admin_url=${ADMIN_URL:-postgres://postgres@127.0.0.1:5432/postgres}

work=$(mktemp -d)
old_db="migcmp_old_$$"
new_db="migcmp_new_$$"
# Drops the scratch databases and the work directory, on any exit.
cleanup() {
    psql "$admin_url" -q -c "DROP DATABASE IF EXISTS $old_db" -c "DROP DATABASE IF EXISTS $new_db" >/dev/null 2>&1 || true
    rm -rf "$work"
}
trap cleanup EXIT

# Prints ADMIN_URL with its database name replaced by $1, keeping any query string.
db_url() {
    python3 -c 'import sys, urllib.parse as u; p = u.urlsplit(sys.argv[1]); print(u.urlunsplit(p._replace(path="/" + sys.argv[2])))' "$admin_url" "$1"
}

# Applies every up.sql in directory $1 to database URL $2, in the order Diesel would. The version
# is computed as Diesel does (migrations_internals::version_from_string): the directory name up to
# the first "_", without dashes.
apply_chain() {
    local dir=$1 url=$2 name
    for name in $(cd "$dir" && for d in */; do d=${d%/}; [ -f "$d/up.sql" ] && printf '%s %s\n' "$(echo "${d%%_*}" | tr -d -)" "$d"; done | sort | cut -d' ' -f2); do
        psql "$url" -q -X -1 -v ON_ERROR_STOP=1 -f "$dir/$name/up.sql" >"$work/apply.log" 2>&1 || {
            echo "Applying $name from $dir failed:" >&2
            cat "$work/apply.log" >&2
            exit 1
        }
    done
}

# Writes the schema of database URL $1 to $2.schema and its rows, masked, to $2.rows.
snapshot() {
    local url=$1 out=$2
    psql "$url" -X -q -v ON_ERROR_STOP=1 -f "$script_dir/schema_snapshot.sql" >"$out.schema"
    # Every row of every table, as JSON with keys sorted.
    psql "$url" -X -q -At -v ON_ERROR_STOP=1 -c "
        SELECT format('SELECT %L || '' '' || (to_jsonb(t))::text FROM %I t', c.relname, c.relname)
        FROM pg_class c
        WHERE c.relnamespace = 'public'::regnamespace AND c.relkind = 'r'
            AND c.relname <> '__diesel_schema_migrations'
        ORDER BY c.relname" >"$work/queries.sql"
    : >"$out.rows"
    while read -r q; do
        psql "$url" -X -q -At -v ON_ERROR_STOP=1 -c "$q" >>"$out.rows"
    done <"$work/queries.sql"
    python3 - "$out.rows" <<'EOF'
import json, re, sys
from datetime import datetime, timedelta, timezone

path = sys.argv[1]
uuid_re = re.compile(r'^[0-9a-f]{8}-[0-9a-f]{4}-([0-9a-f])[0-9a-f]{3}-[0-9a-f]{4}-[0-9a-f]{12}$')
now = datetime.now(timezone.utc)

rows = []
for line in open(path):
    table, _, row = line.rstrip('\n').partition(' ')
    rows.append((table, json.loads(row)))

# A random id that is some seed row's primary key is shown as that row's table and name, so
# foreign keys between seed rows (series_metadata.source_id -> data_sources) are compared too.
labels = {}
for table, row in rows:
    key = next((row[k] for k in ('name', 'iso_code', 'external_id') if isinstance(row.get(k), str)), '?')
    if isinstance(row.get('id'), str):
        labels[row['id']] = '<%s %s>' % (table, key)

def mask(v):
    if isinstance(v, dict):
        return {k: mask(x) for k, x in v.items()}
    if isinstance(v, list):
        return [mask(x) for x in v]
    if isinstance(v, str):
        m = uuid_re.match(v)
        if m and m.group(1) in '47':
            return labels.get(v, '<random uuid>')
        try:
            t = datetime.fromisoformat(v)
            if t.tzinfo and abs(now - t) < timedelta(days=1):
                return '<now>'
        except ValueError:
            pass
    return v

out = sorted(table + ' ' + json.dumps(mask(row), sort_keys=True) for table, row in rows)
unlabelled = sum(line.count('<random uuid>') for line in out)
if unlabelled:
    # A random id that is no seed row's key compares equal whatever it points at.
    print(f'warning: {path}: {unlabelled} random ids are not the key of any seed row', file=sys.stderr)
open(path, 'w').write('\n'.join(out) + '\n')
EOF
}

psql "$admin_url" -q -c "CREATE DATABASE $old_db" -c "CREATE DATABASE $new_db" >/dev/null

mkdir -p "$work/old"
git -C "$backend_dir" archive "$old_ref" migrations | tar -x -C "$work/old"
apply_chain "$work/old/migrations" "$(db_url "$old_db")"
apply_chain "$new_dir" "$(db_url "$new_db")"

snapshot "$(db_url "$old_db")" "$work/old"
snapshot "$(db_url "$new_db")" "$work/new"

status=0
diff -u --label "schema at $old_ref" --label "schema in $new_dir" "$work/old.schema" "$work/new.schema" || status=1
diff -u --label "rows at $old_ref" --label "rows in $new_dir" "$work/old.rows" "$work/new.rows" || status=1
if [ "$status" -eq 0 ]; then
    echo "Same schema and seed rows: $(wc -l <"$work/new.schema") schema lines, $(wc -l <"$work/new.rows") rows."
fi
exit "$status"
