#!/usr/bin/env bash
# Stub-only tests for measure-v41-first-deploy.sh. They never touch a cluster or a real
# database: psql, the crawler and a `kubectl port-forward` are stand-ins on PATH or local ports,
# and every database URL names a stub. They check:
#   - the target guard: no URL, a bad URL (without echoing it), in-cluster names, a comma host
#     list, a host that doesn't resolve, an unidentified local listener and a port-forward into
#     production's namespace are refused (exit 64 / 65) before psql runs; a local Postgres and
#     port-forwards into other clusters and namespaces are allowed; --i-know-this-is-production
#     runs and says so;
#   - one psql session runs every query in one READ ONLY, REPEATABLE READ transaction with the
#     statement and lock timeouts; a failed query is reported and the rest marked not run; the
#     password reaches psql only as PGPASSWORD, and no secret is printed;
#   - dual-stack listeners (127.0.0.1 and [::1] from one process) are judged by that process,
#     and a host label over 63 characters is refused without a traceback;
#   - the BEA rules come from bea.toml and the query applies SeriesKey::parse's rules; the FRED
#     check runs only with FRED_API_KEY, with --count-uncapped and without any database URL, and
#     a report without a count is a failed measurement; table values escape |; the script's
#     synthetic-row SQL matches data_point.rs;
#   - ECO-738: the relabel and twin counts run in the same session, the run-time and WAL
#     estimates are reported against the crawler-migrate Job's deadline (flagged when over
#     it), twins are flagged, a database where it is already applied says so, and one
#     without the World Bank source fails.
set -Eeuo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$ROOT/scripts/deploy/measure-v41-first-deploy.sh"
scratch="$(mktemp -d)"
pids=()
cleanup() {
    for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
    rm -rf "$scratch"
}
trap cleanup EXIT
trap 'echo "test-measure-v41-first-deploy: failed at line $LINENO" >&2' ERR
fail() { echo "FAIL: $*" >&2; exit 1; }

# A psql that records each script and its environment, and answers by the query.
mkdir "$scratch/bin"
cat >"$scratch/bin/psql" <<'STUB'
#!/usr/bin/env python3
# A psql that records each session and its environment, prints the \echo markers and answers
# each query by its text. STUB_FAIL_ON=TEXT fails the query containing TEXT, as psql does.
import json, os, sys
script = sys.stdin.read()
with open(os.environ['STUB_LOG'], 'a') as log:
    log.write(json.dumps({'argv': sys.argv[1:], 'script': script,
                          'env': {k: v for k, v in os.environ.items() if k.startswith('PG')}}) + '\n')
sep, fail_on = '\x1f', os.environ.get('STUB_FAIL_ON')
query = []
for line in script.splitlines():
    if line.startswith('\\echo '):
        print(line[len('\\echo '):], flush=True)
        continue
    query.append(line)
    if not line.rstrip().endswith(';'):
        continue
    sql, query = '\n'.join(query), []
    if sql.startswith(('BEGIN', 'SET ', 'COMMIT')):
        continue
    if fail_on and fail_on in sql:
        sys.stderr.write('ERROR:  canceling statement due to lock timeout\nLINE 1: ...\n        ^\n')
        sys.exit(3)
    if 'server_version' in sql:
        print('18.0 (stub)')
    elif 'FROM series_metadata sm' in sql:
        print('1')
    elif 'is_active' in sql:
        rules = sql.count('count(*) FILTER') - 2
        print(sep.join(['7', '2', '2'] + ['1'] * rules))
    elif 'ROW_NUMBER' in sql:
        print(sep.join(os.environ.get('STUB_ECO_738', '0,1,3600000,0').split(',')))
    elif 'other_original' in sql:
        print(sep.join(['Federal Reserve Economic Data (FRED)', '3']))
        print(sep.join(['A | piped source', '1']))
    else:
        sys.exit('stub psql: unexpected query: ' + sql[:80])
STUB
# A crawler that records its environment and prints what `check-selection --source FRED --json
# --count-uncapped` prints for a selection over the cap: the whole count and an over-cap problem
# on "selected series" (check_selection.rs, check_fred and bound). With $HOME/capped present it
# prints what it prints without --count-uncapped instead: no count, an over-cap problem on
# "selection". (A file, as the script passes the crawler only an allowlist of variables.)
cat >"$scratch/crawler" <<'STUB'
#!/usr/bin/env python3
import json, os, sys
with open(os.environ['HOME'] + '/crawler.log', 'w') as log:
    json.dump({'argv': sys.argv[1:], 'env': sorted(os.environ)}, log)
if os.path.exists(os.environ['HOME'] + '/capped'):
    fred = {'source': 'FRED', 'status': 'problems', 'rules': [], 'bounds': [],
            'problems': [{'kind': 'over_cap', 'entry': 'selection',
                          'detail': 'FRED catalog volume guard: selected series 501 exceeds 500'}],
            'notices': []}
else:
    fred = {'source': 'FRED', 'status': 'problems', 'selected': 812,
            'rules': [{'rule': 'release_id=53', 'matched': 801, 'max_matches': None}],
            'bounds': [{'what': 'catalog rows inspected', 'count': 1902, 'limit': 20000},
                       {'what': 'selected series', 'count': 812, 'limit': 500}],
            'problems': [{'kind': 'over_cap', 'entry': 'selected series',
                          'detail': '812 exceeds the limit of 500'}],
            'notices': [], 'refresh': {'requests': 2436, 'requests_per_minute': 120.0,
                                       'minutes': 20.3, 'daily_quota': None}}
print(json.dumps({'sources': [fred]}))
sys.exit(1)
STUB
chmod +x "$scratch/bin/psql" "$scratch/crawler"
mkdir "$scratch/home"
export PATH="$scratch/bin:$PATH" STUB_LOG="$scratch/psql.log" HOME="$scratch/home"
unset FRED_API_KEY DATABASE_URL DATA_DATABASE_URL PGPASSWORD PGHOST PGDATABASE 2>/dev/null || true

run() {
    : >"$STUB_LOG"
    # In a condition, so an expected non-zero exit doesn't fire the ERR trap.
    status=0
    "$SCRIPT" --crawler "$scratch/crawler" "$@" >"$scratch/out" 2>"$scratch/err" || status=$?
}
psql_ran() { [ -s "$STUB_LOG" ]; }

# Listener stand-ins on free local ports: python3 started under another program's name
# (kubectl, postgres, socat), with that program's arguments for the guard to read.
free_port() { python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])'; }
for name in kubectl postgres socat; do ln -s "$(command -v python3)" "$scratch/bin/$name"; done
serve() {  # serve NAME PORT ARGS...: start NAME listening on PORT, wait until it listens
    local name="$1" port="$2"; shift 2
    "$scratch/bin/$name" -c "import socket,time; s=socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1); s.bind(('127.0.0.1', $port)); s.listen(); time.sleep(120)" \
        "$@" &
    pids+=("$!")
    wait_listening "$name" "$port"
}
serve_dual() {  # serve_dual NAME PORT ARGS...: as serve, on 127.0.0.1 and [::1] from one process
    local name="$1" port="$2"; shift 2
    "$scratch/bin/$name" -c "import socket,time
a=socket.socket(); a.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1); a.bind(('127.0.0.1', $port)); a.listen()
b=socket.socket(socket.AF_INET6); b.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1); b.setsockopt(socket.IPPROTO_IPV6, socket.IPV6_V6ONLY, 1); b.bind(('::1', $port)); b.listen()
time.sleep(120)" "$@" &
    pids+=("$!")
    wait_listening "$name" "$port" 2
}
wait_listening() {  # wait_listening NAME PORT [SOCKETS]
    for _ in $(seq 1 50); do
        [ "$(ss -Hltn "sport = :$2" | grep -c .)" -ge "${3:-1}" ] && return 0
        sleep 0.1
    done
    fail "the $1 stand-in did not listen on $2"
}
forward() { local port="$1"; shift; serve kubectl "$port" port-forward "$@" "$port:5432"; }
postgres_on() { serve postgres "$1" -D /var/lib/postgresql/data; }

# --- The target guard ---
run
[ "$status" = 64 ] || fail "no --database-url: exit $status"
psql_ran && fail "psql ran without a target"

run --database-url 'postgres://u:Ab3/xYz9@127.0.0.1:5432/econ_graph'
[ "$status" = 64 ] || fail "a password with '/': exit $status"
grep -qE "Ab3|xYz9" "$scratch/err" && fail "a bad URL's password was printed: $(cat "$scratch/err")"

long_label="$(printf 'a%.0s' $(seq 1 64)).example"
run --database-url "postgres://postgres@${long_label}:5432/econ_graph"
[ "$status" = 65 ] || fail "a host label over 63 characters: exit $status $(cat "$scratch/err")"
grep -q "Traceback" "$scratch/err" && fail "a long host label raised: $(cat "$scratch/err")"

for host in data-postgres-service postgres-service.econ-graph.svc.cluster.local \
    postgresql-0.postgres-service; do
    run --database-url "postgres://postgres@${host}:5432/econ_graph"
    [ "$status" = 65 ] || fail "in-cluster host $host: exit $status"
    grep -qF "refusing" "$scratch/err" || fail "no refusal message for $host"
    psql_ran && fail "psql ran against $host"
done

port="$(free_port)"
postgres_on "$port"
run --database-url "postgres://postgres@127.0.0.2,127.0.0.1:${port}/econ_graph"
[ "$status" = 64 ] || fail "a comma host list: exit $status"
psql_ran && fail "psql ran with a comma host list"
run --database-url "postgres://postgres@no-such-host.invalid:${port}/econ_graph"
[ "$status" = 65 ] || fail "a host that doesn't resolve: exit $status"
run --database-url "postgres://postgres@127.0.0.1:${port}/econ_graph"
[ "$status" = 0 ] || fail "a local Postgres refused: exit $status $(cat "$scratch/err")"

port="$(free_port)"
serve socat "$port" "TCP-LISTEN:$port,fork" TCP:10.0.0.1:5432
run --database-url "postgres://postgres@127.0.0.1:${port}/econ_graph"
[ "$status" = 65 ] || fail "an unidentified listener (socat): exit $status"
grep -qF "socat" "$scratch/err" || fail "the refusal does not name the listener"

# An `ss` that sees a listener but not its owner (another user's or root's), and one that fails.
mkdir "$scratch/ss-hidden" "$scratch/ss-broken"
printf '#!/bin/sh\necho "LISTEN 0 4096 127.0.0.1:5999 0.0.0.0:*"\n' >"$scratch/ss-hidden/ss"
printf '#!/bin/sh\nexit 1\n' >"$scratch/ss-broken/ss"
chmod +x "$scratch/ss-hidden/ss" "$scratch/ss-broken/ss"
PATH="$scratch/ss-hidden:$PATH" run --database-url "postgres://postgres@127.0.0.1:5999/econ_graph"
[ "$status" = 65 ] || fail "a listener this user cannot see: exit $status"
PATH="$scratch/ss-broken:$PATH" run --database-url "postgres://postgres@127.0.0.1:5999/econ_graph"
[ "$status" = 65 ] || fail "ss failing: exit $status"

port="$(free_port)"
forward "$port" -n econ-graph svc/data-postgres-service --context kind-econ-graph
run --database-url "postgres://postgres@127.0.0.1:${port}/econ_graph"
[ "$status" = 65 ] || fail "port-forward into production: exit $status"
grep -qF "port-forward" "$scratch/err" || fail "the refusal does not name the port-forward"
psql_ran && fail "psql ran against production's port-forward"
run --database-url "postgres://postgres@127.0.0.1:${port}/econ_graph" --i-know-this-is-production
[ "$status" = 0 ] || fail "--i-know-this-is-production: exit $status $(cat "$scratch/err")"
grep -qF "(production guard overridden: port $port on 127.0.0.1 is a port-forward" "$scratch/out" ||
    fail "a forced run does not say why the guard objected: $(head -3 "$scratch/out")"

port="$(free_port)"
KUBECONFIG="$scratch/home/kubeconfig-econ-graph" forward "$port" -n econ-graph svc/data-postgres-service
run --database-url "postgres://postgres@localhost:${port}/econ_graph"
[ "$status" = 65 ] || fail "production's kind-env kubeconfig: exit $status"

port="$(free_port)"
forward "$port" svc/data-postgres-service
run --database-url "postgres://postgres@127.0.0.1:${port}/econ_graph"
[ "$status" = 65 ] || fail "a port-forward whose cluster and namespace are unknown: exit $status"

# The rehearsal in docs/deployment/APP_DATA_CONVERSION.md: a scratch cluster's kind-env.sh
# kubeconfig, without --context.
port="$(free_port)"
KUBECONFIG="$scratch/home/kubeconfig-eco803-rehearsal" forward "$port" -n econ-graph svc/data-postgres-service
run --database-url "postgres://postgres@127.0.0.1:${port}/econ_graph"
[ "$status" = 0 ] || fail "a scratch cluster's port-forward refused: exit $status $(cat "$scratch/err")"

# kubectl port-forward and Postgres listen on 127.0.0.1 and [::1] from one process.
if python3 -c 'import socket; s=socket.socket(socket.AF_INET6); s.bind(("::1", 0))' 2>/dev/null; then
    port="$(free_port)"
    serve_dual kubectl "$port" port-forward --context kind-eco803-rehearsal -n econ-graph \
        svc/data-postgres-service "$port:5432"
    run --database-url "postgres://postgres@localhost:${port}/econ_graph"
    [ "$status" = 0 ] || fail "a dual-stack rehearsal port-forward refused: exit $status $(cat "$scratch/err")"
    port="$(free_port)"
    serve_dual kubectl "$port" port-forward --context kind-econ-graph -n econ-graph \
        svc/data-postgres-service "$port:5432"
    run --database-url "postgres://postgres@localhost:${port}/econ_graph"
    [ "$status" = 65 ] || fail "a dual-stack production port-forward: exit $status"
    port="$(free_port)"
    serve_dual postgres "$port" -D /var/lib/postgresql/data
    run --database-url "postgres://postgres@127.0.0.1:${port}/econ_graph"
    [ "$status" = 0 ] || fail "a dual-stack local Postgres refused: exit $status $(cat "$scratch/err")"
else
    echo "note: no IPv6 loopback here; dual-stack cases skipped" >&2
fi

for args in "--context kind-eco803-rehearsal -n econ-graph" "-n econ-graph-br-feature --context kind-econ-graph"; do
    port="$(free_port)"
    # shellcheck disable=SC2086 # the arguments are words
    forward "$port" $args svc/data-postgres-service
    run --database-url "postgres://postgres@127.0.0.1:${port}/econ_graph"
    [ "$status" = 0 ] || fail "port-forward ($args) refused: exit $status $(cat "$scratch/err")"
done

# --- A measured run: read-only scripts, no secrets, rules from bea.toml, FRED skipped ---
port="$(free_port)"
postgres_on "$port"
run --database-url "postgresql://measure:pw-S3CRET-1@127.0.0.1:${port}/rehearsal" --statement-timeout 42
[ "$status" = 0 ] || fail "measured run: exit $status $(cat "$scratch/err")"
python3 - "$STUB_LOG" <<'PY'
import json, sys
calls = [json.loads(line) for line in open(sys.argv[1])]
assert len(calls) == 1, f'one psql session, not {len(calls)}'
for call in calls:
    script = call['script']
    assert script.startswith('BEGIN;\nSET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY;\n'), script[:120]
    assert "SET LOCAL statement_timeout = '42s';" in script, script[:200]
    assert "SET LOCAL lock_timeout = '5s';" in script, script[:250]
    assert script.rstrip().endswith('COMMIT;'), script[-40:]
    # BEGIN, SET TRANSACTION, two SET LOCAL, five queries, COMMIT: nothing else.
    assert script.count(';') == 10, script.count(';')
    assert script.count('\\echo @@measure-v41:') == 6, script
    env = call['env']
    assert env['PGPASSWORD'] == 'pw-S3CRET-1', env
    assert 'default_transaction_read_only=on' in env['PGOPTIONS'], env
    assert 'statement_timeout=42s' in env['PGOPTIONS'], env
    assert (env['PGHOST'], env['PGPORT'], env['PGDATABASE'], env['PGUSER']) == \
        ('127.0.0.1', env['PGPORT'], 'rehearsal', 'measure'), env
    assert not any('S3CRET' in a for a in call['argv']), call['argv']
PY
grep -q "S3CRET" "$scratch/out" "$scratch/err" && fail "the password was printed"
grep -qF "Inactive BEA series: **7**" "$scratch/out" || fail "BEA total missing"
# The BEA queries apply SeriesKey::parse, the crawler's source name and every rule from bea.toml.
python3 - "$STUB_LOG" "$ROOT" <<'PY'
import json, re, sys, tomllib
from pathlib import Path
calls = [json.loads(line)['script'] for line in open(sys.argv[1])]
rules = tomllib.loads((Path(sys.argv[2]) / 'backend/crates/econ-graph-crawler/data/catalog_selection/bea.toml').read_text())
bea = next(c for c in calls if 'is_active' in c)
meta = next(c for c in calls if 'FROM series_metadata sm' in c)
for sql in (bea, meta):
    assert "ds.name = 'Bureau of Economic Analysis (BEA)'" in sql, sql[:300]
    assert r"~ '^[A-Za-z0-9]+\.[A-Za-z0-9]+\.[^.]*$'" in sql, 'SeriesKey::parse shape missing'
    for rule in rules['rule']:
        assert f"LIKE '{rule['prefix']}%'" in sql, rule['prefix']
    for table in rules['exclude']:
        assert f"'{table}'" in sql, table
assert "~ '^[0-9]{2}000$'" in bea, 'state-level area check missing'
assert "IN ('A', 'Q')" in bea, 'NIPA frequencies missing'
PY
grep -qF "one synthetic legacy row, and no other row: **4**" "$scratch/out" || fail "ECO-807 count missing"
grep -qF "World Bank rows to relabel as revisions: **3600000**" "$scratch/out" || fail "ECO-738 count missing"
grep -qF "Revision twins (the migration refuses while any exist): **0**" "$scratch/out" || fail "ECO-738 twins missing"
grep -qF "Estimated run time: about 90 s at 25 s per million rows; the crawler-migrate Job allows 1800 s" "$scratch/out" ||
    fail "ECO-738 run-time estimate missing"
grep -qF "Estimated WAL: about 3.1 GB" "$scratch/out" || fail "ECO-738 WAL estimate missing"
grep -q "ROW_NUMBER" <<<"$(python3 -c 'import json,sys; print(json.loads(open(sys.argv[1]).readline())["script"])' "$STUB_LOG")" ||
    fail "ECO-738 query not in the one read-only session"
grep -qF '| A \| piped source | 1 |' "$scratch/out" || fail "a | in a table value is not escaped"
grep -qF "skipped: needs FRED_API_KEY" "$scratch/out" || fail "FRED not reported as skipped"
[ ! -e "$scratch/home/crawler.log" ] || fail "the crawler ran without FRED_API_KEY"
rules="$(grep -c '^\[\[rule\]\]' "$ROOT/backend/crates/econ-graph-crawler/data/catalog_selection/bea.toml")"
rows="$(grep -cE '^\| (NIPA|Regional) \|' "$scratch/out")"
[ "$rules" = "$rows" ] || fail "bea.toml has $rules rules, the report $rows rows"

# --- A query that fails: what came before is reported, the rest is a failed measurement ---
STUB_FAIL_ON=other_original run --database-url "postgresql://measure@127.0.0.1:${port}/rehearsal"
[ "$status" = 1 ] || fail "a failed query: exit $status"
grep -qF "Inactive BEA series: **7**" "$scratch/out" || fail "the BEA section before the failure is missing"
grep -qF "FAILED: ERROR:  canceling statement due to lock timeout" "$scratch/out" ||
    fail "the failed query's ERROR is not reported: $(cat "$scratch/out")"
STUB_FAIL_ON='FROM series_metadata sm' run --database-url "postgresql://measure@127.0.0.1:${port}/rehearsal"
[ "$status" = 1 ] || fail "a failed BEA query: exit $status"
grep -qF "FAILED: not run: bea_metadata failed first" "$scratch/out" ||
    fail "a query after the failure is not reported as not run: $(cat "$scratch/out")"

# --- ECO-738: past the Job's deadline, twins, and no World Bank source ---
STUB_ECO_738=0,1,80000000,2 run --database-url "postgresql://measure@127.0.0.1:${port}/rehearsal"
[ "$status" = 0 ] || fail "ECO-738 over the deadline: exit $status"
grep -qF "about 2000 s at 25 s per million rows; the crawler-migrate Job allows 1800 s for every pending data migration (OVER it)" \
    "$scratch/out" || fail "an estimate past the deadline is not flagged: $(grep -A5 ECO-738 "$scratch/out")"
grep -qF "The migration refuses this database until the twins are resolved" "$scratch/out" ||
    fail "twins are not flagged"
STUB_ECO_738=1,1,0,0 run --database-url "postgresql://measure@127.0.0.1:${port}/rehearsal"
[ "$status" = 0 ] || fail "ECO-738 already applied: exit $status"
grep -qF "Already applied here (20261101000100 in __diesel_schema_migrations): nothing is left to relabel" \
    "$scratch/out" || fail "an applied migration is not reported as such"
STUB_ECO_738=0,0,0,0 run --database-url "postgresql://measure@127.0.0.1:${port}/rehearsal"
[ "$status" = 1 ] || fail "no World Bank source: exit $status"
grep -qF "FAILED: unexpected query output: expected one data_sources row named World Bank Open Data, found 0" \
    "$scratch/out" || fail "a missing World Bank source is not a failed measurement: $(grep -A3 ECO-738 "$scratch/out")"

# --- FRED with a key: the crawler gets the key and no database URL; secrets stay hidden ---
FRED_API_KEY=fred-K3Y-secret DATABASE_URL=postgres://x:db-PW@h/d \
    run --database-url "postgresql://measure:pw-S3CRET-1@127.0.0.1:${port}/rehearsal"
[ "$status" = 0 ] || fail "FRED run: exit $status $(cat "$scratch/err")"
python3 - "$scratch/home/crawler.log" <<'PY'
import json, sys
log = json.load(open(sys.argv[1]))
assert log['argv'] == ['check-selection', '--source', 'FRED', '--json', '--count-uncapped'], log['argv']
assert 'FRED_API_KEY' in log['env'], log['env']
for name in ('DATABASE_URL', 'DATA_DATABASE_URL', 'PGPASSWORD'):
    assert name not in log['env'], (name, log['env'])
PY
grep -qF "select now: **812** (max_series 500; OVER the cap)" "$scratch/out" || fail "FRED count missing"
grep -qF "about 2436 requests" "$scratch/out" || fail "FRED fetch estimate missing"
grep -qE "K3Y|S3CRET|db-PW" "$scratch/out" "$scratch/err" && fail "a secret was printed"
# Without a count (an over-cap selection that stopped at the cap), the measurement failed.
touch "$HOME/capped"
FRED_API_KEY=fred-K3Y-secret run --database-url "postgresql://measure:pw-S3CRET-1@127.0.0.1:${port}/rehearsal"
rm "$HOME/capped"
[ "$status" = 1 ] || fail "FRED without a count: exit $status"
grep -qF "FAILED: count unknown" "$scratch/out" || fail "FRED without a count is not a failure"
grep -qF "None" "$scratch/out" && fail "the report prints None"

# --- The synthetic-row SQL is the one in econ-graph-core ---
python3 - "$ROOT" <<'PY'
import importlib.util, re, sys
from pathlib import Path
root = Path(sys.argv[1])
rust = (root / 'backend/crates/econ-graph-core/src/models/data_point.rs').read_text()
body = re.search(r'fn synthetic_legacy_row\(row: &str, other: &str\) -> String \{\s*format!\(\s*"(.*?)"\s*\)\s*\}',
                 rust, re.S).group(1)
rust_sql = re.sub(r'\\\n\s*', '', body).format(row='R', other='O')
spec = importlib.util.spec_from_file_location('m', root / 'scripts/deploy/measure-v41-first-deploy.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
assert m.synthetic_legacy_row('R', 'O') == rust_sql, (m.synthetic_legacy_row('R', 'O'), rust_sql)
PY

echo "measure-v41-first-deploy stub checks passed."
