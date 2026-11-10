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

Every command below runs from the repository root of a checkout of the ref being
promoted, in the session that [Before you start](#before-you-start) sets up. Run it
inside tmux or screen on the operator host, so that a dropped ssh session doesn't
interrupt a step.

## Before you start

### Tools

- `kubectl` 1.36.4 and `helm` 4.3.1, from the URLs and SHA-256 values in
  [`platform/toolchain.lock.json`](../../platform/toolchain.lock.json).
  [`scripts/platform/safety.py`](../../scripts/platform/safety.py) refuses a kubectl more than
  one minor version away from the target.
- `amtool` 0.34.2, also from that file.
- `sops`, with the age identity that decrypts the cluster's exported kubeconfig
  ([`infra/talos/README.md`](../../infra/talos/README.md), "Protect secrets and state").
- `docker` with `buildx`, `jq`, `git`, `python3` with `pip install -r scripts/platform/requirements.txt`.
- `docker login ghcr.io` with a token that has `read:packages`. The release packages are
  private ([Release images](RELEASE_IMAGES.md)).

No Flux CLI and no CNPG kubectl plugin are pinned, so this runbook uses plain kubectl
for Flux and CNPG objects.

### Session variables

Set these once per session. The values in angle brackets are yours to fill in.

```sh
export KUBECTL=/path/to/kubectl-1.36.4          # the pinned client
export STATE=/path/outside/git/<cluster>        # cluster.py --state-dir of this Talos cluster
export TAG=vX.Y.Z-rc.N                          # the release ref being promoted
export PREV_TAG=<tag running now>               # empty on the first promotion
export SLUG="$(printf %s "$TAG" | tr '.' '-')"  # vX-Y-Z-rc-N: a valid Kubernetes name part
export SETTINGS_DIR=/private/<cluster>/settings  # protected storage: one settings file per release
export SECRETS_DIR=/private/<cluster>/secrets    # protected storage: the *.sops.yaml ciphertext
export EVIDENCE="/private/<cluster>/evidence/$TAG"  # private evidence for this run
export DEPLOY_REF="deploy/<cluster>-$TAG"       # see 1.3
mkdir -p "$EVIDENCE"
```

On a scratch cluster also set the values that [`platform/README.md`](../../platform/README.md)
names:

```sh
export TRAIN4_OWNER=<owner_id UUID from the scratch Talos inventory>
export CONTROL_NODE=<a control-plane node name, e.g. econgraph-scratch-cp-1>
export CACHE=/tmp/train4-platform-cache          # populated by validate.py --fetch
```

### Kubeconfig

The Talos tooling exports the kubeconfig only as SOPS ciphertext
(`$STATE/kubeconfig.sops.yaml`). Open a shell that has it decrypted to a temporary file
for that shell's lifetime; every step below runs inside it:

```sh
sops exec-file --no-fifo "$STATE/kubeconfig.sops.yaml" 'KCFG={} bash'
```

Inside that shell:

```sh
export CTX="$("$KUBECTL" --kubeconfig "$KCFG" config current-context)"
k() { "$KUBECTL" --kubeconfig "$KCFG" --context "$CTX" "$@"; }
k get nodes -o wide                          # the inventory's nodes, all Ready
k get namespace kube-system -o jsonpath='{.metadata.uid}{"\n"}'   # record it: this cluster's identity
```

`--no-fifo` is needed because the platform scripts run kubectl many times. The file is
removed when the shell exits. **Gap G12**: the repo has no documented way to hand a
decrypted kubeconfig to the platform scripts; this is an operator convention.

### Database helpers

The CNPG clusters are `econgraph-app` (database `econ_graph_app`), `econgraph-data`
(`econ_graph`) and `econgraph-keycloak` (`keycloak`), all in namespace `econ-graph`
([`platform/databases/clusters.yaml`](../../platform/databases/clusters.yaml)). These
helpers run psql as `postgres` inside the current primary, as
[`scripts/platform/cnpg-drill.py`](../../scripts/platform/cnpg-drill.py) does:

```sh
primary() { k -n econ-graph get cluster "econgraph-$1" -o jsonpath='{.status.currentPrimary}'; }
psql_on() {  # psql_on app|data|keycloak DATABASE SQL
  k -n econ-graph exec -i "$(primary "$1")" -c postgres -- \
    psql -X -v ON_ERROR_STOP=1 -U postgres -d "$2" -qAt -c "$3"
}
flux_wait() {  # flux_wait KUSTOMIZATION COMMIT: until it has applied that commit and is Ready
  until k -n flux-system get kustomization "$1" -o jsonpath='{.status.lastAppliedRevision}' | grep -q "$2" &&
        [ "$(k -n flux-system get kustomization "$1" -o jsonpath='{.status.conditions[?(@.type=="Ready")].status}')" = True ]; do
    sleep 15
  done
}
suspend_ks() { k -n flux-system patch kustomization "$1" --type merge -p '{"spec":{"suspend":true}}'; }
resume_ks()  { k -n flux-system patch kustomization "$1" --type merge -p '{"spec":{"suspend":false}}'
               k -n flux-system annotate --overwrite kustomization "$1" reconcile.fluxcd.io/requestedAt="$(date -u +%s)"; }
```

`flux_wait` has no timeout of its own. If it hasn't returned within the time the step
gives, press Ctrl-C and read the Kustomization's status with
`k -n flux-system describe kustomization NAME`.

### Monitoring access

```sh
k -n monitoring get svc   # names below assume Helm release "monitoring" (platform/addons/monitoring/release.yaml)
k -n monitoring port-forward svc/monitoring-kube-prometheus-alertmanager 9093:9093 >/dev/null &
k -n monitoring port-forward svc/monitoring-kube-prometheus-prometheus 9090:9090 >/dev/null &
k -n monitoring port-forward svc/monitoring-grafana 3000:80 >/dev/null &
firing() { curl -s http://127.0.0.1:9090/api/v1/query --data-urlencode 'query=ALERTS{alertstate="firing"}' |
           jq -r '.data.result[].metric | "\(.alertname)\t\(.severity)\t\(.namespace // "")"' | sort; }
```

Grafana is at `http://127.0.0.1:3000`. Use the admin credentials from the protected file
that the `monitoring/grafana-admin` Secret was made from; never read them from the cluster.

## 1. Promote a release

### 1.1 What reaches Flux

Three inputs decide what runs. None of them is the release tag alone:

| Input | Where it lives | Set by |
| --- | --- | --- |
| Manifests (`platform/`, `k8s/base`, `config/keycloak`, `scripts/keycloak`) | Git commit pinned in GitRepository `flux-system/econgraph-platform` | `bootstrap.py reconcile` pins `spec.ref.commit` to the commit of `origin/<--git-ref>` ([`bootstrap.py`](../../scripts/platform/bootstrap.py) lines 111-124) |
| Encrypted Secrets | `platform/secrets/*.sops.yaml` at that same commit | committed by the operator; on `main` the list is empty ([`platform/secrets/kustomization.yaml`](../../platform/secrets/kustomization.yaml)) |
| Image references (`BACKEND_IMAGE`, `FRONTEND_IMAGE`, `CRAWLER_IMAGE`) and site settings | ConfigMap `flux-system/platform-settings`, substituted by every Kustomization (`postBuild.substituteFrom`) | applied with kubectl from a local file by `bootstrap.py reconcile`; not in Git |

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
- **G3**: image digests are not in Git. The audit trail is this runbook's evidence
  directory, not a pull request (ECO-235, unscheduled).
- **G4**: GHCR packages are private, and nothing under `platform/` or `infra/talos/` gives
  the nodes a pull credential (no `imagePullSecrets`, no Talos registry auth). As the repo
  stands, pods would fail with `ErrImagePull`.
- **G5**: Release Images also builds `econ-graph-chart-api`, but `k8s/base` has no
  chart-api Deployment, while `econ-graph-config` still points `CHART_API_SERVICE_URL` at
  `chart-api-service` (ECO-864).

### 1.2 Resolve the image digests

Resolve the tag to digests the same way [`release.yml`](../../.github/workflows/release.yml)
does, and check that the tag and `sha-<commit>` give one digest:

```sh
git fetch origin --tags
COMMIT="$(git rev-parse "$TAG^{commit}")"
for image in econ-graph-backend econ-graph-frontend econ-graph-crawler-worker; do
  by_tag="$(docker buildx imagetools inspect "ghcr.io/econgraph/$image:$TAG" --format '{{json .Manifest}}' | jq -r .digest)"
  by_sha="$(docker buildx imagetools inspect "ghcr.io/econgraph/$image:sha-$COMMIT" --format '{{json .Manifest}}' | jq -r .digest)"
  [ "$by_tag" = "$by_sha" ] && echo "$image $by_tag" || echo "MISMATCH $image $by_tag $by_sha"
done | tee "$EVIDENCE/digests.txt"
```

Stop on any `MISMATCH` or `null`. Then:

```sh
export BACKEND_DIGEST="$(awk '$1=="econ-graph-backend"{print $2}' "$EVIDENCE/digests.txt")"
export FRONTEND_DIGEST="$(awk '$1=="econ-graph-frontend"{print $2}' "$EVIDENCE/digests.txt")"
export CRAWLER_DIGEST="$(awk '$1=="econ-graph-crawler-worker"{print $2}' "$EVIDENCE/digests.txt")"
```

### 1.3 Make the deploy ref

```sh
git switch -c "$DEPLOY_REF" "$COMMIT"
cp "$SECRETS_DIR"/*.sops.yaml platform/secrets/
{ printf 'apiVersion: kustomize.config.k8s.io/v1beta1\nkind: Kustomization\nresources:\n'
  ls platform/secrets | grep '\.sops\.yaml$' | sed 's/^/- /'; } > platform/secrets/kustomization.yaml
"$KUBECTL" kustomize platform/secrets | python3 -c '
import sys, yaml
sys.path.insert(0, "scripts/platform")
from bootstrap import check_encrypted_inputs
check_encrypted_inputs([d for d in yaml.safe_load_all(sys.stdin) if d])
print("encrypted inputs OK")'
git add platform/secrets
git commit -m "chore(deploy): encrypted inputs for <cluster> $TAG"
git push origin "$DEPLOY_REF"
git fetch origin
export DEPLOY_COMMIT="$(git rev-parse "origin/$DEPLOY_REF")"
echo "$TAG $COMMIT $DEPLOY_REF $DEPLOY_COMMIT" | tee "$EVIDENCE/refs.txt"
```

The check reuses `check_encrypted_inputs` from `bootstrap.py`: every required Secret is
present, SOPS-encrypted, and has its required keys. The required list is in
[`platform/README.md`](../../platform/README.md), "Secret inputs".

Read what else this release changes on the platform, because every unsuspended
Kustomization applies the new commit too. A changed `imageName` in `clusters.yaml`, for
example, restarts each database:

```sh
git diff --stat "$(cat "$SETTINGS_DIR/deploy-commit-$PREV_TAG" 2>/dev/null || echo "$COMMIT")" "$DEPLOY_COMMIT" -- \
  platform k8s/base config/keycloak scripts/keycloak | tee "$EVIDENCE/platform-diff.txt"
echo "$DEPLOY_COMMIT" > "$SETTINGS_DIR/deploy-commit-$TAG"
```

### 1.4 Write the settings file

Start from the previous release's file. On the first promotion, copy
[`settings.example.yaml`](../../platform/clusters/threadripper/settings.example.yaml) and
replace every value as [`platform/README.md`](../../platform/README.md) describes. Then set
the images:

```sh
cp "$SETTINGS_DIR/platform-settings-$PREV_TAG.yaml" "$SETTINGS_DIR/platform-settings-$TAG.yaml"
python3 - "$SETTINGS_DIR/platform-settings-$TAG.yaml" <<'PY'
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
grep -E 'example\.invalid|192\.0\.2\.|localhost|\$\{|feature-build-required' "$SETTINGS_DIR/platform-settings-$TAG.yaml" && echo STOP
cp "$SETTINGS_DIR/platform-settings-$TAG.yaml" "$EVIDENCE/"
```

The `grep` repeats `bootstrap.py`'s refusal of example values (line 95), which the
production path in 1.7 doesn't run. It must print nothing.

**Gap G8**: the ObjectStores write to `s3://econgraph-${ENVIRONMENT}-*-backups`
([`objectstores.yaml`](../../platform/databases/objectstores.yaml)), and `bootstrap.py`
accepts only scratch `ENVIRONMENT` values. No production value or production bucket
names are defined (ECO-864).

### 1.5 Record the way back

Before anything changes, record what runs now, back up all three databases, and mark a
named restore point in each. Rollback after a migration restores to that point (3.3).

```sh
k -n flux-system get gitrepository econgraph-platform -o jsonpath='{.spec.ref}{"\n"}' | tee "$EVIDENCE/before-git-ref.json"
k -n flux-system get configmap platform-settings -o yaml > "$EVIDENCE/before-platform-settings.yaml"
{ echo "app  $(psql_on app econ_graph_app 'SELECT max(version) FROM __diesel_schema_migrations')"
  echo "data $(psql_on data econ_graph 'SELECT max(version) FROM __diesel_schema_migrations')"
  echo "keycloak $(psql_on keycloak keycloak 'SELECT version FROM migration_model ORDER BY update_time DESC LIMIT 1')"
} | tee "$EVIDENCE/before-versions.txt"

for plane in app data keycloak; do
  k apply -f - <<EOF
apiVersion: postgresql.cnpg.io/v1
kind: Backup
metadata:
  name: econgraph-$plane-pre-$SLUG
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
  k -n econ-graph wait "backup/econgraph-$plane-pre-$SLUG" --for=jsonpath='{.status.phase}'=completed --timeout=2h
done
k -n econ-graph get backups -o custom-columns='NAME:.metadata.name,CLUSTER:.spec.cluster.name,PHASE:.status.phase,STOPPED:.status.stoppedAt' | tee "$EVIDENCE/backups.txt"
```

Then the restore points. Each WAL switch makes PostgreSQL archive the segment that holds
the restore point; wait for its archive marker, as `cnpg-drill.py` does:

```sh
for pair in app:econ_graph_app data:econ_graph keycloak:keycloak; do
  plane="${pair%%:*}"; db="${pair#*:}"
  psql_on "$plane" "$db" "SELECT pg_create_restore_point('pre-$SLUG')"
  wal="$(psql_on "$plane" "$db" 'SELECT pg_walfile_name(pg_switch_wal())')"
  until [ "$(psql_on "$plane" "$db" "SELECT EXISTS (SELECT 1 FROM pg_ls_archive_statusdir() WHERE name = '$wal.done')")" = t ]; do sleep 5; done
  echo "$plane restore point pre-$SLUG archived in $wal at $(date -u +%FT%TZ)" | tee -a "$EVIDENCE/restore-points.txt"
done
```

A migration fails if a backup holds a long transaction. A backup that is still running
fails the wait above; start 1.6 only once all three show `completed`.

### 1.6 Quiesce

Hold the application back so the data migrations run first, against a quiet database.
This is the Talos form of kind's `precheck-db-migration.sh --stop-crawler`.

```sh
suspend_ks platform-application
k -n econ-graph scale deployment/crawler-worker --replicas=0
k -n econ-graph wait --for=delete pod -l app=crawler-worker --timeout=5m
psql_on data econ_graph "SELECT pid, usename, application_name, state, now() - xact_start
  FROM pg_stat_activity WHERE datname = current_database() AND pid <> pg_backend_pid()
  AND xact_start IS NOT NULL AND (state LIKE 'idle in transaction%' OR now() - xact_start > interval '5 seconds')"
```

The query must print nothing. If it lists sessions, find their owners before going on;
don't cancel them blindly.

While `platform-application` is suspended, Flux doesn't put the crawler back.

### 1.7 Apply the new ref and settings

**Scratch cluster** (supported by the repo):

```sh
python3 scripts/platform/bootstrap.py reconcile --kubeconfig "$KCFG" --context "$CTX" \
  --owner "$TRAIN4_OWNER" --control-node "$CONTROL_NODE" --cache "$CACHE" --kubectl "$KUBECTL" \
  --settings "$SETTINGS_DIR/platform-settings-$TAG.yaml" --git-ref "$DEPLOY_REF" --execute | tee "$EVIDENCE/reconcile.json"
```

**Production** (**Gap G1**: interim manual equivalent, unreviewed). These are the two
writes `bootstrap.py reconcile` makes, without its scratch-only guards. 1.3 and 1.4
repeat its other checks.

```sh
k -n flux-system get secret sops-age platform-git-auth -o name   # both must exist; never print their data
k apply -f "$SETTINGS_DIR/platform-settings-$TAG.yaml"
k -n flux-system patch gitrepository econgraph-platform --type merge \
  -p "{\"spec\":{\"ref\":{\"branch\":null,\"tag\":null,\"commit\":\"$DEPLOY_COMMIT\"}}}"
k -n flux-system annotate --overwrite gitrepository econgraph-platform reconcile.fluxcd.io/requestedAt="$(date -u +%s)"
```

On both kinds of cluster, check that the application is still held back:

```sh
k -n flux-system get kustomization platform-application -o jsonpath='{.spec.suspend}{"\n"}'   # true
```

`reconcile` re-applies the Kustomization objects with `kubectl apply`, which keeps a field
that only a patch set. If this prints anything but `true`, the backend may roll before
the data migration finishes. It still waits for `platform-data-schema` through
`dependsOn`, but the crawler comes back.

### 1.8 Migrate, then release the application

```sh
flux_wait platform-data-schema "$DEPLOY_COMMIT"        # allow 30 minutes
k -n econ-graph wait --for=condition=complete job/crawler-migrate --timeout=30m
k -n econ-graph logs job/crawler-migrate > "$EVIDENCE/crawler-migrate.log"
resume_ks platform-application
flux_wait platform-application "$DEPLOY_COMMIT"        # allow 30 minutes
for d in econ-graph-backend econ-graph-data-backend econ-graph-frontend keycloak crawler-worker; do
  k -n econ-graph rollout status "deployment/$d" --timeout=15m
done
date -u +%FT%TZ > "$EVIDENCE/promoted-at.txt"
```

If the Job fails, don't resume the application: go to [3.1](#31-did-a-migration-run).
A failed run can still leave earlier migrations applied, so 3.1 decides between 3.2
and 3.3.

The backends migrate `econ_graph_app` when they start, and two replicas roll one at a
time (`maxUnavailable: 1`, `maxSurge: 0`). For a short time an old replica serves
against the new schema. Nothing enforces a rule that migrations stay compatible with
the previous binary. **Gap G6** (ECO-309 migration Job, unscheduled).

Keycloak migrates its own database the first time a new Keycloak version starts.

## 2. Verify

These checks replace kind's `verify-release-images.sh` and the verify step of
`release-deploy.sh`. Save each output to `$EVIDENCE`.

1. **Flux applied the deploy commit everywhere.** Every row is `True`, not suspended, with
   `$DEPLOY_COMMIT` in its revision:

   ```sh
   k -n flux-system get gitrepository econgraph-platform
   k -n flux-system get kustomizations -o custom-columns='NAME:.metadata.name,READY:.status.conditions[?(@.type=="Ready")].status,SUSPENDED:.spec.suspend,REVISION:.status.lastAppliedRevision' | tee "$EVIDENCE/flux.txt"
   k -n flux-system get helmreleases | tee -a "$EVIDENCE/flux.txt"
   ```

2. **Every pod runs the promoted digest:**

   ```sh
   for pair in "econ-graph-backend=$BACKEND_DIGEST" "econ-graph-data-backend=$BACKEND_DIGEST" \
               "econ-graph-frontend=$FRONTEND_DIGEST" "crawler-worker=$CRAWLER_DIGEST"; do
     app="${pair%%=*}"; want="${pair#*=}"
     k -n econ-graph get pods -l app="$app" -o jsonpath='{range .items[*]}{.metadata.name}{" "}{.status.containerStatuses[0].imageID}{"\n"}{end}' |
       while read -r pod id; do case "$id" in *"$want") echo "OK $pod";; *) echo "MISMATCH $pod $id";; esac; done
   done | tee "$EVIDENCE/images.txt"
   ```

3. **Migrations.** Record the versions after promotion:

   ```sh
   { echo "app  $(psql_on app econ_graph_app 'SELECT max(version) FROM __diesel_schema_migrations')"
     echo "data $(psql_on data econ_graph 'SELECT max(version) FROM __diesel_schema_migrations')"
     echo "keycloak $(psql_on keycloak keycloak 'SELECT version FROM migration_model ORDER BY update_time DESC LIMIT 1')"
   } | tee "$EVIDENCE/after-versions.txt"
   ```

4. **Databases and backups.** Each cluster reports `Cluster in healthy state` with 2 ready
   instances and `ContinuousArchiving` `True`, and a manual backup health check passes:

   ```sh
   for plane in app data keycloak; do
     k -n econ-graph get cluster "econgraph-$plane" -o jsonpath='{.metadata.name} {.status.phase} {.status.readyInstances} {.status.conditions[?(@.type=="ContinuousArchiving")].status}{"\n"}'
   done | tee "$EVIDENCE/clusters.txt"
   job="backup-health-$(date -u +%Y%m%d%H%M%S)"
   k -n econ-graph create job --from=cronjob/cnpg-backup-health "$job"
   k -n econ-graph wait --for=condition=complete "job/$job" --timeout=5m
   k -n econ-graph logs "job/$job" | tee -a "$EVIDENCE/backup-health.txt"
   ```

5. **Alerts.** Only `Watchdog`, and `InfoInhibitor` if present, may be firing:

   ```sh
   firing | tee "$EVIDENCE/alerts-after.txt"
   amtool --alertmanager.url=http://127.0.0.1:9093 alert query
   ```

   Also check that healthchecks.io shows regular Watchdog pings
   ([Alert delivery](../../platform/README.md#alert-delivery)). The rules come from
   `platform/observability/`: `monitors.yaml` (Postgres readiness, backup health),
   `rules-edge.yaml` (edge 5xx and latency, Keycloak), `rules-auth.yaml` (anonymous
   grants), `rules-crawler.yaml` (crawler and SEC), `rules-disk.yaml` (host, node and
   database volumes), `rules-timeseries.yaml` and `rules-tunnel.yaml`. Watch for 30 minutes after the
   rollout. Several crawler alerts need `for: 1h` to fire, so look again the next day.

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
   GraphQL request returns a `data` object with a type name, not `errors`. Then run the
   QA scenarios ([qa-scenarios.md](../release/qa-scenarios.md)) with `PUBLIC_HOST`
   resolving to the gateway on the operator host. Remove the hosts line afterwards:

   ```sh
   echo "$GATEWAY_IP $PUBLIC_HOST" | sudo tee -a /etc/hosts
   scripts/qa/run-scenarios.sh --target "$(k -n flux-system get configmap platform-settings -o jsonpath='{.data.PUBLIC_ORIGIN}')" \
     --profile lab --json "$EVIDENCE/qa.json"
   ```

   After the tunnel is on, use `scripts/deploy/check-public-site.sh --host econgraph.com
   --via-cloudflare` and the `production` profile (ECO-859 step 5). **Gap G11**:
   `check-public-site.sh`'s default local mode expects kind's HTTPS gateway on the host,
   so the Talos gateway has no pre-tunnel public-site check.

## 3. Roll back

Under Flux, a rollback re-pins the previous deploy commit and re-applies the previous
settings file. `kubectl set image` or `kubectl rollout undo` would only last until Flux
next reconciles. Which rollback you need depends on whether a migration ran.

### 3.1 Did a migration run?

```sh
{ echo "app  $(psql_on app econ_graph_app 'SELECT max(version) FROM __diesel_schema_migrations')"
  echo "data $(psql_on data econ_graph 'SELECT max(version) FROM __diesel_schema_migrations')"
  echo "keycloak $(psql_on keycloak keycloak 'SELECT version FROM migration_model ORDER BY update_time DESC LIMIT 1')"
} > "$EVIDENCE/rollback-versions.txt"
diff "$EVIDENCE/before-versions.txt" "$EVIDENCE/rollback-versions.txt" && echo "no migration ran"
```

If there's no difference, use 3.2. Otherwise use 3.3 and restore each plane whose line
changed. If in doubt, restore all three. `econ_graph_app` changes as soon as one backend
pod has started on the new image.

### 3.2 Before a migration ran

```sh
export PREV_DEPLOY_COMMIT="$(cat "$SETTINGS_DIR/deploy-commit-$PREV_TAG")"
k apply -f "$SETTINGS_DIR/platform-settings-$PREV_TAG.yaml"
k -n flux-system patch gitrepository econgraph-platform --type merge \
  -p "{\"spec\":{\"ref\":{\"branch\":null,\"tag\":null,\"commit\":\"$PREV_DEPLOY_COMMIT\"}}}"
k -n flux-system annotate --overwrite gitrepository econgraph-platform reconcile.fluxcd.io/requestedAt="$(date -u +%s)"
resume_ks platform-application     # no-op if it was already resumed
flux_wait platform-application "$PREV_DEPLOY_COMMIT"
for d in econ-graph-backend econ-graph-data-backend econ-graph-frontend keycloak crawler-worker; do
  k -n econ-graph rollout status "deployment/$d" --timeout=15m
done
```

On a scratch cluster you can instead re-run the 1.7 `bootstrap.py reconcile` command with
`--git-ref "deploy/<cluster>-$PREV_TAG"` and the previous settings file. Then run all of
[Verify](#2-verify) with the previous digests.

### 3.3 After a migration ran

The previous release can't run on the migrated schema, and migrations are forward-only
([MIGRATIONS.md](../development/MIGRATIONS.md)). Rollback means restoring each changed
database to the `pre-$SLUG` restore point from 1.5, then running the previous release.
Writes made after that point are lost: sign-ups, annotations and settings since the
promotion on `econ_graph_app`, and crawled data on `econ_graph`. Say so wherever users
are told about the outage.

1. Follow [4. Restore](#4-restore-a-cnpg-cluster-from-the-object-store) for each changed
   plane, with `RESTORE_POINT=pre-$SLUG` and `RESTORE_FOR_TAG=$PREV_TAG`, so that the
   recovery spec is committed on the **previous** release's deploy ref.
2. Before resuming the application in step 4.10, apply the previous settings file:
   `k apply -f "$SETTINGS_DIR/platform-settings-$PREV_TAG.yaml"`.
3. Run all of [Verify](#2-verify) with the previous digests. The migration versions must
   match `$EVIDENCE/before-versions.txt`.

### 3.4 Alerts expected during a rollback

Silence only the alerts the outage causes, for the length of the window. Never silence
`Watchdog`:

```sh
amtool --alertmanager.url=http://127.0.0.1:9093 silence add --duration=2h --comment="rollback $TAG (ECO-887 runbook)" \
  'alertname=~"EconGraphEdge5xxRatioHigh|EconGraphKeycloakDown|EconGraphKeycloakInstanceDown|CrawlerWorkerDown"'
```

| Alert (source) | Severity, delay | Image rollback (3.2) | Restore rollback (3.3) |
| --- | --- | --- | --- |
| `CrawlerWorkerDown` (`rules-crawler.yaml`) | critical, 10m | if the crawler stays at 0 for 10 minutes (1.6 to 3.2) | yes: the crawler is at 0 throughout |
| `CrawlerMetricsMissing` (`rules-crawler.yaml`) | info, 1h | no | if the window lasts over an hour |
| `EconGraphEdge5xxRatioHigh` (`rules-edge.yaml`) | critical, 5m | no: one replica keeps serving | yes, when there is traffic: the backends are at 0 and the gateway returns 503 |
| `EconGraphKeycloakDown` (`rules-edge.yaml`) | critical, 5m | no | yes, while Keycloak is at 0 |
| `EconGraphKeycloakInstanceDown` (`rules-edge.yaml`) | warning, 10m | only if a Keycloak pod hangs during the roll | yes, while one instance comes back after the other |
| `EconGraphPostgresInstanceNotReady` (`monitors.yaml`) | warning, 2m | no | yes, while a restored instance recovers or joins |
| `EconGraphBackupHealthCheckStale` (`monitors.yaml`) | critical, about 25m after the last good check | no | yes, if the window lasts over about 25 minutes: the check fails while a cluster is missing or not yet archiving |
| `KubeJobFailed` (chart default rules) | warning | no | likely, for the `cnpg-backup-health` Jobs that fail during the restore |
| `AnonymousGrant*` (`rules-auth.yaml`) | warning or critical | no | no: these have no series while the backends are down. If one fires after the backends return, that's a real fault |
| `CloudflareTunnel*` (`rules-tunnel.yaml`) | critical | no | no while the tunnel and production marker aren't on the platform (**Gap G10**); afterwards `CloudflareTunnelDown` only if cloudflared is stopped |

Anything else that fires is not expected; investigate it before you close the window.
Afterwards, expire the silence (`amtool ... silence expire ID`) and check that `firing`
shows only `Watchdog`. **Gap G9**: no alert fires for a Flux Kustomization or HelmRelease
that isn't Ready, or for one left suspended. Check `flux.txt` (Verify step 1) by hand.

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
only on a disposable, annotated copy ([4.11](#411-the-scratch-drill)). The procedure below
restores **under the same cluster name**. It keeps the Services, the `*-rw` endpoints in
`econ-graph-secrets`, the ScheduledBackups, the backup health check and the alert regexes
working unchanged. Because Flux owns the Cluster, the restore is a Git change: the
cluster's entry in `platform/databases/clusters.yaml` switches from `initdb` to
`recovery`, and the restored cluster archives under a new `serverName` so that it never
writes into the archive it restores from. The rehearsal must show that CNPG and Flux
accept this.

### 4.1 Choose and check

```sh
export PLANE=app                     # app | data | keycloak
export RESTORE_POINT="pre-$SLUG"     # a pg_create_restore_point name from 1.5
export RESTORE_FOR_TAG="$TAG"        # the release to run afterwards: $PREV_TAG for a 3.3 rollback
export DEPLOY_REF="deploy/<cluster>-$RESTORE_FOR_TAG"
export SOURCE_SERVER="$(k -n econ-graph get cluster "econgraph-$PLANE" -o jsonpath='{.spec.plugins[0].parameters.serverName}')"
export SOURCE_SERVER="${SOURCE_SERVER:-econgraph-$PLANE}"   # empty means the cluster name
export NEW_SERVER="econgraph-$PLANE-r$(date -u +%Y%m%d%H%M)"
echo "$PLANE $RESTORE_POINT $SOURCE_SERVER -> $NEW_SERVER" | tee -a "$EVIDENCE/restore.txt"
k -n econ-graph get backups -o custom-columns='NAME:.metadata.name,CLUSTER:.spec.cluster.name,PHASE:.status.phase,STOPPED:.status.stoppedAt' |
  grep "econgraph-$PLANE " | tee -a "$EVIDENCE/restore.txt"
grep "^$PLANE " "$EVIDENCE/restore-points.txt"
```

A completed backup must have stopped before the restore point. Check the space too. The
old cluster's volumes are kept (`econgraph-local` uses `Retain`), so the restored copy
needs its full request again on each of the two workers:

```sh
curl -s http://127.0.0.1:9090/api/v1/query \
  --data-urlencode 'query=node_filesystem_avail_bytes{mountpoint="/var/mnt/postgres"} / 2^30' |
  jq -r '.data.result[] | "\(.metric.instance) \(.value[1]) GiB free"' | tee -a "$EVIDENCE/restore.txt"
```

Each worker needs at least the plane's volume request free, plus room for WAL. If it
doesn't, stop. Freeing space means deleting a retained volume, which is an owner
decision ([BACKUPS.md](BACKUPS.md), "Nothing is deleted automatically").

### 4.2 Stop writes and Flux

Silence the alerts listed in 3.4, then:

```sh
for ks in platform-application platform-data-schema platform-databases; do suspend_ks "$ks"; done
for d in econ-graph-backend econ-graph-data-backend crawler-worker keycloak; do
  k -n econ-graph scale "deployment/$d" --replicas=0
done
k -n econ-graph wait --for=delete pod -l 'app in (econ-graph-backend,econ-graph-data-backend,crawler-worker,keycloak)' --timeout=10m
```

This is a full outage, which is the safe default for any of the three planes.

### 4.3 Record the cluster being replaced

```sh
k -n econ-graph get cluster "econgraph-$PLANE" -o yaml > "$EVIDENCE/replaced-cluster-$PLANE.yaml"
k -n econ-graph get pvc -l "cnpg.io/cluster=econgraph-$PLANE" -o custom-columns='PVC:.metadata.name,PV:.spec.volumeName,NODE:.metadata.annotations.volume\.kubernetes\.io/selected-node' |
  tee "$EVIDENCE/replaced-volumes-$PLANE.txt"
```

### 4.4 Commit the recovery spec

Work on the deploy ref of the release that runs after the restore (`$DEPLOY_REF`, set
in 4.1).

```sh
git fetch origin
git switch "$DEPLOY_REF" && git merge --ff-only "origin/$DEPLOY_REF"
python3 - "$PLANE" "$RESTORE_POINT" "$SOURCE_SERVER" "$NEW_SERVER" <<'PY'
import sys, yaml
from pathlib import Path
plane, point, source, new = sys.argv[1:]
path = Path('platform/databases/clusters.yaml')
docs = list(yaml.safe_load_all(path.read_text()))
for doc in docs:
    if doc['metadata']['name'] != 'econgraph-' + plane:
        continue
    spec = doc['spec']
    init = spec['bootstrap'].get('initdb') or spec['bootstrap']['recovery']
    spec['bootstrap'] = {'recovery': {
        'source': 'origin', 'database': init['database'], 'owner': init['owner'],
        'secret': init['secret'], 'recoveryTarget': {'targetName': point}}}
    spec['externalClusters'] = [{'name': 'origin', 'plugin': {
        'name': 'barman-cloud.cloudnative-pg.io',
        'parameters': {'barmanObjectName': plane + '-backups', 'serverName': source}}}]
    spec['plugins'][0]['parameters']['serverName'] = new
path.write_text(yaml.safe_dump_all(docs, sort_keys=False))
PY
git diff platform/databases/clusters.yaml | tee "$EVIDENCE/recovery-spec-$PLANE.diff"
git commit -am "chore(deploy): restore econgraph-$PLANE to $RESTORE_POINT"
git push origin "$DEPLOY_REF"
git fetch origin
export RESTORE_COMMIT="$(git rev-parse "origin/$DEPLOY_REF")"
echo "$RESTORE_COMMIT" > "$SETTINGS_DIR/deploy-commit-$RESTORE_FOR_TAG"
```

The last line points that release's `deploy-commit` file at the commit that describes
the restored cluster, so a later rollback to it keeps the recovery spec. To restore to a time
instead of the restore point, replace `{'targetName': point}` with
`{'targetTime': '2026-11-10T15:04:05Z'}`. The base backup must have stopped before that
time, and the archive must hold WAL after it.

### 4.5 Replace the cluster

```sh
k -n econ-graph delete cluster "econgraph-$PLANE" --wait=true --timeout=10m
k get pv | grep "econgraph-$PLANE" | tee -a "$EVIDENCE/replaced-volumes-$PLANE.txt"   # Released, not gone
k -n flux-system patch gitrepository econgraph-platform --type merge \
  -p "{\"spec\":{\"ref\":{\"branch\":null,\"tag\":null,\"commit\":\"$RESTORE_COMMIT\"}}}"
k -n flux-system annotate --overwrite gitrepository econgraph-platform reconcile.fluxcd.io/requestedAt="$(date -u +%s)"
resume_ks platform-databases
k -n econ-graph wait "cluster/econgraph-$PLANE" --for=jsonpath='{.status.readyInstances}'=2 --timeout=4h
flux_wait platform-databases "$RESTORE_COMMIT"
```

Deleting the Cluster deletes its PVCs. The PVs stay `Released` with their data, as the
fallback copy until the owner decides otherwise. Allow time in proportion to size: the
data plane restores up to 100Gi from the object store.

### 4.6 Check the restored cluster

```sh
k -n econ-graph get cluster "econgraph-$PLANE" -o jsonpath='{.status.phase} {.status.readyInstances} {.status.conditions[?(@.type=="ContinuousArchiving")].status}{"\n"}' | tee -a "$EVIDENCE/restore.txt"
case "$PLANE" in
  app)      psql_on app econ_graph_app 'SELECT max(version) FROM __diesel_schema_migrations' ;;
  data)     psql_on data econ_graph 'SELECT max(version) FROM __diesel_schema_migrations' ;;
  keycloak) psql_on keycloak keycloak 'SELECT version FROM migration_model ORDER BY update_time DESC LIMIT 1' ;;
esac | tee -a "$EVIDENCE/restore.txt"
grep "^$PLANE" "$EVIDENCE/before-versions.txt"
```

The phase is `Cluster in healthy state` with 2 instances and `ContinuousArchiving` is
`True`. For a 3.3 rollback the version must equal the `before-versions.txt` line.

### 4.7 Take a first backup under the new server name

PITR from the new archive prefix needs a base backup there:

```sh
k apply -f - <<EOF
apiVersion: postgresql.cnpg.io/v1
kind: Backup
metadata:
  name: econgraph-$PLANE-restored-$(date -u +%Y%m%d%H%M)
  namespace: econ-graph
spec:
  cluster:
    name: econgraph-$PLANE
  method: plugin
  pluginConfiguration:
    name: barman-cloud.cloudnative-pg.io
EOF
k -n econ-graph get backups -o custom-columns='NAME:.metadata.name,CLUSTER:.spec.cluster.name,PHASE:.status.phase' | grep "econgraph-$PLANE-restored"
```

Wait until it shows `completed`.

### 4.8 Repeat for the other planes

For another plane, set `PLANE` again and repeat 4.1 to 4.7. Keep everything suspended and
scaled down until every plane is restored. In 4.4, commit each restore on top of the
previous one.

### 4.9 Restored clusters in `main`

The recovery spec and the new `serverName` must also reach `main`, in a PR that changes
only `platform/databases/clusters.yaml` the same way. Otherwise the next release's deploy
ref brings back the `initdb` spec and the old `serverName`. Flux would then try to archive
into the old prefix, and archiving would fail. Merge that PR before the next promotion.

### 4.10 Restart the application

```sh
for ks in platform-data-schema platform-application; do resume_ks "$ks"; done
flux_wait platform-application "$RESTORE_COMMIT"
for d in econ-graph-backend econ-graph-data-backend econ-graph-frontend keycloak crawler-worker; do
  k -n econ-graph rollout status "deployment/$d" --timeout=15m
done
```

Resuming `platform-application` restores the replica counts from Git. Then run all of
[Verify](#2-verify), expire the silence, and record the outage window in `$EVIDENCE/restore.txt`.

### 4.11 The scratch drill

The platform's own PITR gate runs on a disposable copy, not on the `econ-graph` clusters.
It needs the explicit `econgraph.io/disposable-drill` annotations that
[`platform/README.md`](../../platform/README.md) ("Backups, copy migration and acceptance")
describes:

```sh
python3 scripts/platform/cnpg-drill.py backup-restore --kubeconfig "$KCFG" --context "$CTX" \
  --namespace train4-proof --owner "$TRAIN4_OWNER" --source-cluster owned-copy \
  --object-store owned-backups --kubectl "$KUBECTL" --report "$EVIDENCE/cnpg-pitr.json" --execute
```

Without `--execute` it is a dry run. It proves base backup, WAL archiving and PITR to a
time. Steps 4.1 to 4.10 prove the procedure production would use. The rehearsal runs both.

## 5. Cutover from kind

The relaunch is one cutover (ECO-859). This is the order, with the runbook's part in each
step:

1. **Preconditions.** All of these must be recorded:
   - the platform's external release gates ([`platform/README.md`](../../platform/README.md),
     "Backups, copy migration and acceptance", the bold list) and the Talos fit gate
     ([`infra/talos/README.md`](../../infra/talos/README.md));
   - ECO-864: production bootstrap and reconcile, the tunnel, release images for every
     workload; with it Gaps G1, G2, G4, G5, G8 and G10;
   - ECO-438 and ECO-426: moving the kind databases into the three CNPG clusters;
   - the ECO-883 decision.
2. **Rehearse** the whole path on a fresh scratch Talos cluster, on a copy of the data,
   with [section 7](#7-rehearsal-checklist) as the checklist (ECO-859 step 1).
3. **Approval.** Joe approves the rehearsed runbook (ECO-859 step 2).
4. **Freeze and export kind.** The site is offline, so kind serves nobody. Take the final
   dumps with `scripts/deploy/backup-kind-databases.sh --dry-run`, then without
   `--dry-run`, and keep that backup directory. **Gap G13**: `cnpg-drill.py migrate-copy` is
   scratch-only. It takes a per-database pg_dump archive and a JSON manifest of table
   counts, which `backup-kind-databases.sh` doesn't write (it writes `counts.env` inside an
   encrypted tarball). The production copy procedure belongs to ECO-438 and ECO-426.
5. **Load production.** Bootstrap the production Talos cluster (ECO-864), load the data
   (ECO-438), then promote the release candidate with section 1 and verify with section 2.
   The tunnel stays off.
6. **Catalog roles, tag, tunnel, QA, www, reboot test, refetch**: ECO-859 steps 3 to 8. Tag
   `v4.2.0` on the same digests (1.2 must give the same three digests for the final tag),
   then turn the tunnel on. **Gaps**: `switch-main-host.sh` (step 6) changes Keycloak and the
   backend with `kubectl set env`, which Flux reverts. On Talos the main host is
   `PUBLIC_HOST`/`PUBLIC_ORIGIN` in the settings file, applied as in 1.7. Also,
   `refetch-full-history.sh` (step 8) runs only against kind. Neither is ported.

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
authorized ([`platform/README.md`](../../platform/README.md), last paragraph of "Backups,
copy migration and acceptance"). How long kind and its data are kept after cutover is
ECO-883 question 4. Retiring kind means following the table in section 6. Deleting kind's
volumes, snapshots or dumps is a separate owner decision; nothing here deletes them.

## 6. Kind scripts and documents

Recommendations only; this runbook deletes nothing. "Ported" means the Talos procedure is
in this runbook or in [`platform/README.md`](../../platform/README.md); delete the kind
original once kind is retired. If ECO-883 picks the kind fallback, every "ported" kind
item stays until that fallback is no longer needed. "Release lab" means kept while the
release lab runs on kind (ECO-860 and ECO-862 decide what replaces it). "Not ported" is a
gap.

### Documents and commands

| Item | Recommendation | Where it went, or why |
| --- | --- | --- |
| [docs/development/RELEASE_PROCESS.md](../development/RELEASE_PROCESS.md) | Keep | Branches, QA and tagging don't depend on the cluster; it links here |
| [RELEASE_DEPLOY.md](RELEASE_DEPLOY.md) | Ported, then delete | Sections 1 to 3 |
| [RELEASE_IMAGES.md](RELEASE_IMAGES.md) | Ported, then delete | Digest resolution in 1.2, image check in Verify step 2; nodes pull by digest, so load and per-node rollback have no Talos equivalent |
| [RELEASE_LAB.md](RELEASE_LAB.md) | Release lab | ECO-860 says which parts still apply |
| [POSTGRES_BACKUP_RESTORE.md](POSTGRES_BACKUP_RESTORE.md) | Ported, then delete; keep "Restore encrypted archives" while kind-era dumps are kept | CNPG backups and section 4 |
| [BACKUPS.md](BACKUPS.md) | Ported in part | CNPG object-store backups replace the nightly dumps. The host object store's own protection and off-site copies stay (ECO-835, ECO-497); its restore drill is still the way to open kind-era archives |
| [k8s/README.md](../../k8s/README.md) | Ported, then delete; keep "Disposable branch environments" until ECO-862 | [`platform/README.md`](../../platform/README.md) and this runbook |
| [HOST_STORAGE.md](HOST_STORAGE.md) | Delete after retirement | Kind's static host-subvolume PVs; Talos uses `econgraph-local` |
| [MAIN_HOST.md](MAIN_HOST.md) | Not ported | See section 5, step 6 |
| [APP_DATA_CONVERSION.md](APP_DATA_CONVERSION.md) | Release lab, and the cutover | ECO-426 |
| [HOST_DISK_MONITORING.md](HOST_DISK_MONITORING.md), [OBJECTSTORE_HOST.md](OBJECTSTORE_HOST.md) | Keep | Already describe the platform's host |
| `.cursor/commands/deploy.md` | Delete after retirement | Runs kind's `deploy.sh`; promotion is section 1 |
| `.cursor/commands/k8s-deploy.md` | Delete after retirement | Applies `k8s/manifests` by hand, which Flux would revert or fight |
| `.cursor/commands/k8s-restart.md` | Keep | `rollout restart` works under Flux |
| `.cursor/commands/k8s-context.md`, `k8s-logs.md`, `debug-k8s.md` | Keep | Use the current context; point it at the Talos cluster |
| MicroK8s documents, `DEPLOYMENT_RESTART.md`, `K8S_RESTART_COMMANDS.md` | Delete | ECO-898 owns this |

### `scripts/deploy/`

Each script's `test-*` file goes with it.

| Script | Recommendation | Where it went, or why |
| --- | --- | --- |
| `release-deploy.sh` | Ported, then delete | Sections 1 to 3 |
| `backup-postgres-now.sh` | Ported, then delete | On-demand `Backup` in 1.5 |
| `load-release-images.sh`, `verify-release-images.sh`, `rollback-release-images.sh`, `lib/release-images.sh` | Ported, then delete | 1.2, Verify step 2, 3.2 |
| `precheck-db-migration.sh` | Ported, then delete | 1.6 |
| `deploy.sh`, `lib/deploy-config.sh`, `lib/app-config.sh`, `lib/data-migrations.sh` | Ported, then delete | Flux applies; 1.8 orders the migration |
| `backup-keycloak-db.sh`, `restore-keycloak-db.sh`, `lib/keycloak-db.sh` | Ported, then delete | The keycloak plane in 1.5 and section 4 |
| `restore-postgres-backup.sh` | Ported, then delete | Section 4 |
| `create-secrets.sh`, `set-crawler-keys.sh`, `lib/keycloak-secret.sh` | Ported, then delete | SOPS inputs ([`platform/README.md`](../../platform/README.md), "Secret inputs") and 1.3 |
| `install-gateway.sh`, `apply-gateway-routes.sh`, `cutover-to-gateway.sh`, `render-public-host.py`, `lib/public-host.sh` | Ported, then delete | `platform/network` and the application's HTTPRoutes |
| `apply-host-storage.sh`, `migrate-to-host-storage.sh`, `protect-postgres-pv.sh`, `released-pvs.sh` | Delete after retirement | `econgraph-local` with `Retain` |
| `recreate-kind-cluster.sh`, `teardown.sh`, `setup-port-forwarding.sh`, `setup-letsencrypt-cloudflare.sh`, `init-database.sh` | Delete after retirement | Kind only |
| `install-kind.sh`, `load-kind-images.sh`, `lib/kind-env.sh`, `lib/kind-version.sh`, `lib/kind_tools.py` | Release lab | The lab is a kind cluster |
| `release-lab.sh`, `release-lab.py`, `backup-kind-databases.sh` | Release lab | `backup-kind-databases.sh` is also the cutover export (section 5, step 4) |
| `restore-kind-databases.sh` | Keep until kind is retired | The kind side of any ECO-883 fallback |
| `convert-app-data.sh`, `split-app-data.py`, `lib/app-database-guard.sh` | Release lab, and the cutover | ECO-426 |
| `measure-v41-first-deploy.sh`, `measure-v41-first-deploy.py` | Delete | v4.1.0 is orphaned (ECO-860) |
| `mark-production-cluster.sh`, `lib/cluster-role.sh` | Not ported | The Talos cluster has no production marker; **Gap G10** (ECO-864) |
| `stage-tunnel-origin-ca.sh` | Not ported | ECO-864 decides the tunnel on the platform |
| `switch-main-host.sh` | Not ported | Section 5, step 6 |
| `check-public-site.sh` | Keep; `--via-cloudflare` only | **Gap G11** |
| `refetch-full-history.sh`, `check-selection.sh` | Not ported | They use `lib/kind-env.sh`; ECO-859 step 8 needs the refetch on Talos |
| `create-agent-kubeconfig.sh`, `run-agent-session.sh`, `test-agent-access.sh`, `lib/agent-access.sh`, `lib/agent_paths.py` | Not ported | Talos has no read-only agent identity yet ([AGENT_ACCESS.md](AGENT_ACCESS.md)) |
| `deploy-branch.sh`, `deploy-branch.py`, `render-branch.py`, `branch-network-check.py` | Keep | Branch environments; ECO-862 moves them to the new cluster |
| `build-images.sh` | Keep | Local development builds |
| `restart-k8s-rollout.sh` | Delete | ECO-898 owns this |

## 7. Rehearsal checklist

The rehearsal is Joe's, or an authorised operator's, on a fresh scratch Talos cluster.
Keep all output in `$EVIDENCE`, which stays private until sanitized: reports can contain
hostnames and connection details. When it's done, post a sanitized summary as a comment
on ECO-887 and add the artifacts to the Talos fit evidence file
([`fit-evidence.example.json`](../../infra/talos/fit-evidence.example.json):
`fresh-cluster-restore`, release-E2E during drain).

- [ ] Session set up as in [Before you start](#before-you-start). Record the cluster's
      `kube-system` UID and the operator's name.
- [ ] Gap G4 worked around for the rehearsal, with how it was done recorded. Without it,
      no release image pulls.
- [ ] **Promote** release A, then release B, with section 1. Record `digests.txt`,
      `refs.txt`, `platform-diff.txt`, the settings files, `backups.txt`,
      `restore-points.txt`, `crawler-migrate.log` and `promoted-at.txt`. Pick a B whose
      data or app migrations change the schema.
- [ ] `platform-application` stayed suspended across 1.7 (the `true` check).
- [ ] **Verify** B with section 2. Record `flux.txt`, `images.txt`, `after-versions.txt`,
      `clusters.txt`, `backup-health.txt`, `alerts-after.txt`, `qa.json`, and dashboard
      screenshots around the promotion time.
- [ ] **Roll back before a migration**: promote B again, stop after 1.6, then run 3.2. Record
      the versions (unchanged) and Verify against A.
- [ ] **Roll back after a migration**: promote B fully, then run 3.3 for every changed plane.
      Record each `restore.txt`, `recovery-spec-*.diff`, `replaced-*` file, the outage
      window, and the versions matching `before-versions.txt`.
- [ ] Record the alerts that fired during each rollback (`firing` every few minutes) and
      compare them with the 3.4 table. Correct the table with what actually fired.
- [ ] **Restore** the keycloak plane to a time (`targetTime`) with section 4 on its own, and
      check that sign-in works afterwards.
- [ ] CNPG accepted the recovery spec on an existing cluster name, and Flux re-applied it
      without errors (4.5). If either refused, record the error: Gap G7 then needs a
      different design.
- [ ] The scratch drill (4.11) passed. Record `cnpg-pitr.json`.
- [ ] Time each step. The data plane's restore time is the outage a 3.3 rollback costs.
- [ ] List every place where you had to ask, guess, or type a command this runbook
      doesn't contain. Each one is a fix to this runbook before production.

## Gaps

Where the repo has no mechanism. Each one needs a ticket, or a note on the ticket named.

| Id | Gap | Ticket |
| --- | --- | --- |
| G1 | `bootstrap.py` cni, flux and reconcile accept scratch clusters only (`scripts/platform/bootstrap.py` lines 60-63, 93); production has only the unreviewed manual 1.7 | ECO-864 |
| G2 | Flux can't track a release tag as is: `--git-ref` must be an `origin` branch, and the encrypted inputs must be committed at the tracked commit, so a deploy ref (tag plus ciphertext commit) is needed | ECO-864, ECO-235 |
| G3 | Image digests live in an out-of-Git ConfigMap; there is no promotion PR or audit trail | ECO-235 |
| G4 | Nodes have no credential to pull the private GHCR packages | none yet (ECO-864 acceptance needs it) |
| G5 | No chart-api workload on the platform; `CHART_API_SERVICE_URL` points at a missing Service | ECO-864 |
| G6 | Backends migrate at start during a rolling update; no expand/contract rule | ECO-309 |
| G7 | No reviewed production restore under Flux; `cnpg-drill.py` is scratch-only; a restored cluster's Git spec must be forward-ported to `main` | none yet |
| G8 | No production `ENVIRONMENT` or bucket names for the ObjectStores | ECO-864 |
| G9 | No alerts for Flux objects that aren't Ready or are left suspended | none yet |
| G10 | No production marker or tunnel on the platform, so `CloudflareTunnel*` alerts are inert | ECO-864 |
| G11 | No pre-tunnel public-site check for the HTTP-only Talos gateway | none yet |
| G12 | No documented way to give the platform scripts a decrypted kubeconfig | none yet |
| G13 | No production kind-to-CNPG copy; `migrate-copy` input doesn't match `backup-kind-databases.sh` output | ECO-438, ECO-426 |
| G14 | `switch-main-host.sh` and `refetch-full-history.sh` are kind-only, and ECO-859 steps 6 and 8 need both | ECO-859 |
| G15 | `crawler-migrate` and `keycloak-realm-import` have `ttlSecondsAfterFinished: 86400`, so Flux recreates and reruns them after each TTL (confirm in the rehearsal) | none yet |
