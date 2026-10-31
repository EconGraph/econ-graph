#!/usr/bin/env bash
# Stub-only tests for measure-v41-first-deploy.sh. They never touch a cluster or a real
# database: psql, the crawler and a `kubectl port-forward` are stand-ins on PATH or local ports,
# and every database URL names a stub. They check:
#   - the target guard: no URL, in-cluster names and a port-forward into production's namespace
#     are refused (exit 64 / 65) before psql runs; other clusters and namespaces are allowed;
#     --i-know-this-is-production runs and says so;
#   - every psql script is one READ ONLY transaction with the statement timeout, the password
#     reaches psql only as PGPASSWORD, and no secret is printed;
#   - the BEA rules come from bea.toml, the FRED check runs only with FRED_API_KEY and without
#     any database URL, and the script's synthetic-row SQL matches data_point.rs.
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
import json, os, re, sys
script = sys.stdin.read()
with open(os.environ['STUB_LOG'], 'a') as log:
    log.write(json.dumps({'argv': sys.argv[1:], 'script': script,
                          'env': {k: v for k, v in os.environ.items() if k.startswith('PG')}}) + '\n')
sep = '\x1f'
if 'server_version' in script:
    print('18.0 (stub)')
elif 'is_active' in script:
    rules = script.count('count(*) FILTER') - 2
    print(sep.join(['7', '2', '2'] + ['1'] * rules))
elif 'other_original' in script:
    print(sep.join(['Federal Reserve Economic Data (FRED)', '3']))
else:
    sys.exit('stub psql: unexpected query')
STUB
# A crawler that records its environment and prints a FRED report over the cap.
cat >"$scratch/crawler" <<'STUB'
#!/usr/bin/env python3
import json, os, sys
with open(os.environ['HOME'] + '/crawler.log', 'w') as log:
    json.dump({'argv': sys.argv[1:], 'env': sorted(os.environ)}, log)
print(json.dumps({'sources': [{'source': 'FRED', 'status': 'problems', 'selected': 812,
    'rules': [], 'bounds': [{'what': 'selected series', 'count': 812, 'limit': 500}],
    'problems': [{'kind': 'over_cap', 'entry': 'selected series', 'detail': '812 > 500'}],
    'notices': [], 'refresh': {'requests': 2436, 'requests_per_minute': 120.0,
    'minutes': 20.3, 'daily_quota': None}}]}))
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

# A `kubectl port-forward` stand-in, listening on a free local port: a python3 started
# under the name kubectl, with port-forward arguments for the guard to read.
free_port() { python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])'; }
ln -s "$(command -v python3)" "$scratch/bin/kubectl"
forward() {  # forward PORT ARGS...: start the stand-in, wait until it listens
    local port="$1"; shift
    "$scratch/bin/kubectl" -c "import socket,time; s=socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1); s.bind(('127.0.0.1', $port)); s.listen(); time.sleep(120)" \
        port-forward "$@" "$port:5432" &
    pids+=("$!")
    for _ in $(seq 1 50); do
        ss -Hltn "sport = :$port" | grep -q . && return 0
        sleep 0.1
    done
    fail "the port-forward stand-in did not listen on $port"
}

# --- The target guard ---
run
[ "$status" = 64 ] || fail "no --database-url: exit $status"
psql_ran && fail "psql ran without a target"

for host in data-postgres-service postgres-service.econ-graph.svc.cluster.local \
    postgresql-0.postgres-service; do
    run --database-url "postgres://postgres@${host}:5432/econ_graph"
    [ "$status" = 65 ] || fail "in-cluster host $host: exit $status"
    grep -qF "refusing" "$scratch/err" || fail "no refusal message for $host"
    psql_ran && fail "psql ran against $host"
done

port="$(free_port)"
forward "$port" -n econ-graph svc/data-postgres-service --context kind-econ-graph
run --database-url "postgres://postgres@127.0.0.1:${port}/econ_graph"
[ "$status" = 65 ] || fail "port-forward into production: exit $status"
grep -qF "port-forward" "$scratch/err" || fail "the refusal does not name the port-forward"
psql_ran && fail "psql ran against production's port-forward"
run --database-url "postgres://postgres@127.0.0.1:${port}/econ_graph" --i-know-this-is-production
[ "$status" = 0 ] || fail "--i-know-this-is-production: exit $status $(cat "$scratch/err")"
grep -qF "(production)" "$scratch/out" || fail "a production run is not marked as such"

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

for args in "--context kind-eco803-rehearsal -n econ-graph" "-n econ-graph-br-feature --context kind-econ-graph"; do
    port="$(free_port)"
    # shellcheck disable=SC2086 # the arguments are words
    forward "$port" $args svc/data-postgres-service
    run --database-url "postgres://postgres@127.0.0.1:${port}/econ_graph"
    [ "$status" = 0 ] || fail "port-forward ($args) refused: exit $status $(cat "$scratch/err")"
done

# --- A measured run: read-only scripts, no secrets, rules from bea.toml, FRED skipped ---
port="$(free_port)"
run --database-url "postgresql://measure:pw-S3CRET-1@127.0.0.1:${port}/rehearsal" --statement-timeout 42
[ "$status" = 0 ] || fail "measured run: exit $status $(cat "$scratch/err")"
python3 - "$STUB_LOG" <<'PY'
import json, sys
calls = [json.loads(line) for line in open(sys.argv[1])]
assert len(calls) == 3, len(calls)
for call in calls:
    script = call['script']
    assert script.startswith('BEGIN;\nSET TRANSACTION READ ONLY;\n'), script[:80]
    assert "SET LOCAL statement_timeout = '42s';" in script, script[:200]
    assert script.rstrip().endswith('COMMIT;'), script[-40:]
    assert script.count(';') == 5, script  # BEGIN, SET, SET LOCAL, the query, COMMIT
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
grep -qF "one synthetic legacy row, and no other row: **3**" "$scratch/out" || fail "ECO-807 count missing"
grep -qF "skipped: needs FRED_API_KEY" "$scratch/out" || fail "FRED not reported as skipped"
[ ! -e "$scratch/home/crawler.log" ] || fail "the crawler ran without FRED_API_KEY"
rules="$(grep -c '^\[\[rule\]\]' "$ROOT/backend/crates/econ-graph-crawler/data/catalog_selection/bea.toml")"
rows="$(grep -cE '^\| (NIPA|Regional) \|' "$scratch/out")"
[ "$rules" = "$rows" ] || fail "bea.toml has $rules rules, the report $rows rows"

# --- FRED with a key: the crawler gets the key and no database URL; secrets stay hidden ---
FRED_API_KEY=fred-K3Y-secret DATABASE_URL=postgres://x:db-PW@h/d \
    run --database-url "postgresql://measure:pw-S3CRET-1@127.0.0.1:${port}/rehearsal"
[ "$status" = 0 ] || fail "FRED run: exit $status $(cat "$scratch/err")"
python3 - "$scratch/home/crawler.log" <<'PY'
import json, sys
log = json.load(open(sys.argv[1]))
assert log['argv'] == ['check-selection', '--source', 'FRED', '--json'], log['argv']
assert 'FRED_API_KEY' in log['env'], log['env']
for name in ('DATABASE_URL', 'DATA_DATABASE_URL', 'PGPASSWORD'):
    assert name not in log['env'], (name, log['env'])
PY
grep -qF "select now: **812** (max_series 500; OVER the cap)" "$scratch/out" || fail "FRED count missing"
grep -qF "about 2436 requests" "$scratch/out" || fail "FRED fetch estimate missing"
grep -qE "K3Y|S3CRET|db-PW" "$scratch/out" "$scratch/err" && fail "a secret was printed"

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
