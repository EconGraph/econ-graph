# Sanitized fixtures

A sanitized fixture is a copy of the production databases with the personal
data and secrets taken out: the data that release labs and CI upgrade
rehearsals restore instead of real user data. It is built from the
production dumps by `scripts/fixtures/build-sanitized-fixture.sh`, on the
host, never in CI.

Two kinds:

| Kind | What it holds | Where it lives |
|------|---------------|----------------|
| Full fixture | every database, every row, sanitized | `~/econgraph-fixtures/<UTC stamp>/`; stays on the host |
| CI subset (`--ci-subset`) | a few hundred series spread over every source with their observations and vintages, the reference tables they need, and the whole sanitized app and Keycloak databases | `~/econgraph-fixtures/<UTC stamp>-ci-subset/`; the only part that may leave the host (committed or published as a CI artifact) |

Fixtures are never overwritten and never pruned. Remove an old one by hand.

## Building one

The builder accepts **plaintext pg_dump archives**: either retained legacy dumps
or copies decrypted by the operator into private temporary storage on the host.
The current in-cluster backup job writes `.dump.age` ciphertext; those files
cannot be passed directly to this builder. Follow
[operator-host recovery](POSTGRES_BACKUP_RESTORE.md#restore) to decrypt the
chosen archives, then supply their plaintext paths explicitly:

```sh
scripts/fixtures/build-sanitized-fixture.sh --data /dev/shm/fixture-stage/econ_graph.dump \
  --app /dev/shm/fixture-stage/econ_graph_app.dump --keycloak /dev/shm/fixture-stage/keycloak.dump
```

Omit `--app` only for the shared-database layout. Keep private identities on the
operator host, never in a pod or fixture output, and remove the operator-created
plaintext staging after the build. Host backups that use `backup.tar.age` must
also be opened by their operator before the extracted dump paths are supplied.
This builder does not decrypt archives or manage identities.

For retained **plaintext** verified host dumps (`scripts/backup/dump-databases.sh`,
see [BACKUPS.md](BACKUPS.md)), the legacy convenience form is:

```sh
scripts/fixtures/build-sanitized-fixture.sh --latest --dry-run   # the plan; starts and writes nothing
scripts/fixtures/build-sanitized-fixture.sh --latest
scripts/fixtures/build-sanitized-fixture.sh --latest --ci-subset
```

Other inputs:

- `--from-dir DIR`: a plaintext host dump directory (`econ_graph.dump`,
  `econ_graph_app.dump`, `keycloak.dump`), or a retained legacy plaintext copy of the in-cluster
  backup volume ([POSTGRES_BACKUP_RESTORE.md](POSTGRES_BACKUP_RESTORE.md)):
  `latest.dump`, `latest-econ_graph_app.dump` and `latest-keycloak.dump`,
  or, without those links (an S3 copy), the newest `econ_graph-<stamp>.dump`
  and the app and Keycloak dumps of the same stamp.
- `--data FILE --keycloak FILE [--app FILE]`: the dumps themselves.

`--from-dir` and `--latest` refuse directories containing encrypted archives,
including mixed directories that retain older plaintext alongside ciphertext.
They do not fall back to a stale plaintext dump. Decrypt the selected backups
into a separate private staging directory and pass explicit paths instead.

Both app database layouts work. With `APP_DATABASE=shared` there is no app
dump: the app tables are in `econ_graph` and are sanitized there. With
`APP_DATABASE=separate` the app dump is given and sanitized on its own.

`--output-dir DIR` chooses the output directory. A run into a directory
that already holds a fixture of the same inputs and mode does nothing and
exits 0 if its actual dump size also meets the requested CI subset cap; any
other existing directory is refused. A lower cap can reject an existing subset
without replacing or deleting it.

The CI subset takes `--subset-series N` (default 300: that many series in
all, the same number from every source), `--subset-max-points N` (default
5000: a series with more observation rows than this, every vintage counted,
is never picked) and `--subset-max-kb N` (default 51200: a subset whose dumps
total more fails the run). The run prints the subset's size, and the
MANIFEST records it (`total_kib`).

## How it runs

1. The inputs are mounted read-only into a scratch `postgres:18` container
   with no network, on the **rootless** Docker daemon
   (`unix:///run/user/<uid>/docker.sock`, or `FIXTURE_DOCKER_HOST`). The
   script refuses any other daemon, so it never runs on the rootful daemon
   the cluster runs on. Nothing that holds a dump or backup is written,
   moved or deleted.
2. Each dump is restored into its own database (`pg_restore --no-owner
   --no-privileges`).
3. The sanitize steps (`scripts/fixtures/sql/steps.sql`) run on each database,
   in one transaction per database, with triggers off so timestamps keep
   their values.
4. Verification (below) runs on every database. Anything it finds fails the
   run.
5. With `--ci-subset`, the data database is cut down
   (`scripts/fixtures/sql/subset.sql`) and verified again.
6. Each database is dumped (`pg_dump --format=custom --no-owner
   --no-privileges`) into a scratch directory next to the output directory,
   with a MANIFEST and SHA256SUMS, and only then moved into place. A failed
   run removes its scratch directory and container and writes nothing to the
   output directory.

`scripts/fixtures/build-sanitized-fixture.sh --list-steps` lists the steps.

## What is scrubbed

Pseudonyms are keyed by a host-local secret,
`~/.config/econgraph/fixture-salt` (created 0600 on first use, or
`FIXTURE_SALT_FILE`): `h` below is the first 16 hex digits of sha256 of the
key and the lower-cased value. The same address gets the same pseudonym in
every table, every database and every fixture built with the same key, and
without the key a guessed address can't be checked against a pseudonym. The
MANIFEST records which key was used (`pseudonym_key_id`), not the key.

### App tables

| Table | Columns | Becomes |
|-------|---------|---------|
| `users` | `email` | `user-<h>@example.invalid` |
| | `name` | `User <h>`, h of the account's address |
| | `avatar_url`, `password_hash` | NULL |
| | `organization` | `Organization <h>` |
| | `provider_id` | kept when it is a UUID (a Keycloak subject), else `p-<h>` |
| `user_sessions` | every row | deleted |
| `audit_logs` | `user_name` | the pseudonym of the account (`User <h>`, as in `users.name`), or of the stored name when the account is gone |
| | `ip_address` | a stand-in in 10.0.0.0/8, the same for the same address |
| | `user_agent` | NULL |
| `security_events` | `user_email` | `user-<h>@example.invalid` |
| | `ip_address`, `user_agent` | as in `audit_logs` |

`user_id` columns are random UUIDs and stay, so audit rows, annotations and
grants still belong to the same (pseudonymous) users.

### Keycloak

| Tables | Becomes |
|--------|---------|
| `user_entity` | `email` and `email_constraint` as above; `username` the address's pseudonym when it is an address, else `user-<h>`; `first_name` `User` and `last_name` h, so the name in a token equals `users.name`; service accounts keep their names |
| `org_invitation` | first and last name as in `user_entity`; address as above |
| `user_attribute`, `fed_user_attribute` | every attribute but `locale` deleted |
| `credential`, `fed_user_credential`, `user_ver_credential`, `fed_user_ver_credential`, `issued_ver_credential`, `fed_issued_ver_credential` | deleted (passwords, OTP, WebAuthn, verifiable credentials) |
| `federated_identity`, `broker_link` | deleted (Google subject ids and any stored provider tokens) |
| `offline_user_session`, `offline_client_session`, `auth_session`, `root_auth_session`, `revoked_token`, `single_use_object`, `client_initial_access` (and the older `user_session`/`client_session*` tables where they exist) | deleted (user sessions, offline tokens, action tokens, registration access) |
| `event_entity`, `admin_event_entity`, `login_failure`, `outbox_entry` | deleted (login and admin events, brute-force records) |
| `client.secret` | `fixture-client-secret`; `client.registration_token` NULL; `client.secret.rotated*` attributes deleted |
| `identity_provider_config` `clientSecret`, `realm_smtp_config` `password`, `component_config` `bindCredential` | placeholders (`fixture-idp-client-secret`, `fixture-smtp-password`, `fixture-bind-credential`) |
| `component` and `component_config` of `org.keycloak.keys.KeyProvider` | deleted (realm signing and encryption keys, HMAC and AES secrets); Keycloak generates new keys on its first start |
| `server_config`, `jgroups_ping`, `cluster_event` | deleted (the cluster's JGroups certificate and key, cluster membership); Keycloak creates them again |

Realms, roles, clients (with their redirect URIs and scopes), client scopes,
groups, role mappings, group memberships, authentication flows and identity
provider definitions are kept. A fixture has no password for any account,
the master realm's admin included. To recover lab administration, stop all
Keycloak nodes and run `kc.sh bootstrap-admin user` with the same fixture
database settings used by the server. Use that temporary administrator to
create permanent lab access and users, then remove the temporary account.
Startup `KC_BOOTSTRAP_ADMIN_*` variables do not recover access when the master
realm already exists; follow Keycloak's
[admin recovery guide](https://www.keycloak.org/server/bootstrap-admin-recovery).

### Data tables

| Table | Becomes |
|-------|---------|
| `crawl_attempts` | `request_headers` and `response_headers` lose every header whose name contains auth, key, token, cookie, secret, session, signature or password |

### Every text and JSON column, in every database

- e-mail addresses outside `example.invalid` (also `%40`-encoded ones in
  URLs, which stay encoded) become their pseudonyms, wherever they are:
  annotation text, comments, audit `details`, series descriptions;
- JSON Web Tokens become `redacted.jwt.fixture`;
- the values of credential query parameters in URLs (`api_key=`, `token=`,
  `key=`, `UserID=`, `secret=`, `password=`, `sig=`, ...) become `REDACTED`.

Keycloak's Liquibase table `databasechangelog` is left as it is: its author
column holds Keycloak maintainers' addresses, and Keycloak identifies its
applied migrations by them.

Annotation, comment and chart text is otherwise kept as written.

## Verification

After sanitizing, and before anything is written, every database is
checked; any finding fails the run (exit status 1, nothing written):

- **The scan.** Every text, JSON and text-array column of every table is
  searched for e-mail addresses outside `example.invalid` and for
  secret-shaped values: JWTs, `-----BEGIN` (PEM blocks), runs of 32 or more
  base64url characters mixing upper case, lower case and digits, runs of 44
  or more base64 characters with a `+`, and runs of 32 or more hex digits
  mixing letters and digits. Content hashes of public files and HTTP
  validators (`etag`, `content_sha256`, `file_hash`, `xbrl_file_hash`) are
  searched for addresses only; Keycloak's `databasechangelog` not at all
  (`fixture_sanitize.exempt` in `scripts/fixtures/sql/functions.sql`).
- **Per-table checks** (`scripts/fixtures/sql/checks.sql`): every column in
  the tables above has the pseudonym's form, every table that is emptied is
  empty, every secret is its placeholder, no key provider is left.

A finding names the database, the table and column, the kind and the number
of rows, never the value:

```
VERIFICATION FAILED [econ_graph] FOUND: token in public.economic_series.description (1 rows)
```

A finding is never waved through by the script. Either a sanitize step is
missing (add it to `steps.sql`, with its check), or the value is not a
secret (add an exemption with its reason); both are reviewed changes.

## The output

```
~/econgraph-fixtures/20261028T060000Z/
  econ_graph.dump        # pg_dump --format=custom, sanitized
  econ_graph_app.dump    # only with APP_DATABASE=separate
  keycloak.dump
  MANIFEST
  SHA256SUMS
```

The directory is 0700 and its files 0600. `MANIFEST` is `key=value` lines:
the mode and subset parameters; for each input its file
(`<directory>/<file>`), size, mtime, sha256, the database it was dumped from
and when (`archive_created`, from the dump's own header), and which kinds of
tables it held; for each output its size and sha256; and the row count of
every table (`rows.<database>.<schema>.<table>=<n>`). `SHA256SUMS` covers the
dumps and the MANIFEST.

Restore a fixture with `pg_restore --no-owner --no-privileges` into empty
databases (for Keycloak, before Keycloak first starts on it).

## Review before use

A fixture built from real dumps is reviewed by the operator before anything
uses it, and before any CI subset leaves the host: read the run's log and
MANIFEST, restore it into a scratch database, and look at the tables that
hold free text (annotations, comments, audit `details`, Keycloak's realm and
client attributes) for anything personal the scan can't recognise, such as
names in free text. The first fixture from real dumps gets this review in
full; after that, review what changed (new tables, new columns, findings).

## Tests

`scripts/fixtures/test-build-sanitized-fixture.sh` builds synthetic dumps
from `backend/migrations` and from a real Keycloak (the version the cluster
deploys, with `config/keycloak/econ-graph-realm.json`), with planted
addresses, names, JWTs, API keys, client secrets, passwords, sessions,
offline tokens, events and keys, and checks that:

- none of them is left in any output, in either app database layout or in
  the CI subset;
- pseudonyms agree between the app database and Keycloak;
- Keycloak starts on the sanitized database and serves the realm with new
  keys;
- leaving out any single sanitize step makes verification fail the run with
  nothing written, as does a secret no step covers;
- `--dry-run` starts no container and writes nothing; a second run is a
  no-op when its dumps meet the requested cap; a stricter cap refuses an
  oversized existing subset without changing it; a daemon that isn't rootless
  is refused; no input changes.

It runs in CI (the "Sanitized Fixture Builder" job) and needs only Docker.
Run it locally with `FIXTURE_DOCKER_HOST` set to the rootless socket and
`TMPDIR` on a disk with a few GB free.

The suite first runs `scripts/fixtures/test_fixture_reuse.py`, an offline
full-script check using synthetic files, a Docker-info stub and an uncontended
lock stub. It exercises checksum/idempotency behavior and actual dump byte
limits, and confirms encrypted inputs are refused before Docker access, without
starting containers or restoring data. The required workflow
lint job also checks that sanitizer failures and cancellations fail Core CI
Result, and that the merge queue cannot pass a skipped sanitizer job.
