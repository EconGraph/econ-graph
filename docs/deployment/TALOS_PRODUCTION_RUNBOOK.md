# Talos production runbook

This runbook covers a release on the Talos platform under [`platform/`](../../platform/README.md).
It covers promotion, verification, rollback and restore of the three CloudNativePG
clusters, the cutover from kind, and a rehearsal checklist. It replaces
[Deploying a release](RELEASE_DEPLOY.md), which describes the kind cluster only.

**Status: draft, not rehearsed.** Nothing here has run on real infrastructure. The repo
cannot yet run a production promotion end to end. Each place where a mechanism is
missing is marked **Gap** and listed in [Gaps](#gaps). Where the existing tooling
supports an interim manual step, the step is written out and flagged. Run the
[rehearsal](#7-rehearsal-checklist) on a scratch cluster before production uses any of
it (ECO-887, ECO-859 step 1).

Every command below runs from the repository root, in the session that
[Before you start](#before-you-start) sets up. Run it inside tmux or screen on the
operator host, so that a dropped ssh session doesn't interrupt a step. Checks print a
line starting `STOP` when something is wrong: stop there and investigate. The
commands never exit your shell.

## Before you start

### Tools

- `kubectl` 1.36.4 and `helm` 4.3.1, from the URLs and SHA-256 values in
  [`platform/toolchain.lock.json`](../../platform/toolchain.lock.json).
  [`scripts/platform/safety.py`](../../scripts/platform/safety.py) refuses a kubectl more than
  one minor version away from the target.
- `amtool` 0.34.2, also from that file.
- `sops`, with the age identity that decrypts the cluster's exported kubeconfig
  ([`infra/talos/README.md`](../../infra/talos/README.md), "Protect secrets and state").
- `docker` with `buildx`, `jq`, `git`, and `python3` with
  `pip install -r scripts/platform/requirements.txt`.
- `docker login ghcr.io` with a token that has `read:packages`. The release packages are
  private ([Release images](RELEASE_IMAGES.md)).

No Flux CLI and no CNPG kubectl plugin are pinned, so this runbook uses plain kubectl
for Flux and CNPG objects.

### Session variables

Set these at the start of every session. Fill in the values in angle brackets.

```sh
export CLUSTER=<name of this Talos cluster, from its inventory>
export KUBECTL="$HOME/bin/kubectl-1.36.4"       # the pinned client
export HELM="$HOME/bin/helm-4.3.1"
export STATE="$HOME/econgraph-talos/$CLUSTER"    # cluster.py --state-dir of this cluster
export TAG=vX.Y.Z-rc.N                           # the release being promoted, or running now
export PREV_TAG=<release that ran before TAG>    # empty if TAG is the first on this cluster
export SLUG="$(printf %s "$TAG" | tr '.' '-')"   # vX-Y-Z-rc-N: a valid Kubernetes name part
export OPS="$HOME/econgraph-ops/$CLUSTER"        # protected operator storage, outside Git
export SETTINGS_DIR="$OPS/settings"              # one settings file, digest list and deploy commit per release
export SECRETS_DIR="$OPS/secrets"                # the *.sops.yaml ciphertext for this cluster
export EVIDENCE_ROOT="$OPS/evidence"
export DEPLOY_REF="deploy/$CLUSTER-$TAG"         # see 1.3
mkdir -p "$SETTINGS_DIR" "$EVIDENCE_ROOT"
chmod 700 "$OPS"
```

A promotion starts a new run ([1.0](#10-start-a-run)). Any other session for the same
release reads back the run it continues:

```sh
export RUN="$(cat "$SETTINGS_DIR/last-run-$TAG")"
export EVIDENCE="$EVIDENCE_ROOT/$RUN"
```

On a scratch cluster also set the values that [`platform/README.md`](../../platform/README.md)
names:

```sh
export TRAIN4_OWNER=<owner_id UUID from the scratch Talos inventory>
export CONTROL_NODE=<a control-plane node name, e.g. econgraph-scratch-cp-1>
export CACHE="$HOME/.cache/train4-platform"      # populated by validate.py --fetch
```

### Kubeconfig

The Talos tooling exports the kubeconfig only as SOPS ciphertext
(`$STATE/kubeconfig.sops.yaml`). Open a shell with it decrypted to a temporary file for
that shell's lifetime. Every later step runs inside that shell:

```sh
sops exec-file --no-fifo "$STATE/kubeconfig.sops.yaml" 'KCFG={} bash'
```

`--no-fifo` is needed because the platform scripts run kubectl many times. The file is
removed when the shell exits. **Gap G12**: the repo has no documented way to give the
platform scripts a decrypted kubeconfig; this is an operator convention.

### Helpers

Paste these into the shell. The CNPG clusters are `econgraph-app` (database
`econ_graph_app`), `econgraph-data` (`econ_graph`) and `econgraph-keycloak` (`keycloak`),
all in namespace `econ-graph` ([`clusters.yaml`](../../platform/databases/clusters.yaml)).
psql runs as `postgres` inside the current primary, as
[`cnpg-drill.py`](../../scripts/platform/cnpg-drill.py) does.

```sh
export CTX="$("$KUBECTL" --kubeconfig "$KCFG" config current-context)"
k() { "$KUBECTL" --kubeconfig "$KCFG" --context "$CTX" "$@"; }

primary() { k -n econ-graph get cluster "$1" -o jsonpath='{.status.currentPrimary}'; }
psql_in() {  # psql_in CLUSTER DATABASE SQL
  k -n econ-graph exec -i "$(primary "$1")" -c postgres -- \
    psql -X -v ON_ERROR_STOP=1 -U postgres -d "$2" -qAt -c "$3"
}
plane_version() {  # plane_version app|data|keycloak CLUSTER: every applied migration, in order
  case "$1" in
    app)      psql_in "$2" econ_graph_app "SELECT string_agg(version, ',' ORDER BY version) FROM __diesel_schema_migrations" ;;
    data)     psql_in "$2" econ_graph "SELECT string_agg(version, ',' ORDER BY version) FROM __diesel_schema_migrations" ;;
    keycloak) psql_in "$2" keycloak "SELECT string_agg(version, ',' ORDER BY version) FROM migration_model" ;;
  esac
}
versions() { for p in app data keycloak; do echo "$p $(plane_version "$p" "econgraph-$p")"; done; }

suspend_ks() { k -n flux-system patch kustomization "$1" --type merge -p '{"spec":{"suspend":true}}'; }
resume_ks()  { k -n flux-system patch kustomization "$1" --type merge -p '{"spec":{"suspend":false}}'
               k -n flux-system annotate --overwrite kustomization "$1" reconcile.fluxcd.io/requestedAt="$(date -u +%s)"; }

flux_wait() {  # flux_wait KUSTOMIZATION COMMIT MINUTES: until it has applied COMMIT and is Ready
  for _ in $(seq $(( $3 * 4 ))); do
    ks="$(k -n flux-system get kustomization "$1" -o json)"
    applied="$(jq -r '.status.lastAppliedRevision // ""' <<<"$ks")"
    attempted="$(jq -r '.status.lastAttemptedRevision // ""' <<<"$ks")"
    ready="$(jq -r '[.status.conditions[]? | select(.type=="Ready")][0].status // ""' <<<"$ks")"
    reason="$(jq -r '[.status.conditions[]? | select(.type=="Ready")][0].reason // ""' <<<"$ks")"
    case "$applied" in *"$2"*) [ "$ready" = True ] && return 0 ;; esac
    case "$attempted" in *"$2"*) case "$reason" in BuildFailed|ReconciliationFailed|ArtifactFailed)
      echo "STOP: $1 failed on $2: $(jq -r '[.status.conditions[]? | select(.type=="Ready")][0].message' <<<"$ks")"; return 1 ;; esac ;; esac
    sleep 15
  done
  echo "STOP: $1 has not applied $2 after $3 minutes (Ready=$ready $reason)"; return 1
}
wait_job() {  # wait_job NAME MINUTES: until the Job completes; stops on failure
  for _ in $(seq $(( $2 * 4 ))); do
    [ "$(k -n econ-graph get job "$1" -o jsonpath='{.status.conditions[?(@.type=="Complete")].status}' 2>/dev/null)" = True ] && return 0
    if [ "$(k -n econ-graph get job "$1" -o jsonpath='{.status.conditions[?(@.type=="Failed")].status}' 2>/dev/null)" = True ]; then
      echo "STOP: job $1 failed"; k -n econ-graph logs "job/$1" --tail=50; return 1
    fi
    sleep 15
  done
  echo "STOP: job $1 not finished after $2 minutes"; return 1
}
wait_backup() {  # wait_backup NAME: until completed; stops on failed, or after 2 hours
  for _ in $(seq 480); do
    phase="$(k -n econ-graph get backup "$1" -o jsonpath='{.status.phase}')"
    case "$phase" in
      completed) return 0 ;;
      failed) echo "STOP: backup $1 failed: $(k -n econ-graph get backup "$1" -o jsonpath='{.status.error}')"; return 1 ;;
    esac
    sleep 15
  done
  echo "STOP: backup $1 still '$phase' after 2 hours"; return 1
}
wait_cluster() {  # wait_cluster NAME INSTANCES HOURS: until the Cluster exists, then until ready
  for _ in $(seq 60); do k -n econ-graph get cluster "$1" -o name >/dev/null 2>&1 && break; sleep 10; done
  k -n econ-graph wait "cluster/$1" --for=jsonpath='{.status.readyInstances}'="$2" --timeout="$3h"
}
load_digests() {  # load_digests TAG: export the digests recorded for that release in 1.2
  f="$SETTINGS_DIR/digests-$1.txt"
  export BACKEND_DIGEST="$(awk '$1=="econ-graph-backend"{print $2}' "$f")"
  export FRONTEND_DIGEST="$(awk '$1=="econ-graph-frontend"{print $2}' "$f")"
  export CRAWLER_DIGEST="$(awk '$1=="econ-graph-crawler-worker"{print $2}' "$f")"
  [ -n "$BACKEND_DIGEST" ] && [ -n "$FRONTEND_DIGEST" ] && [ -n "$CRAWLER_DIGEST" ] || echo "STOP: no digests in $f"
}
check_settings() {  # check_settings FILE: bootstrap.py reconcile's settings checks (lines 89-103), without the scratch ENVIRONMENT rule
  python3 - "$1" <<'PY'
import json, re, sys, yaml
doc = yaml.safe_load(open(sys.argv[1]))
problems = []
if doc.get('kind') != 'ConfigMap' or doc.get('metadata', {}).get('name') != 'platform-settings':
    problems.append('not the platform-settings ConfigMap')
if doc.get('metadata', {}).get('namespace') != 'flux-system':
    problems.append('metadata.namespace must be flux-system')
data = doc.get('data', {})
problems += ['example value: ' + x for x in ['example.invalid', '192.0.2.', 'localhost', '${'] if x in json.dumps(data)]
problems += ['no digest: ' + k for k in ['BACKEND_IMAGE', 'FRONTEND_IMAGE', 'CRAWLER_IMAGE']
             if not re.fullmatch(r'[^\s]+@sha256:[0-9a-f]{64}', data.get(k, ''))]
if not re.fullmatch(r'\d{1,3}(\.\d{1,3}){3}:\d{1,5}', data.get('HOST_EXPORTER_TARGET', '')):
    problems.append('HOST_EXPORTER_TARGET must be the host bridge IPv4 address and node_exporter port')
print('\n'.join('STOP: ' + p for p in problems) or 'settings OK')
sys.exit(1 if problems else 0)
PY
}
pin_ref() {  # pin_ref COMMIT: apply the Flux objects rendered from this checkout, with the GitRepository pinned to COMMIT
  [ "$(git rev-parse HEAD)" = "$1" ] || { echo "STOP: check out $1 first"; return 1; }
  "$KUBECTL" kustomize platform/clusters/threadripper | PIN="$1" python3 -c '
import os, sys, yaml
docs = [d for d in yaml.safe_load_all(sys.stdin) if d]
for d in docs:
    if d["kind"] == "GitRepository":
        d["spec"]["ref"] = {"commit": os.environ["PIN"]}
sys.stdout.write(yaml.safe_dump_all(docs))' | k apply -f -
  k -n flux-system annotate --overwrite gitrepository econgraph-platform reconcile.fluxcd.io/requestedAt="$(date -u +%s)"
}
```

`pin_ref` makes the same write that `bootstrap.py reconcile` makes last
([`bootstrap.py`](../../scripts/platform/bootstrap.py) lines 119-124). It renders the
Kustomizations and the GitRepository from the checked-out `platform/clusters/threadripper`,
sets the GitRepository's `spec.ref` to `{commit: COMMIT}`, and applies them all. Because it
uses `kubectl apply`, a `suspend: true` that was set with a patch survives it.

Check the cluster you're on:

```sh
k get nodes -o wide                                                # the inventory's nodes, all Ready
k get namespace kube-system -o jsonpath='{.metadata.uid}{"\n"}'    # record it: this cluster's identity
```

### Monitoring access

```sh
k -n monitoring get svc   # the names below assume Helm release "monitoring" (platform/addons/monitoring/release.yaml)
k -n monitoring port-forward svc/monitoring-kube-prometheus-alertmanager 9093:9093 >/dev/null &
k -n monitoring port-forward svc/monitoring-kube-prometheus-prometheus 9090:9090 >/dev/null &
k -n monitoring port-forward svc/monitoring-grafana 3000:80 >/dev/null &
export AM=http://127.0.0.1:9093
firing() { curl -s http://127.0.0.1:9090/api/v1/query --data-urlencode 'query=ALERTS{alertstate="firing"}' |
           jq -r '.data.result[].metric | "\(.alertname)\t\(.severity)\t\(.namespace // "")"' | sort; }
```

Grafana is at `http://127.0.0.1:3000`. Use the admin credentials from the protected file
that the `monitoring/grafana-admin` Secret was made from; never read them from the cluster.

## 1. Promote a release

### What reaches Flux

Three inputs decide what runs. The release tag alone is none of them:

| Input | Where it lives | Set by |
| --- | --- | --- |
| Manifests (`platform/`, `k8s/base`, `config/keycloak`, `scripts/keycloak`) | the Git commit pinned in GitRepository `flux-system/econgraph-platform` | `bootstrap.py reconcile` pins `spec.ref.commit` to the commit of `origin/<--git-ref>` (lines 111-124) |
| Encrypted Secrets | `platform/secrets/*.sops.yaml` at that same commit | committed by the operator; on `main` the list is empty ([`platform/secrets/kustomization.yaml`](../../platform/secrets/kustomization.yaml)) |
| Image references (`BACKEND_IMAGE`, `FRONTEND_IMAGE`, `CRAWLER_IMAGE`) and site settings | ConfigMap `flux-system/platform-settings`, substituted into every Kustomization (`postBuild.substituteFrom`) | applied with kubectl from a local file by `bootstrap.py reconcile`; not in Git |

`BACKEND_IMAGE` runs `econ-graph-backend` and `econ-graph-data-backend`. `CRAWLER_IMAGE`
runs `crawler-worker` and the `crawler-migrate` Job. `FRONTEND_IMAGE` runs
`econ-graph-frontend`. Keycloak's image is fixed in `k8s/base/application.yaml`, so it
changes only with the Git commit.

Gaps:

- **G1**: `bootstrap.py` runs only on a scratch cluster. Every node must carry the scratch
  owner label (lines 60-63) and `ENVIRONMENT` must be `fit-` or `scratch-` plus the owner
  (line 93, `scratch_environment`). No production bootstrap or reconcile exists (ECO-864).
- **G2**: `--git-ref` must be a branch on `origin`, not `main` or `master`, and not a tag
  (it resolves `refs/remotes/origin/<ref>`). A release tag's commit has no encrypted
  inputs. So the ref Flux tracks is a **deploy ref**: the tag's commit plus one commit
  that adds the ciphertext (1.3).
- **G3**: image digests are not in Git. The audit trail is `$SETTINGS_DIR` and the run's
  evidence, not a pull request (ECO-235, unscheduled).
- **G4**: GHCR packages are private, and nothing under `platform/` or `infra/talos/` gives
  the nodes a pull credential (no `imagePullSecrets`, no Talos registry auth). As the repo
  stands, pods fail with `ErrImagePull`.
- **G5**: Release Images also builds `econ-graph-chart-api`, but `k8s/base` has no
  chart-api Deployment, while `econ-graph-config` still points `CHART_API_SERVICE_URL` at
  `chart-api-service` (ECO-864).

### 1.0 Start a run

Each promotion attempt gets its own id. Backups, restore points and evidence are named
by it, so promoting the same tag again never reuses a name:

```sh
export RUN="$SLUG-$(date -u +%Y%m%d%H%M)"
export EVIDENCE="$EVIDENCE_ROOT/$RUN"
mkdir -p "$EVIDENCE"
echo "$RUN" > "$SETTINGS_DIR/last-run-$TAG"
echo "$RUN $TAG prev=$PREV_TAG cluster=$CLUSTER $(k get namespace kube-system -o jsonpath='{.metadata.uid}') operator=$(whoami)" | tee "$EVIDENCE/run.txt"
```

### 1.1 Resolve the image digests

Resolve the tag to digests the same way [`release.yml`](../../.github/workflows/release.yml)
does, and check that the tag and `sha-<commit>` give one digest:

```sh
git fetch origin --tags
export COMMIT="$(git rev-parse "$TAG^{commit}")"
for image in econ-graph-backend econ-graph-frontend econ-graph-crawler-worker; do
  by_tag="$(docker buildx imagetools inspect "ghcr.io/econgraph/$image:$TAG" --format '{{json .Manifest}}' | jq -r .digest)"
  by_sha="$(docker buildx imagetools inspect "ghcr.io/econgraph/$image:sha-$COMMIT" --format '{{json .Manifest}}' | jq -r .digest)"
  if [ "$by_tag" = "$by_sha" ] && [ "${by_tag#sha256:}" != "$by_tag" ]; then echo "$image $by_tag"; else echo "STOP: $image $by_tag $by_sha"; fi
done | tee "$EVIDENCE/digests.txt"
grep -q STOP "$EVIDENCE/digests.txt" || cp "$EVIDENCE/digests.txt" "$SETTINGS_DIR/digests-$TAG.txt"
load_digests "$TAG"
```

### 1.2 Make the deploy ref

Promoting a tag again reuses its deploy ref. The ref may also carry restore commits
(4.5), which must stay.

```sh
git fetch origin
if git ls-remote --exit-code --heads origin "$DEPLOY_REF" >/dev/null; then
  git switch -C "$DEPLOY_REF" "origin/$DEPLOY_REF"
  git merge-base --is-ancestor "$COMMIT" HEAD || echo "STOP: $DEPLOY_REF is not built on $TAG"
else
  git switch -C "$DEPLOY_REF" "$COMMIT"
fi
git diff --name-only "$COMMIT" HEAD | grep -v -E '^platform/(secrets/|databases/clusters\.yaml$)' | sed 's/^/STOP: unexpected change: /'

cp "$SECRETS_DIR"/*.sops.yaml platform/secrets/
{ printf 'apiVersion: kustomize.config.k8s.io/v1beta1\nkind: Kustomization\nresources:\n'
  ls platform/secrets | grep '\.sops\.yaml$' | sed 's/^/- /'; } > platform/secrets/kustomization.yaml
"$KUBECTL" kustomize platform/secrets | python3 -c '
import sys, yaml
sys.path.insert(0, "scripts/platform")
from bootstrap import check_encrypted_inputs
try:
    check_encrypted_inputs([d for d in yaml.safe_load_all(sys.stdin) if d])
    print("encrypted inputs OK")
except ValueError as error:
    print("STOP:", error)'
git add platform/secrets
git diff --cached --quiet || git commit -m "chore(deploy): encrypted inputs for $CLUSTER $TAG"
git push origin "$DEPLOY_REF"
git fetch origin
export DEPLOY_COMMIT="$(git rev-parse "origin/$DEPLOY_REF")"
[ "$(git rev-parse HEAD)" = "$DEPLOY_COMMIT" ] || echo "STOP: local $DEPLOY_REF differs from origin"
git diff --quiet "$DEPLOY_COMMIT" -- platform/secrets || echo "STOP: platform/secrets differs from $DEPLOY_COMMIT"
echo "$TAG $COMMIT $DEPLOY_REF $DEPLOY_COMMIT" | tee "$EVIDENCE/refs.txt"
```

The check reuses `check_encrypted_inputs` from `bootstrap.py`: every required Secret is
present, SOPS-encrypted, and has its required keys. The list is in
[`platform/README.md`](../../platform/README.md), "Secret inputs".

Read what else this release changes on the platform, because every unsuspended
Kustomization applies the new commit too. A changed `imageName` in `clusters.yaml`, for
example, restarts each database:

```sh
base="$( [ -n "$PREV_TAG" ] && cat "$SETTINGS_DIR/deploy-commit-$PREV_TAG" || echo "$COMMIT")"
git diff --stat "$base" "$DEPLOY_COMMIT" -- platform k8s/base config/keycloak scripts/keycloak | tee "$EVIDENCE/platform-diff.txt"
echo "$DEPLOY_COMMIT" > "$SETTINGS_DIR/deploy-commit-$TAG"
```

### 1.3 Write the settings file

Start from the previous release's file. On the first promotion, start from
[`settings.example.yaml`](../../platform/clusters/threadripper/settings.example.yaml) and
replace every value as [`platform/README.md`](../../platform/README.md) describes. Then set
the images and check the file:

```sh
export SETTINGS="$SETTINGS_DIR/platform-settings-$TAG.yaml"
if [ -n "$PREV_TAG" ]; then cp "$SETTINGS_DIR/platform-settings-$PREV_TAG.yaml" "$SETTINGS"
else cp platform/clusters/threadripper/settings.example.yaml "$SETTINGS"; echo "edit every value in $SETTINGS now"; fi
python3 - "$SETTINGS" <<'PY'
import os, sys, yaml
path = sys.argv[1]
doc = yaml.safe_load(open(path))
doc['metadata']['namespace'] = 'flux-system'
for key, image, digest in [('BACKEND_IMAGE', 'econ-graph-backend', 'BACKEND_DIGEST'),
                           ('FRONTEND_IMAGE', 'econ-graph-frontend', 'FRONTEND_DIGEST'),
                           ('CRAWLER_IMAGE', 'econ-graph-crawler-worker', 'CRAWLER_DIGEST')]:
    doc['data'][key] = 'ghcr.io/econgraph/%s@%s' % (image, os.environ[digest])
yaml.safe_dump(doc, open(path, 'w'), sort_keys=False)
PY
check_settings "$SETTINGS" && cp "$SETTINGS" "$EVIDENCE/"
```

`check_settings` must print `settings OK`.

**Gap G8**: the ObjectStores write to `s3://econgraph-${ENVIRONMENT}-*-backups`
([`objectstores.yaml`](../../platform/databases/objectstores.yaml)), and `bootstrap.py`
accepts only scratch `ENVIRONMENT` values. No production value or production bucket
names are defined (ECO-864).

### 1.4 Record the way back

Before anything changes, record what runs now and back up all three databases.

```sh
k -n flux-system get gitrepository econgraph-platform -o jsonpath='{.spec.ref}{"\n"}' | tee "$EVIDENCE/before-git-ref.json"
k -n flux-system get configmap platform-settings -o yaml > "$EVIDENCE/before-platform-settings.yaml"
versions | tee "$EVIDENCE/before-versions.txt"
k -n econ-graph get deployments -o custom-columns='NAME:.metadata.name,REPLICAS:.spec.replicas' | tee "$EVIDENCE/before-replicas.txt"

for plane in app data keycloak; do
  k apply -f - <<EOF
apiVersion: postgresql.cnpg.io/v1
kind: Backup
metadata:
  name: econgraph-$plane-pre-$RUN
  namespace: econ-graph
spec:
  cluster:
    name: econgraph-$plane
  method: plugin
  pluginConfiguration:
    name: barman-cloud.cloudnative-pg.io
EOF
done
for plane in app data keycloak; do
  wait_backup "econgraph-$plane-pre-$RUN" &&
    echo "$plane $(k -n econ-graph get backup "econgraph-$plane-pre-$RUN" -o jsonpath='{.status.backupId} {.status.stoppedAt}')"
done | tee "$EVIDENCE/backup-ids.txt"
```

`backup-ids.txt` must have three lines of the form `PLANE BACKUP_ID STOPPED_AT`. A
restore to this run's restore point (4.1) names that backup, because for a named target
CNPG otherwise uses the newest backup, and the app and keycloak clusters back up hourly.

### 1.5 Quiesce

Hold the application back, so that the data migrations run first, against a quiet
database. This is the Talos form of kind's `precheck-db-migration.sh --stop-crawler`.

```sh
suspend_ks platform-application
k -n econ-graph scale deployment/crawler-worker --replicas=0
k -n econ-graph wait --for=delete pod -l app=crawler-worker --timeout=5m
psql_in econgraph-data econ_graph "SELECT pid, usename, application_name, state, now() - xact_start
  FROM pg_stat_activity WHERE datname = current_database() AND pid <> pg_backend_pid()
  AND xact_start IS NOT NULL AND (state LIKE 'idle in transaction%' OR now() - xact_start > interval '5 seconds')"
```

The query must print nothing. If it lists sessions, find their owners before going on;
don't cancel them blindly. While `platform-application` is suspended, Flux doesn't put
the crawler back.

**Abandoning here**: before 1.7 nothing has changed but the crawler. To undo:

```sh
resume_ks platform-application
k -n econ-graph rollout status deployment/crawler-worker --timeout=10m
k -n econ-graph get deployment crawler-worker -o jsonpath='{.status.availableReplicas}{"\n"}'   # 1
```

### 1.6 Mark the restore points

With the crawler stopped, mark a named restore point in each database. A rollback after a
migration restores to it (3.3). Each WAL switch makes PostgreSQL archive the segment that
holds the point. Wait for that segment's archive marker, as `cnpg-drill.py` does:

```sh
for pair in app:econ_graph_app data:econ_graph keycloak:keycloak; do
  plane="${pair%%:*}"; db="${pair#*:}"
  psql_in "econgraph-$plane" "$db" "SELECT pg_create_restore_point('pre-$RUN')" >/dev/null
  wal="$(psql_in "econgraph-$plane" "$db" 'SELECT pg_walfile_name(pg_switch_wal())')"
  until [ "$(psql_in "econgraph-$plane" "$db" "SELECT EXISTS (SELECT 1 FROM pg_ls_archive_statusdir() WHERE name = '$wal.done')")" = t ]; do sleep 5; done
  echo "$plane pre-$RUN $wal $(date -u +%FT%TZ)"
done | tee "$EVIDENCE/restore-points.txt"
```

The application still serves until the backends roll in 1.8. Sign-ups, annotations and
logins made between the restore point and the roll are lost if 3.3 restores to it.

### 1.7 Apply the new ref and settings

The checkout must be at `$DEPLOY_COMMIT` (1.2).

**Scratch cluster** (supported by the repo):

```sh
python3 scripts/platform/bootstrap.py reconcile --kubeconfig "$KCFG" --context "$CTX" \
  --owner "$TRAIN4_OWNER" --control-node "$CONTROL_NODE" --cache "$CACHE" \
  --kubectl "$KUBECTL" --helm "$HELM" \
  --settings "$SETTINGS" --git-ref "$DEPLOY_REF" --execute | tee "$EVIDENCE/reconcile.json"
```

**Production** (**Gap G1**: interim manual equivalent, unreviewed). `bootstrap.py
reconcile` checks that `sops-age` and `platform-git-auth` exist and that the encrypted
package matches the commit. It then makes three writes: the settings ConfigMap, then the
rendered Kustomizations and the GitRepository pinned to the commit. The checks ran in 1.2
and 1.3 (`check_encrypted_inputs`, the `platform/secrets` diff, `check_settings`), except
the scratch `ENVIRONMENT` rule (G8). These are the writes:

```sh
k -n flux-system get secret sops-age platform-git-auth -o name   # both must exist; never print their data
check_settings "$SETTINGS" && k apply -f "$SETTINGS"
pin_ref "$DEPLOY_COMMIT"
```

On both kinds of cluster, check that the application is still held back:

```sh
k -n flux-system get kustomization platform-application -o jsonpath='{.spec.suspend}{"\n"}'   # true
```

If it prints anything else, the crawler comes back before the migration. The backend
still waits for `platform-data-schema` through `dependsOn`.

### 1.8 Migrate, then release the application

```sh
flux_wait platform-data-schema "$DEPLOY_COMMIT" 30
wait_job crawler-migrate 30
k -n econ-graph logs job/crawler-migrate > "$EVIDENCE/crawler-migrate.log"
resume_ks platform-application
flux_wait platform-application "$DEPLOY_COMMIT" 30
wait_job keycloak-realm-import 15
for d in econ-graph-backend econ-graph-data-backend econ-graph-frontend keycloak crawler-worker; do
  k -n econ-graph rollout status "deployment/$d" --timeout=15m
done
date -u +%FT%TZ > "$EVIDENCE/promoted-at.txt"
```

If `crawler-migrate` fails, don't resume the application: go to
[3.1](#31-stop-and-decide). A failed run can still leave earlier migrations applied, so
3.1 decides between 3.2 and 3.3.

If `platform-application` stops with `field is immutable` on Job `keycloak-realm-import`,
the completed Job from the last release is still there (**Gap G15**). Delete it, then wait
again:

```sh
k -n econ-graph delete job keycloak-realm-import
k -n flux-system annotate --overwrite kustomization platform-application reconcile.fluxcd.io/requestedAt="$(date -u +%s)"
flux_wait platform-application "$DEPLOY_COMMIT" 30
```

The backends migrate `econ_graph_app` when they start, and two replicas roll one at a
time (`maxUnavailable: 1`, `maxSurge: 0`). For a short time an old replica serves
against the new schema. No rule requires migrations to stay compatible with the
previous binary. **Gap G6** (ECO-309 migration Job, unscheduled). Keycloak migrates its
own database the first time a new Keycloak version starts.

## 2. Verify

These checks replace kind's `verify-release-images.sh` and the verify step of
`release-deploy.sh`. Each one saves to `$EVIDENCE`. To verify after a rollback, first run
`load_digests "$PREV_TAG"` and use `PIN="$(cat "$SETTINGS_DIR/deploy-commit-$PREV_TAG")"`.
Otherwise use `PIN="$DEPLOY_COMMIT"`.

1. **Flux applied the pinned commit everywhere.** Every row is `True`, not suspended, with
   `$PIN` in its revision:

   ```sh
   k -n flux-system get gitrepository econgraph-platform -o jsonpath='{.status.artifact.revision}{"\n"}'
   k -n flux-system get kustomizations -o custom-columns='NAME:.metadata.name,READY:.status.conditions[?(@.type=="Ready")].status,SUSPENDED:.spec.suspend,REVISION:.status.lastAppliedRevision' | tee "$EVIDENCE/flux.txt"
   grep -v -e "$PIN" -e '^NAME' "$EVIDENCE/flux.txt" | sed 's/^/STOP: not on the pinned commit: /'
   grep -v -e '^NAME' "$EVIDENCE/flux.txt" | grep -v -E ' True +(false|<none>) ' | sed 's/^/STOP: not Ready or suspended: /'
   k -n flux-system get helmreleases | tee "$EVIDENCE/helmreleases.txt"   # every READY is True
   ```

2. **Every pod runs the digest, and each Deployment has all its replicas:**

   ```sh
   for pair in "econ-graph-backend=$BACKEND_DIGEST" "econ-graph-data-backend=$BACKEND_DIGEST" \
               "econ-graph-frontend=$FRONTEND_DIGEST" "crawler-worker=$CRAWLER_DIGEST"; do
     d="${pair%%=*}"; want="${pair#*=}"
     n="$(k -n econ-graph get deployment "$d" -o jsonpath='{.spec.replicas}')"
     ids="$(k -n econ-graph get pods -l app="$d" --field-selector=status.phase=Running \
            -o jsonpath='{range .items[*]}{.status.containerStatuses[0].imageID}{"\n"}{end}')"
     total="$(grep -c . <<<"$ids")"; ok="$(grep -c -- "$want\$" <<<"$ids")"
     echo "$d replicas=$n running=$total on-digest=$ok"
     [ "$n" = "$total" ] && [ "$n" = "$ok" ] || echo "STOP: $d"
   done | tee "$EVIDENCE/images.txt"
   ```

3. **Migrations.** Record what is applied now. After a promotion, compare it with
   `before-versions.txt` to see what the release migrated. After a 3.3 rollback, it must
   equal `before-versions.txt`.

   ```sh
   versions | tee "$EVIDENCE/after-versions.txt"
   diff "$EVIDENCE/before-versions.txt" "$EVIDENCE/after-versions.txt"
   ```

4. **Databases and backups.** Each cluster reports `Cluster in healthy state` with 2 ready
   instances and `ContinuousArchiving` `True`, and a manual backup health check passes:

   ```sh
   for plane in app data keycloak; do
     k -n econ-graph get cluster "econgraph-$plane" -o jsonpath='{.metadata.name} {.status.phase} {.status.readyInstances} {.status.conditions[?(@.type=="ContinuousArchiving")].status}{"\n"}'
   done | tee "$EVIDENCE/clusters.txt"
   job="backup-health-$(date -u +%Y%m%d%H%M%S)"
   k -n econ-graph create job --from=cronjob/cnpg-backup-health "$job"
   wait_job "$job" 5
   k -n econ-graph logs "job/$job" | tee -a "$EVIDENCE/backup-health.txt"
   ```

5. **Alerts.** Only `Watchdog`, and `InfoInhibitor` if present, may be firing:

   ```sh
   firing | tee "$EVIDENCE/alerts-after.txt"
   amtool --alertmanager.url="$AM" alert query
   amtool --alertmanager.url="$AM" silence query
   ```

   Also check that healthchecks.io shows regular Watchdog pings
   ([Alert delivery](../../platform/README.md#alert-delivery)), and that no silence from a
   rollback or restore is still active. The rules come from `platform/observability/`:
   - `monitors.yaml`: Postgres readiness and backup health.
   - `rules-edge.yaml`: edge 5xx and latency, Keycloak.
   - `rules-auth.yaml`: anonymous grants.
   - `rules-crawler.yaml`: crawler and SEC.
   - `rules-disk.yaml`: host, node and database volumes.
   - `rules-timeseries.yaml` and `rules-tunnel.yaml`.

   Watch for 30 minutes after the rollout. Several crawler alerts need `for: 1h` to fire,
   so look again the next day.

6. **Dashboards** in Grafana. Look for a dip or step at the promotion time:

   | Dashboard (uid) | Look at |
   | --- | --- |
   | EconGraph platform recovery (`econgraph-platform`) | CNPG pod readiness; seconds since the last successful backup check |
   | EconGraph disk and capacity (`econgraph-disk`) | database volumes against their 10Gi/100Gi/10Gi requests, growth per day, Prometheus and Loki volumes |
   | Crawler Overview (`econgraph-crawler-overview`) | queue depth and completions resuming after the crawler restart |
   | Crawler Politeness (`econgraph-crawler-politeness`) | rate-limit hits after the restart |
   | SEC EDGAR Crawler (`econgraph-sec-edgar-crawler`) | the nightly load (ECO-861) |
   | EconGraph Logs & Debugging (`econgraph-logging`) | errors from the new pods |

   The chart's own Kubernetes dashboards show restarts and resource use per workload.

7. **The site.** Before the tunnel is on, request the gateway directly. It has an HTTP
   listener only ([`gateway.yaml`](../../platform/network/gateway.yaml)) and routes by
   `PUBLIC_HOST`:

   ```sh
   GATEWAY_IP="$(k -n econ-graph-gateway get gateway econ-graph-gateway -o jsonpath='{.status.addresses[0].value}')"
   PUBLIC_HOST="$(k -n flux-system get configmap platform-settings -o jsonpath='{.data.PUBLIC_HOST}')"
   curl -sS -o /dev/null -w '%{http_code} /\n' -H "Host: $PUBLIC_HOST" "http://$GATEWAY_IP/"
   curl -sS -H "Host: $PUBLIC_HOST" "http://$GATEWAY_IP/idp/realms/econ-graph/.well-known/openid-configuration" | jq -r .issuer
   curl -sS -H "Host: $PUBLIC_HOST" -H 'Content-Type: application/json' \
     -d '{"query":"{ __typename }"}' "http://$GATEWAY_IP/graphql"
   ```

   The first prints `200`. The issuer is `${PUBLIC_ORIGIN}/idp/realms/econ-graph`. The
   GraphQL request returns a `data` object with a type name, not `errors`.

   Then run the QA scenarios ([qa-scenarios.md](../release/qa-scenarios.md)) over plain
   HTTP, with `PUBLIC_HOST` resolving to the gateway, and remove the hosts line afterwards:

   ```sh
   echo "$GATEWAY_IP $PUBLIC_HOST # econgraph-runbook" | sudo tee -a /etc/hosts
   scripts/qa/run-scenarios.sh --target "http://$PUBLIC_HOST" --profile lab --json "$EVIDENCE/qa.json"
   grep -v '# econgraph-runbook$' /etc/hosts > "$HOME/hosts.runbook" && sudo cp "$HOME/hosts.runbook" /etc/hosts && rm "$HOME/hosts.runbook"
   ```

   This run has limits. When `PUBLIC_ORIGIN` is `https://...`, sign-in redirects, secure
   cookies and the issuer expect HTTPS, so scenarios that sign in are expected to fail.
   Record which ones fail. After the tunnel is on, run
   `scripts/deploy/check-public-site.sh --host econgraph.com --via-cloudflare` and the
   `production` profile (ECO-859 step 5). **Gap G11**: there is no HTTPS check before the
   tunnel, and `check-public-site.sh`'s default local mode expects kind's HTTPS gateway on
   the host.

## 3. Roll back

Under Flux, a rollback re-pins the previous deploy commit and re-applies the previous
settings file. `kubectl set image` or `kubectl rollout undo` would only last until Flux
next reconciles. Which rollback you need depends on whether a migration ran.

**First release on this cluster** (`PREV_TAG` empty): there is no previous release to go
back to, so 3.2 and 3.3 don't apply. Keep `platform-application` suspended and the
backends at 0 (3.1). Then either promote a fixed release candidate (section 1), or
restore the databases to the run's restore point with section 4, using
`RESTORE_FOR_TAG=$TAG`, and leave the application down until a fixed candidate is
promoted. Starting the same release again would re-run its migrations. Before the tunnel
is on, nothing is public. Whether kind is the fallback is ECO-883's decision.

Silence the alerts that the rollback causes ([3.4](#34-alerts-expected-during-a-rollback))
before you start.

### 3.1 Stop and decide

Stop everything that migrates, then compare the full set of applied migrations with the
set from before the promotion:

```sh
suspend_ks platform-application
k -n econ-graph scale deployment/econ-graph-backend deployment/econ-graph-data-backend --replicas=0
k -n econ-graph wait --for=delete pod -l 'app in (econ-graph-backend,econ-graph-data-backend)' --timeout=10m
versions > "$EVIDENCE/rollback-versions.txt"
diff "$EVIDENCE/before-versions.txt" "$EVIDENCE/rollback-versions.txt" && echo "no migration ran"
```

This is an outage from here until 3.2 or 3.3 ends. Keycloak keeps running, and so does
the frontend, which shows errors.

- No difference: use 3.2.
- Any difference: use 3.3.

### 3.2 Before a migration ran

Check out the previous release's deploy ref, then apply it.

```sh
load_digests "$PREV_TAG"
export PREV_DEPLOY_REF="deploy/$CLUSTER-$PREV_TAG"
git fetch origin
git switch -C "$PREV_DEPLOY_REF" "origin/$PREV_DEPLOY_REF"
export PREV_DEPLOY_COMMIT="$(git rev-parse HEAD)"
[ "$PREV_DEPLOY_COMMIT" = "$(cat "$SETTINGS_DIR/deploy-commit-$PREV_TAG")" ] || echo "STOP: $PREV_DEPLOY_REF moved since it was promoted"
```

Scratch cluster:

```sh
python3 scripts/platform/bootstrap.py reconcile --kubeconfig "$KCFG" --context "$CTX" \
  --owner "$TRAIN4_OWNER" --control-node "$CONTROL_NODE" --cache "$CACHE" \
  --kubectl "$KUBECTL" --helm "$HELM" \
  --settings "$SETTINGS_DIR/platform-settings-$PREV_TAG.yaml" --git-ref "$PREV_DEPLOY_REF" --execute | tee "$EVIDENCE/rollback-reconcile.json"
```

Production (Gap G1, as in 1.7):

```sh
check_settings "$SETTINGS_DIR/platform-settings-$PREV_TAG.yaml" && k apply -f "$SETTINGS_DIR/platform-settings-$PREV_TAG.yaml"
pin_ref "$PREV_DEPLOY_COMMIT"
```

Then, on both kinds of cluster:

```sh
flux_wait platform-data-schema "$PREV_DEPLOY_COMMIT" 30
wait_job crawler-migrate 30
resume_ks platform-application
flux_wait platform-application "$PREV_DEPLOY_COMMIT" 30
for d in econ-graph-backend econ-graph-data-backend econ-graph-frontend keycloak crawler-worker; do
  k -n econ-graph rollout status "deployment/$d" --timeout=15m
done
k -n econ-graph get deployments -o custom-columns='NAME:.metadata.name,REPLICAS:.spec.replicas' | diff "$EVIDENCE/before-replicas.txt" -
```

Resuming `platform-application` restores the replica counts from Git. The `diff` must
print nothing. Then run all of [Verify](#2-verify) with the previous release's digests and
commit, as its first paragraph says.

### 3.3 After a migration ran

The previous release can't run on the migrated schema, and migrations are forward-only
([MIGRATIONS.md](../development/MIGRATIONS.md)). Rollback means restoring the databases to
the run's `pre-$RUN` restore points (1.6), then running the previous release. Writes made
after those points are lost: on `econ_graph_app`, sign-ups, annotations and settings since
the promotion; on `econ_graph`, crawled data. Say so wherever users are told about the
outage.

Which planes to restore:

- **app and data together** whenever either one's line differs. App rows refer to data
  series, and the two restore points were taken at the same moment, so restoring both
  keeps them in step.
- **keycloak** as well when its line differs. If you are unsure, restore all three.

Steps:

1. Follow [4. Restore](#4-restore-a-cnpg-cluster-from-the-object-store) for each plane,
   with `RESTORE_FOR_TAG=$PREV_TAG` and `RECOVERY_TARGET=name:pre-$RUN`. The recovery
   spec is then committed on the previous release's deploy ref.
2. In 4.12, apply the previous settings file before resuming the application:
   `check_settings "$SETTINGS_DIR/platform-settings-$PREV_TAG.yaml" && k apply -f "$SETTINGS_DIR/platform-settings-$PREV_TAG.yaml"`.
3. Run all of [Verify](#2-verify) with the previous release's digests and commit. The
   versions must equal `before-versions.txt`.

### 3.4 Alerts expected during a rollback

Silence the alerts the outage causes, sized to the window. Allow 1 hour for 3.2. For 3.3,
allow 2 hours for the app and keycloak planes and 6 hours with the data plane, which
restores up to 100Gi:

```sh
export SILENCE_FOR=1h     # 3.2; 2h for an app or keycloak restore; 6h with the data plane
export SILENCE_ID="$(amtool --alertmanager.url="$AM" silence add --duration="$SILENCE_FOR" \
  --comment="rollback or restore $RUN (Talos production runbook)" \
  'alertname=~"CrawlerWorkerDown|CrawlerMetricsMissing|EconGraphEdge5xxRatioHigh|EconGraphKeycloakDown|EconGraphKeycloakInstanceDown|EconGraphPostgresInstanceNotReady|EconGraphBackupHealthCheckStale|KubeJobFailed"')"
echo "$SILENCE_ID $SILENCE_FOR" | tee -a "$EVIDENCE/silences.txt"
```

The silence covers every alert in the table below except `AnonymousGrant*` and
`CloudflareTunnel*`. Those are left to page, because firing during a rollback would be a
real fault. `KubeJobFailed` is silenced for every Job, so watch `crawler-migrate`
directly (`wait_job`) while the silence is active. Never silence `Watchdog`.

| Alert (source) | Severity, delay | Image rollback (3.2) | Restore rollback (3.3) |
| --- | --- | --- | --- |
| `CrawlerWorkerDown` (`rules-crawler.yaml`) | critical, 10m | yes, if the crawler stays at 0 for 10 minutes | yes: the crawler is at 0 throughout |
| `CrawlerMetricsMissing` (`rules-crawler.yaml`) | info, 1h | no | if the window lasts over an hour |
| `EconGraphEdge5xxRatioHigh` (`rules-edge.yaml`) | critical, 5m | yes, when there is traffic: 3.1 stops the backends | yes, when there is traffic |
| `EconGraphKeycloakDown` (`rules-edge.yaml`) | critical, 5m | no | yes, while Keycloak is at 0 |
| `EconGraphKeycloakInstanceDown` (`rules-edge.yaml`) | warning, 10m | only if a Keycloak pod hangs during the roll | yes, while one instance comes back after the other |
| `EconGraphPostgresInstanceNotReady` (`monitors.yaml`) | warning, 2m | no | yes, while a restored instance recovers or joins |
| `EconGraphBackupHealthCheckStale` (`monitors.yaml`) | critical, about 25m after the last good check | no | yes, if the window lasts over about 25 minutes: the check fails while a cluster is missing or not yet archiving |
| `KubeJobFailed` (chart default rules) | warning | no | likely, for the `cnpg-backup-health` Jobs that fail during the restore |
| `AnonymousGrant*` (`rules-auth.yaml`) | warning or critical | no | no: these have no series while the backends are down |
| `CloudflareTunnel*` (`rules-tunnel.yaml`) | critical | no | no while the tunnel and production marker aren't on the platform (**Gap G10**) |

Afterwards, expire the silence and check that `firing` shows only `Watchdog`:

```sh
amtool --alertmanager.url="$AM" silence expire "$SILENCE_ID"
firing
```

Anything else that fired is not expected: investigate it, and correct this table after
the rehearsal. **Gap G9**: no alert fires for a Flux Kustomization or HelmRelease that
isn't Ready, or for one left suspended. Verify step 1 checks them by hand.

## 4. Restore a CNPG cluster from the object store

Each plane has its own ObjectStore, which receives its base backups and WAL
([`objectstores.yaml`](../../platform/databases/objectstores.yaml),
[`schedules.yaml`](../../platform/databases/schedules.yaml)). The parts that differ per
plane:

| Plane | Cluster | Database | Owner, Secret | ObjectStore | Volume request |
| --- | --- | --- | --- | --- | --- |
| app | `econgraph-app` | `econ_graph_app` | `econgraph`, `econ-graph-app-credentials` | `app-backups` | 10Gi |
| data | `econgraph-data` | `econ_graph` | `econgraph`, `econ-graph-data-credentials` | `data-backups` | 100Gi |
| keycloak | `econgraph-keycloak` | `keycloak` | `keycloak`, `econ-graph-keycloak-credentials` | `keycloak-backups` | 10Gi |

**Gap G7**: the repo has no reviewed production restore path.
[`cnpg-drill.py backup-restore`](../../scripts/platform/cnpg-drill.py) proves PITR, but
only on a disposable, annotated copy ([4.14](#414-the-scratch-drill)). This procedure
restores **under the same cluster name**. That keeps the Services, the `*-rw` endpoints
in `econ-graph-secrets`, the ScheduledBackups, the backup health check and the alert
regexes working unchanged.

Because Flux owns the Cluster, the restore is a Git change on the deploy ref. The
cluster's entry in `platform/databases/clusters.yaml` switches from `initdb` to
`recovery`, and the restored cluster archives under a new `serverName`, so that it never
writes into the archive it restores from. A trial restore outside Flux proves the backup
before the live cluster is deleted. The rehearsal must show that CNPG and Flux accept all
of this.

### 4.0 When to use this section

- **As part of a 3.3 rollback**: 3.1 has already stopped the backends. Use
  `RESTORE_FOR_TAG=$PREV_TAG` and `RECOVERY_TARGET=name:pre-$RUN`.
- **On its own** (a lost or corrupted database, with the release unchanged): use
  `RESTORE_FOR_TAG=$TAG`, and one of these targets:
  - `RECOVERY_TARGET=latest`: the end of the archived WAL, for a cluster that is lost
    but whose data was sound.
  - `RECOVERY_TARGET=time:<RFC 3339 UTC time>`: a time before the damage.

  Read the run back as in [Session variables](#session-variables), or start a new one
  with the [1.0](#10-start-a-run) commands so that this restore has its own evidence.

### 4.1 Set the target and check the backups

```sh
export PLANE=app                                  # app | data | keycloak
export RESTORE_FOR_TAG="$TAG"                     # the release that runs afterwards: $PREV_TAG in a 3.3 rollback
export RESTORE_REF="deploy/$CLUSTER-$RESTORE_FOR_TAG"
export RECOVERY_TARGET="name:pre-$RUN"            # or latest, or time:2026-11-10T15:04:05Z
export SOURCE_SERVER="$(k -n econ-graph get cluster "econgraph-$PLANE" -o jsonpath='{.spec.plugins[0].parameters.serverName}')"
export SOURCE_SERVER="${SOURCE_SERVER:-econgraph-$PLANE}"   # empty means the cluster name
export NEW_SERVER="econgraph-$PLANE-r$(date -u +%Y%m%d%H%M)"
export BACKUP_ID=""
case "$RECOVERY_TARGET" in
  name:*) export BACKUP_ID="$(awk -v p="$PLANE" '$1==p{print $2}' "$EVIDENCE/backup-ids.txt")"
          [ -n "$BACKUP_ID" ] || echo "STOP: no backup id for $PLANE in $EVIDENCE/backup-ids.txt"
          grep -q "^$PLANE ${RECOVERY_TARGET#name:} " "$EVIDENCE/restore-points.txt" || echo "STOP: restore point not recorded" ;;
esac
echo "$PLANE $RECOVERY_TARGET backup=$BACKUP_ID $SOURCE_SERVER -> $NEW_SERVER ref=$RESTORE_REF" | tee -a "$EVIDENCE/restore.txt"
k -n econ-graph get backups -o json | jq -r --arg c "econgraph-$PLANE" \
  '.items[] | select(.spec.cluster.name==$c and .status.phase=="completed") | "\(.metadata.name) \(.status.backupId) \(.status.stoppedAt)"' |
  sort -k3 | tee "$EVIDENCE/backups-$PLANE.txt"
```

Check that the target has a base backup:

- `name:`: `BACKUP_ID` is set, and that id appears in `backups-$PLANE.txt`.
- `time:`: CNPG picks the newest backup that stopped before the target. This must print
  a backup:

  ```sh
  awk -v t="${RECOVERY_TARGET#time:}" '$3 < t' "$EVIDENCE/backups-$PLANE.txt" | tail -1
  ```

  The archive must also hold WAL after the target time. If nothing has been written to
  the database since then, recovery stops with "recovery ended before configured recovery
  target was reached".
- `latest`: `backups-$PLANE.txt` is not empty.

Backups taken before an earlier restore live under that earlier `serverName`. To restore
from one of them, set `SOURCE_SERVER` to the earlier name.

### 4.2 Check the space

The replaced cluster's volumes are kept (`econgraph-local` uses `Retain`). So the
restored cluster needs its full volume request again on each of the two workers. The
trial in 4.6 needs one more request on one worker, and keeps it after the trial is
deleted:

```sh
curl -s http://127.0.0.1:9090/api/v1/query \
  --data-urlencode 'query=node_filesystem_avail_bytes{mountpoint="/var/mnt/postgres"} / 2^30' |
  jq -r '.data.result[] | "\(.metric.instance) \(.value[1]) GiB free"' | tee -a "$EVIDENCE/restore.txt"
```

If each worker can't hold the plane's request (10Gi or 100Gi) plus WAL, stop. If one
worker can't also hold the trial, use the server dry run in 4.6 instead of the full
trial. Freeing space means deleting a retained volume. That is an owner decision, Joe's
([BACKUPS.md](BACKUPS.md), "Nothing is deleted automatically").

### 4.3 Silence, stop writes, stop Flux

Silence the alerts as [3.4](#34-alerts-expected-during-a-rollback) describes, then:

```sh
for ks in platform-application platform-data-schema platform-databases; do suspend_ks "$ks"; done
k -n econ-graph scale deployment/econ-graph-backend deployment/econ-graph-data-backend \
  deployment/crawler-worker deployment/keycloak --replicas=0
k -n econ-graph wait --for=delete pod -l 'app in (econ-graph-backend,econ-graph-data-backend,crawler-worker,keycloak)' --timeout=10m
```

This is a full outage. It is the safe default for any of the three planes.

### 4.4 Record the cluster being replaced

```sh
k -n econ-graph get cluster "econgraph-$PLANE" -o yaml > "$EVIDENCE/replaced-cluster-$PLANE.yaml"
k -n econ-graph get pvc -l "cnpg.io/cluster=econgraph-$PLANE" \
  -o custom-columns='PVC:.metadata.name,PV:.spec.volumeName,NODE:.metadata.annotations.volume\.kubernetes\.io/selected-node' |
  tee "$EVIDENCE/replaced-volumes-$PLANE.txt"
```

### 4.5 Commit the recovery spec

This edits only the plane's entry in `clusters.yaml`, as text. It replaces the
`bootstrap:` block (with any earlier `externalClusters:`) up to `postgresql:`, and sets
`serverName` under the WAL archiver's `barmanObjectName`. The rest of the file is left
byte for byte.

```sh
git fetch origin
git switch -C "$RESTORE_REF" "origin/$RESTORE_REF"
python3 - "$PLANE" "$RECOVERY_TARGET" "$BACKUP_ID" "$SOURCE_SERVER" "$NEW_SERVER" <<'PY'
import re, sys, yaml
from pathlib import Path
plane, target, backup_id, source, new = sys.argv[1:]
db, owner, secret = {'app': ('econ_graph_app', 'econgraph', 'econ-graph-app-credentials'),
                     'data': ('econ_graph', 'econgraph', 'econ-graph-data-credentials'),
                     'keycloak': ('keycloak', 'keycloak', 'econ-graph-keycloak-credentials')}[plane]
if target.startswith('name:'):
    goal = '      recoveryTarget:\n        backupID: %s\n        targetName: %s\n' % (backup_id, target[5:])
elif target.startswith('time:'):
    goal = "      recoveryTarget:\n        targetTime: '%s'\n" % target[5:]
elif target == 'latest':
    goal = ''
else:
    sys.exit('STOP: RECOVERY_TARGET must be name:..., time:... or latest')
path = Path('platform/databases/clusters.yaml')
parts = path.read_text().split('\n---\n')
[i] = [n for n, p in enumerate(parts) if re.search(r'(?m)^  name: econgraph-%s$' % plane, p)]
part = parts[i]
start, end = part.index('\n  bootstrap:\n') + 1, part.index('\n  postgresql:\n') + 1
block = ('  bootstrap:\n    recovery:\n      source: origin\n      database: %s\n      owner: %s\n'
         '      secret:\n        name: %s\n%s'
         '  externalClusters:\n  - name: origin\n    plugin:\n      name: barman-cloud.cloudnative-pg.io\n'
         '      parameters:\n        barmanObjectName: %s-backups\n        serverName: %s\n'
         % (db, owner, secret, goal, plane, source))
part = part[:start] + block + part[end:]
part, count = re.subn(r'(?m)^(      barmanObjectName: %s-backups\n)(      serverName: .*\n)?' % plane,
                      r'\g<1>      serverName: %s\n' % new, part)
if count != 1:
    sys.exit('STOP: WAL archiver entry not found exactly once')
parts[i] = part
text = '\n---\n'.join(parts)
doc = [d for d in yaml.safe_load_all(text) if d and d['metadata']['name'] == 'econgraph-' + plane][0]
assert list(doc['spec']['bootstrap']) == ['recovery'] and doc['spec']['plugins'][0]['parameters']['serverName'] == new
path.write_text(text)
print('recovery spec written')
PY
git diff platform/databases/clusters.yaml | tee "$EVIDENCE/recovery-spec-$PLANE.diff"
git commit -am "chore(deploy): restore econgraph-$PLANE to $RECOVERY_TARGET"
git push origin "$RESTORE_REF"
git fetch origin
export RESTORE_COMMIT="$(git rev-parse "origin/$RESTORE_REF")"
[ "$(git rev-parse HEAD)" = "$RESTORE_COMMIT" ] || echo "STOP: local $RESTORE_REF differs from origin"
echo "$RESTORE_COMMIT" > "$SETTINGS_DIR/deploy-commit-$RESTORE_FOR_TAG"
```

The last line points that release's `deploy-commit` file at the commit with the recovery
spec, so that a later rollback to that release keeps it. The diff must change only the
`econgraph-$PLANE` entry.

### 4.6 Prove the restore on a trial cluster

Before the live cluster is deleted, run the same recovery into a throwaway Cluster
outside Flux. It has one instance and no WAL archiver, so it never writes to an archive.
The namespace's default-deny ingress policy would block the CNPG operator, so the trial
gets its own NetworkPolicy.

```sh
python3 - "$PLANE" > "$EVIDENCE/trial-$PLANE.json" <<'PY'
import json, sys, yaml
plane = sys.argv[1]
docs = [d for d in yaml.safe_load_all(open('platform/databases/clusters.yaml')) if d]
doc = [d for d in docs if d['metadata']['name'] == 'econgraph-' + plane][0]
doc['metadata'] = {'name': 'econgraph-%s-trial' % plane, 'namespace': 'econ-graph',
                   'labels': {'econgraph.io/restore-trial': plane}}
doc['spec']['instances'] = 1
doc['spec'].pop('plugins', None)
json.dump(doc, sys.stdout)
PY
k apply --dry-run=server -f "$EVIDENCE/trial-$PLANE.json"
```

The server dry run is the minimum check: the CNPG webhook accepts the recovery spec. If
4.2 showed room for the trial, run it:

```sh
k apply -f - <<EOF
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: restore-trial-$PLANE
  namespace: econ-graph
spec:
  podSelector:
    matchLabels:
      cnpg.io/cluster: econgraph-$PLANE-trial
  policyTypes:
  - Ingress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          kubernetes.io/metadata.name: cnpg-system
    ports:
    - port: 8000
      protocol: TCP
    - port: 5432
      protocol: TCP
EOF
k apply -f "$EVIDENCE/trial-$PLANE.json"
wait_cluster "econgraph-$PLANE-trial" 1 4
echo "trial $PLANE $(plane_version "$PLANE" "econgraph-$PLANE-trial")" | tee "$EVIDENCE/trial-version-$PLANE.txt"
k -n econ-graph get pvc -l "cnpg.io/cluster=econgraph-$PLANE-trial" -o custom-columns='PVC:.metadata.name,PV:.spec.volumeName' | tee -a "$EVIDENCE/trial-version-$PLANE.txt"
k -n econ-graph delete cluster "econgraph-$PLANE-trial" --wait=true --timeout=10m
k -n econ-graph delete networkpolicy "restore-trial-$PLANE"
```

In a 3.3 rollback, the trial's version must equal the plane's line in
`before-versions.txt`:

```sh
[ "$(grep "^$PLANE " "$EVIDENCE/before-versions.txt" | cut -d' ' -f2-)" = "$(head -1 "$EVIDENCE/trial-version-$PLANE.txt" | cut -d' ' -f3-)" ] && echo MATCH || echo "STOP: trial restored a different schema"
```

If the trial doesn't become ready, or shows the wrong schema, the live cluster is still
intact. Fix the target (4.1) and commit again (4.5) before going on. The trial's volume is
left `Released` on its worker. Recording it is enough; removing it is Joe's decision.

### 4.7 Replace the cluster

```sh
k -n econ-graph delete cluster "econgraph-$PLANE" --wait=true --timeout=10m
k get pv | grep "econ-graph/.*econgraph-$PLANE-" | tee -a "$EVIDENCE/replaced-volumes-$PLANE.txt"   # Released, not gone
pin_ref "$RESTORE_COMMIT"
resume_ks platform-databases
wait_cluster "econgraph-$PLANE" 2 6
flux_wait platform-databases "$RESTORE_COMMIT" 30
```

Deleting the Cluster deletes its PVCs. The PVs stay `Released` with their data. Allow
time in proportion to size: the data plane restores up to 100Gi from the object store.
`platform-databases` has a 15-minute health check timeout, so during a long restore it
shows `Ready=False` (`HealthCheckFailed`) and retries. That is expected while
`wait_cluster` is still waiting. `flux_wait` doesn't treat it as a failure.

### 4.8 If the restore fails

If the cluster doesn't become ready in 4.7, or 4.9 shows the wrong schema, **stay down
and restore again**. Keep everything suspended and scaled down. Don't resume
`platform-application`. Joe decides which target to try next. Then:

```sh
k -n econ-graph get cluster "econgraph-$PLANE" -o yaml > "$EVIDENCE/failed-cluster-$PLANE-$(date -u +%H%M%S).yaml"
k -n econ-graph logs -l "cnpg.io/cluster=econgraph-$PLANE" --all-containers --tail=200 > "$EVIDENCE/failed-restore-$PLANE.log"
suspend_ks platform-databases
k -n econ-graph delete cluster "econgraph-$PLANE" --wait=true --timeout=10m
```

Then go back to 4.1. Choose an earlier backup or target, set a new `NEW_SERVER`, and
repeat 4.5 to 4.7, with the trial.

Re-attaching the replaced cluster's `Released` volumes (clearing each PV's `claimRef` and
recreating PVCs that CNPG adopts) is not written here. It has never been tested, and a
mistake there loses the last copy. It is a last resort, for Joe to decide, and part of
Gap G7.

### 4.9 Check the restored cluster

```sh
k -n econ-graph get cluster "econgraph-$PLANE" -o jsonpath='{.status.phase} {.status.readyInstances} {.status.conditions[?(@.type=="ContinuousArchiving")].status}{"\n"}' | tee -a "$EVIDENCE/restore.txt"
echo "$PLANE $(plane_version "$PLANE" "econgraph-$PLANE")" | tee -a "$EVIDENCE/restore.txt"
```

The phase is `Cluster in healthy state` with 2 instances, and `ContinuousArchiving` is
`True`. In a 3.3 rollback, the version line must equal the plane's line in
`before-versions.txt`.

### 4.10 Take a first backup under the new server name

PITR from the new archive prefix needs a base backup there:

```sh
export FIRST="econgraph-$PLANE-restored-$(date -u +%Y%m%d%H%M)"
k apply -f - <<EOF
apiVersion: postgresql.cnpg.io/v1
kind: Backup
metadata:
  name: $FIRST
  namespace: econ-graph
spec:
  cluster:
    name: econgraph-$PLANE
  method: plugin
  pluginConfiguration:
    name: barman-cloud.cloudnative-pg.io
EOF
wait_backup "$FIRST" && echo "$PLANE $FIRST $(k -n econ-graph get backup "$FIRST" -o jsonpath='{.status.backupId}')" | tee -a "$EVIDENCE/restore.txt"
```

### 4.11 Restore the other planes

For another plane, set `PLANE` and `NEW_SERVER` again (4.1) and repeat 4.1, 4.2 and 4.4 to
4.10. Keep everything suspended and scaled down until every plane is restored. Each
restore commit goes on top of the previous one on `$RESTORE_REF`. Before 4.7, suspend
`platform-databases` again with `suspend_ks platform-databases`.

### 4.12 Restart the application

In a 3.3 rollback, apply the previous settings file first (3.3, step 2). Then:

```sh
for ks in platform-data-schema platform-application; do resume_ks "$ks"; done
flux_wait platform-data-schema "$RESTORE_COMMIT" 30
wait_job crawler-migrate 30
flux_wait platform-application "$RESTORE_COMMIT" 30
for d in econ-graph-backend econ-graph-data-backend econ-graph-frontend keycloak crawler-worker; do
  k -n econ-graph rollout status "deployment/$d" --timeout=15m
done
```

Resuming `platform-application` restores the replica counts from Git. Then run all of
[Verify](#2-verify), expire the silence (3.4), and record the outage window in
`$EVIDENCE/restore.txt`.

### 4.13 Carry the new server name to later releases

The next release's deploy ref is built from a later tag, whose `clusters.yaml` has
`initdb` and the default `serverName`. When Flux applies it to the restored cluster, the
archive goes back to the old prefix and archiving fails. Don't put the recovery spec on
`main`. A `bootstrap.recovery` on `main` would make every fresh cluster built from
`main` try to restore. Carry only the `serverName`, as a per-environment setting:

1. A PR to `main` changes each WAL archiver entry in `clusters.yaml` to
   `serverName: ${APP_ARCHIVE_SERVER:=econgraph-app}`,
   `${DATA_ARCHIVE_SERVER:=econgraph-data}` and
   `${KEYCLOAK_ARCHIVE_SERVER:=econgraph-keycloak}`. Flux substitutes these with the
   defaults shown, so fresh and unrestored clusters keep their names. Merge it before the
   next promotion on this cluster.
2. Add the restored plane's key to the settings files of every later release on this
   cluster, for example `APP_ARCHIVE_SERVER: econgraph-app-r202611101504` (the
   `NEW_SERVER` from 4.1).

Applying a later release also switches the restored Cluster's `bootstrap` back from
`recovery` to `initdb`. CNPG ignores `bootstrap` after creation, but whether its webhook
accepts that change hasn't been checked. The rehearsal checks it (Gap G7).

### 4.14 The scratch drill

The platform's own PITR gate runs on a disposable copy, not on the `econ-graph` clusters.
It needs the explicit `econgraph.io/disposable-drill` annotations that
[`platform/README.md`](../../platform/README.md) describes, in "Backups, copy migration and
acceptance":

```sh
python3 scripts/platform/cnpg-drill.py backup-restore --kubeconfig "$KCFG" --context "$CTX" \
  --namespace train4-proof --owner "$TRAIN4_OWNER" --source-cluster owned-copy \
  --object-store owned-backups --kubectl "$KUBECTL" --report "$EVIDENCE/cnpg-pitr.json" --execute
```

Without `--execute` it is a dry run. It proves base backup, WAL archiving and PITR to a
time. Steps 4.1 to 4.13 prove the procedure production would use. The rehearsal runs both.

## 5. Cutover from kind

The relaunch is one cutover (ECO-859). This is the order, and this runbook's part in each
step:

1. **Preconditions.** All of these must be recorded:
   - the platform's external release gates ([`platform/README.md`](../../platform/README.md),
     the bold list in "Backups, copy migration and acceptance") and the Talos fit gate
     ([`infra/talos/README.md`](../../infra/talos/README.md));
   - ECO-864: production bootstrap and reconcile, the tunnel, and release images for every
     workload, which closes Gaps G1, G2, G4, G5, G8 and G10;
   - ECO-438 and ECO-426: moving the kind databases into the three CNPG clusters;
   - the ECO-883 decision.
2. **Rehearse** the whole path on a fresh scratch Talos cluster, on a copy of the data,
   with [section 7](#7-rehearsal-checklist) as the checklist (ECO-859 step 1).
3. **Approval**: Joe approves the rehearsed runbook (ECO-859 step 2).
4. **Freeze and export kind.** The site is offline, so kind serves nobody. Take the final
   dumps with `scripts/deploy/backup-kind-databases.sh --dry-run`, then without
   `--dry-run`, and keep that backup directory. **Gap G13**: `cnpg-drill.py migrate-copy` is
   scratch-only. It takes a per-database pg_dump archive and a JSON manifest of table
   counts, but `backup-kind-databases.sh` writes `counts.env` inside an encrypted tarball.
   The production copy procedure belongs to ECO-438 and ECO-426.
5. **Load production.** Bootstrap the production Talos cluster (ECO-864) and load the data
   (ECO-438). Then promote the release candidate with section 1, as its first release
   (`PREV_TAG` empty), and verify with section 2. The tunnel stays off.
6. **Catalog roles, tag, tunnel, QA, www, reboot test, refetch**: ECO-859 steps 3 to 8. Tag
   `v4.2.0` on the same digests: 1.1 must give the same three digests for the final tag.
   Then turn the tunnel on. **Gap G14**: two scripts these steps need aren't ported.
   `switch-main-host.sh` (step 6) changes Keycloak and the backend with `kubectl set env`,
   which Flux reverts. On Talos the main host is `PUBLIC_HOST` and `PUBLIC_ORIGIN` in the
   settings file, applied as in 1.7. `refetch-full-history.sh` (step 8) runs only against
   kind.

### Fall-back point

Until the Talos tunnel is on, kind and its data are untouched and nothing public has run
on Talos. Falling back means leaving the tunnel off and not carrying on. After the tunnel
is on, Talos takes writes that kind doesn't have. Going back then means copying those
writes back or losing them. No tooling for that exists.

Which fallback applies, and what triggers it, is open: it is **ECO-883's decision (Joe)**.
The candidates named there are v4.2.0 on kind with catalog roles and monitoring ported,
or Talos without the Iceberg cutover. This runbook doesn't choose. Once ECO-883 is
decided, write the chosen fallback here as steps.

### Retiring kind

Kind and its data stay intact until the gates in step 1 pass and retirement is explicitly
authorised ([`platform/README.md`](../../platform/README.md), last paragraph of "Backups,
copy migration and acceptance"). How long kind and its data are kept after cutover is
ECO-883's question 4. Retiring kind means following the table in section 6. Deleting
kind's volumes, snapshots or dumps is a separate owner decision; nothing here deletes them.

## 6. Kind scripts and documents

These are recommendations only; this runbook deletes nothing. The recommendations mean:

- **Ported**: the Talos procedure is in this runbook or in
  [`platform/README.md`](../../platform/README.md). Delete the kind original once kind is
  retired. If ECO-883 picks the kind fallback, every ported kind item stays until that
  fallback is no longer needed.
- **Release lab**: kept while the release lab runs on kind. ECO-860 and ECO-862 decide what
  replaces it.
- **Not ported**: a gap.

### Documents and commands

| Item | Recommendation | Where it went, or why |
| --- | --- | --- |
| [docs/development/RELEASE_PROCESS.md](../development/RELEASE_PROCESS.md) | Keep | Branches, QA and tagging don't depend on the cluster; it links here |
| [RELEASE_DEPLOY.md](RELEASE_DEPLOY.md) | Ported, then delete | Sections 1 to 3 |
| [RELEASE_IMAGES.md](RELEASE_IMAGES.md) | Ported, then delete | Digest resolution in 1.1, image check in Verify step 2. Nodes pull by digest, so loading images and per-node rollback have no Talos equivalent |
| [RELEASE_LAB.md](RELEASE_LAB.md) | Release lab | ECO-860 says which parts still apply |
| [SANITIZED_FIXTURES.md](SANITIZED_FIXTURES.md) | Release lab | The lab's and CI's data |
| [APP_DATA_CONVERSION.md](APP_DATA_CONVERSION.md) | Release lab, and the cutover | ECO-426 |
| [POSTGRES_BACKUP_RESTORE.md](POSTGRES_BACKUP_RESTORE.md) | Ported, then delete; keep "Restore encrypted archives" while kind-era dumps are kept | CNPG backups and section 4 |
| [BACKUPS.md](BACKUPS.md) | Ported in part | CNPG object-store backups replace the nightly dumps. The host object store's own protection and off-site copies stay (ECO-835, ECO-497). Its restore drill is still how to open kind-era archives |
| [k8s/README.md](../../k8s/README.md) | Ported, then delete; keep "Disposable branch environments" until ECO-862 | [`platform/README.md`](../../platform/README.md) and this runbook |
| [KUBERNETES_DEPLOYMENT.md](KUBERNETES_DEPLOYMENT.md) | Ported, then delete | The kind deployment guide |
| [HOST_STORAGE.md](HOST_STORAGE.md) | Delete after retirement | Kind's static host-subvolume PVs; Talos uses `econgraph-local` |
| [GATEWAY_ROUTING_TROUBLESHOOTING.md](GATEWAY_ROUTING_TROUBLESHOOTING.md) | Not ported | Envoy Gateway on kind; the Talos gateway (`platform/network`) needs its own version |
| [LETSENCRYPT_CLOUDFLARE_DNS01_INTEGRATION.md](LETSENCRYPT_CLOUDFLARE_DNS01_INTEGRATION.md), [CLUSTER_DNS_TROUBLESHOOTING.md](CLUSTER_DNS_TROUBLESHOOTING.md) | Delete after retirement, unless ECO-864 adopts DNS-01 on the platform | The optional kind certificate path |
| [TUNNEL_ORIGIN_TLS.md](TUNNEL_ORIGIN_TLS.md) | Not ported | ECO-864 decides the tunnel on the platform |
| [MAIN_HOST.md](MAIN_HOST.md) | Not ported | Gap G14 |
| [AGENT_ACCESS.md](AGENT_ACCESS.md) | Not ported | Talos has no read-only agent identity yet |
| [CHART_API_SERVICE.md](CHART_API_SERVICE.md) | Keep | Describes the service, not kind; the platform runs no chart-api (Gap G5) |
| [HOST_DISK_MONITORING.md](HOST_DISK_MONITORING.md), [OBJECTSTORE_HOST.md](OBJECTSTORE_HOST.md), [OBJECTSTORE_SMOKE.md](OBJECTSTORE_SMOKE.md) | Keep | They describe the platform's host and its object-store gate |
| [MICROK8S_DEPLOYMENT.md](MICROK8S_DEPLOYMENT.md), [MICROK8S_TROUBLESHOOTING.md](MICROK8S_TROUBLESHOOTING.md), [K8S_DEPLOYMENT_GUIDE.md](K8S_DEPLOYMENT_GUIDE.md), [DEPLOYMENT_RESTART.md](DEPLOYMENT_RESTART.md), [K8S_RESTART_COMMANDS.md](K8S_RESTART_COMMANDS.md) | Delete | MicroK8s material and `restart-k8s-rollout.sh`: ECO-898 owns them |
| `.cursor/commands/deploy.md` | Delete after retirement | Runs kind's `deploy.sh`; promotion is section 1 |
| `.cursor/commands/k8s-deploy.md` | Delete after retirement | Applies `k8s/manifests` by hand, which Flux would revert or fight |
| `.cursor/commands/k8s-restart.md` | Keep | `rollout restart` works under Flux |
| `.cursor/commands/k8s-context.md`, `k8s-logs.md`, `debug-k8s.md` | Keep | They use the current context; point it at the Talos cluster |

### `scripts/deploy/`

Each script's `test-*` file goes with it.

| Script | Recommendation | Where it went, or why |
| --- | --- | --- |
| `release-deploy.sh` | Ported, then delete | Sections 1 to 3 |
| `backup-postgres-now.sh` | Ported, then delete | On-demand `Backup` in 1.4 |
| `load-release-images.sh`, `verify-release-images.sh`, `rollback-release-images.sh`, `lib/release-images.sh` | Ported, then delete | 1.1, Verify step 2, 3.2 |
| `precheck-db-migration.sh` | Ported, then delete | 1.5 |
| `deploy.sh`, `lib/deploy-config.sh`, `lib/app-config.sh`, `lib/data-migrations.sh` | Ported, then delete | Flux applies; 1.8 orders the migration |
| `backup-keycloak-db.sh`, `restore-keycloak-db.sh`, `lib/keycloak-db.sh` | Ported, then delete | The keycloak plane in 1.4 and section 4 |
| `restore-postgres-backup.sh` | Ported, then delete | Section 4 |
| `create-secrets.sh`, `set-crawler-keys.sh`, `lib/keycloak-secret.sh` | Ported, then delete | SOPS inputs ([`platform/README.md`](../../platform/README.md), "Secret inputs") and 1.2 |
| `install-gateway.sh`, `apply-gateway-routes.sh`, `cutover-to-gateway.sh`, `render-public-host.py`, `lib/public-host.sh` | Ported, then delete | `platform/network` and the application's HTTPRoutes |
| `apply-host-storage.sh`, `migrate-to-host-storage.sh`, `protect-postgres-pv.sh`, `released-pvs.sh` | Delete after retirement | `econgraph-local` with `Retain` |
| `recreate-kind-cluster.sh`, `teardown.sh`, `setup-port-forwarding.sh`, `setup-letsencrypt-cloudflare.sh`, `init-database.sh` | Delete after retirement | Kind only |
| `install-kind.sh`, `load-kind-images.sh`, `lib/kind-env.sh`, `lib/kind-version.sh`, `lib/kind_tools.py` | Release lab | The lab is a kind cluster |
| `release-lab.sh`, `release-lab.py`, `lib/keycloak_sanitize.py`, `backup-kind-databases.sh` | Release lab | `keycloak_sanitize.py` also serves `scripts/qa/deploy-rc-to-qa.sh`. `backup-kind-databases.sh` is also the cutover export (section 5, step 4) |
| `restore-kind-databases.sh` | Keep until kind is retired | The kind side of any ECO-883 fallback |
| `convert-app-data.sh`, `split-app-data.py`, `lib/app-database-guard.sh` | Release lab, and the cutover | ECO-426 |
| `measure-v41-first-deploy.sh`, `measure-v41-first-deploy.py` | Delete | v4.1.0 is orphaned (ECO-860) |
| `mark-production-cluster.sh`, `lib/cluster-role.sh` | Not ported | The Talos cluster has no production marker; Gap G10 (ECO-864) |
| `stage-tunnel-origin-ca.sh` | Not ported | ECO-864 decides the tunnel on the platform |
| `switch-main-host.sh` | Not ported | Gap G14 |
| `check-public-site.sh` | Keep; `--via-cloudflare` only | Gap G11 |
| `refetch-full-history.sh`, `check-selection.sh` | Not ported | They use `lib/kind-env.sh`; ECO-859 step 8 needs the refetch on Talos (Gap G14) |
| `create-agent-kubeconfig.sh`, `run-agent-session.sh`, `lib/agent-access.sh`, `lib/agent_paths.py` | Not ported | Talos has no read-only agent identity yet |
| `deploy-branch.sh`, `deploy-branch.py`, `render-branch.py`, `branch-network-check.py` | Keep | Branch environments; ECO-862 moves them to the new cluster |
| `build-images.sh` | Keep | Local development builds |
| `restart-k8s-rollout.sh` | Delete | ECO-898 owns this |

## 7. Rehearsal checklist

The rehearsal is Joe's, or an authorised operator's, on a fresh scratch Talos cluster.
Keep all output under `$EVIDENCE_ROOT`. It stays private until sanitized, because reports
can contain hostnames and connection details. When the rehearsal is done, post a
sanitized summary as a comment on ECO-887. Add the artifacts to the Talos fit evidence
file ([`fit-evidence.example.json`](../../infra/talos/fit-evidence.example.json):
`fresh-cluster-restore`, release-E2E during drain).

- [ ] Session set up as in [Before you start](#before-you-start). `run.txt` records the
      cluster's `kube-system` UID and the operator.
- [ ] Gap G4 worked around for the rehearsal, with how it was done recorded. Without it,
      no release image pulls.
- [ ] **Promote** release A as the first release (`PREV_TAG` empty), then release B, with
      section 1. Pick a B whose data or app migrations change the schema. Record
      `digests.txt`, `refs.txt`, `platform-diff.txt`, the settings files,
      `backup-ids.txt`, `restore-points.txt`, `crawler-migrate.log` and `promoted-at.txt`.
- [ ] `platform-application` stayed suspended across 1.7 (the `true` check).
- [ ] **Promote B again** with a new run (1.0). Each run's Backup names and restore points
      are distinct, and 1.2 reused the deploy ref without a new commit.
- [ ] **Abandon after 1.5** once, using the undo there. `crawler-worker` is back at 1.
- [ ] **Verify** B with section 2. Record `flux.txt`, `images.txt`, `after-versions.txt`,
      `clusters.txt`, `backup-health.txt`, `alerts-after.txt`, `qa.json`, and dashboard
      screenshots around the promotion time.
- [ ] **Roll back before a migration**: promote B, stop after 1.6, then run 3.1 and 3.2.
      Record that the versions are unchanged, and Verify against A.
- [ ] **Roll back after a migration**: promote B fully, then run 3.1 and 3.3 with app and
      data together. Record each `restore.txt`, `trial-version-*`,
      `recovery-spec-*.diff`, `replaced-*` file, the outage window, and the versions
      matching `before-versions.txt`.
- [ ] The trial (4.6) ran in full at least once, and its server dry run was accepted.
- [ ] Record the alerts that fired during each rollback (`firing` every few minutes) and
      compare them with the 3.4 table. Correct the table with what actually fired.
- [ ] **Restore on its own**: the keycloak plane to `time:` (4.0). Check that sign-in
      works afterwards.
- [ ] CNPG accepted the recovery spec under an existing cluster name, and Flux re-applied
      it without errors (4.7). If either refused, record the error: Gap G7 then needs a
      different design.
- [ ] **Promote a later release after the restore**, with the 4.13 `serverName` setting.
      Record whether CNPG's webhook accepts `bootstrap` switching from `recovery` back to
      `initdb`, and that `ContinuousArchiving` stays `True` (Gap G7).
- [ ] Force a failed restore once (for example a `targetTime` before every backup), and
      follow 4.8.
- [ ] Check G15: after 24 hours, did Flux recreate and rerun `crawler-migrate` and
      `keycloak-realm-import`?
- [ ] The scratch drill (4.14) passed. Record `cnpg-pitr.json`.
- [ ] Time each step. The data plane's restore time is the outage a 3.3 rollback costs;
      use it to size the 3.4 silence.
- [ ] List every place where you had to ask, guess, or type a command this runbook
      doesn't contain. Each one is a fix to this runbook before production.

## Gaps

Where the repo has no mechanism. Each one needs a ticket, or a note on the ticket named.

| Id | Gap | Ticket |
| --- | --- | --- |
| G1 | `bootstrap.py` cni, flux and reconcile accept scratch clusters only (`scripts/platform/bootstrap.py` lines 60-63, 93); production has only the unreviewed manual equivalent in 1.7 and 3.2 | ECO-864 |
| G2 | Flux can't track a release tag as is. `--git-ref` must be an `origin` branch, and the encrypted inputs must be committed at the tracked commit, so a deploy ref (the tag plus a ciphertext commit) is needed | ECO-864, ECO-235 |
| G3 | Image digests live in an out-of-Git ConfigMap; there is no promotion PR or audit trail | ECO-235 |
| G4 | Nodes have no credential to pull the private GHCR packages | none yet (ECO-864's acceptance needs it) |
| G5 | No chart-api workload on the platform; `CHART_API_SERVICE_URL` points at a missing Service | ECO-864 |
| G6 | Backends migrate at start during a rolling update; no expand/contract rule | ECO-309 |
| G7 | No reviewed production restore under Flux. `cnpg-drill.py` is scratch-only. A restored cluster's `serverName` has to be carried to later releases (4.13), and it hasn't been checked whether CNPG accepts `bootstrap` switching from `recovery` back to `initdb`. No tested way to re-attach a replaced cluster's volumes | none yet |
| G8 | No production `ENVIRONMENT` or bucket names for the ObjectStores | ECO-864 |
| G9 | No alerts for Flux objects that aren't Ready or are left suspended | none yet |
| G10 | No production marker or tunnel on the platform, so the `CloudflareTunnel*` alerts are inert | ECO-864 |
| G11 | No HTTPS public-site check before the tunnel; the Talos gateway is HTTP-only | none yet |
| G12 | No documented way to give the platform scripts a decrypted kubeconfig | none yet |
| G13 | No production kind-to-CNPG copy; `migrate-copy`'s input doesn't match `backup-kind-databases.sh`'s output | ECO-438, ECO-426 |
| G14 | `switch-main-host.sh`, `refetch-full-history.sh` and `check-selection.sh` are kind-only, and ECO-859 steps 6 and 8 need the first two | ECO-859 |
| G15 | `crawler-migrate` and `keycloak-realm-import` have `ttlSecondsAfterFinished: 86400`, so Flux probably recreates and reruns them after each TTL (confirm in the rehearsal). `platform-application` has no `force`, so a changed `keycloak-realm-import` spec fails with `field is immutable` while the old Job exists; 1.8 deletes it by hand. `force` or the `kustomize.toolkit.fluxcd.io/force: enabled` annotation on that Job would fix it | none yet |
