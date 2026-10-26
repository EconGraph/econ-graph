# Secrets management

Kubernetes Secrets are created by a human operator with
[`create-secrets.sh`](../../scripts/deploy/create-secrets.sh).
[`deploy.sh`](../../scripts/deploy/deploy.sh) invokes that script on each deploy.
There is no Sealed Secrets controller or secrets submodule in this repository.
Do not commit credentials, plaintext Secret manifests, decryption keys, or secret
values in documentation, issue comments, logs, or command arguments.

## Current workflow

Run secret setup yourself in your terminal. Agents may edit scripts and review
non-secret configuration, but must never read, enter, copy, or decrypt secret
values. Do not paste them into an agent session or ask an agent to run the secret
setup commands. Kubernetes base64 encoding is not encryption.

For crawler API keys, from the repository root:

```bash
scripts/deploy/set-crawler-keys.sh
```

The script prompts with hidden input for FRED, BLS, BEA, and Census keys and passes
them to `create-secrets.sh` through its environment. Enter keeps the existing
value. Values are never printed or passed as command arguments. Only the selected
mode's credentials are inherited by `create-secrets.sh`.

By default the crawler worker is scaled to one replica and restarted afterwards.
Use `--no-start-crawler` to enter keys without starting it. `--google` also prompts
for the Google identity-provider client and restarts Keycloak and reruns its realm
import after new credentials are entered. `--tunnel` is a separate run that prompts
only for the Cloudflare Tunnel connector token. It refuses before prompting unless
the cluster is marked production; see the [public-site guide](../../k8s/README.md#-public-site-cloudflare-tunnel).

`set-crawler-keys.sh --dry-run` reads key presence and shows the intended actions,
without prompting or writing secrets. It still queries the cluster. It is a
human-run check, not an offline mode. `create-secrets.sh --dry-run` also performs
real cluster checks and resolves existing values without printing them or writing
Secrets.

`create-secrets.sh` uses the selected kubectl context and namespace (`econ-graph`
by default). The interactive wrapper selects the kind cluster's kubectl and
context. Confirm the intended cluster before running either script.

The script generates missing database, Keycloak, and Grafana passwords, preserving
existing values on later runs. Optional unset or empty inputs keep the existing
value, or leave a missing key absent. Obvious placeholders are refused. Values
reach kubectl through a private temporary directory and server-side apply; they
do not enter command arguments or the last-applied-configuration annotation.
The script stops if the cluster cannot be queried or if it finds a database volume
without its matching password Secret. Do not bypass that guard to repair a lost
password: follow the [database recovery guidance](../../k8s/README.md#secrets).

The [Secrets table](../../k8s/README.md#secrets) lists Secret names, keys, environment
variables, and consumers. Machine-only database passwords stay generated in the
cluster. Store human login credentials, such as the Keycloak and Grafana admin
passwords, in the operator's password manager. Changing a Kubernetes password
Secret does not itself change a password in an existing database or Grafana
volume; coordinate rotation in the service too.

## Secret tiers

| Tier | Examples | Handling |
| --- | --- | --- |
| T1: free data API keys | FRED, BLS, BEA, Census | Human entry through the hidden-input wrapper. Only these keys may be added to GitHub Actions secrets for source integration tests; never use workflow inputs for values. |
| T2: application and service credentials | Database passwords, Keycloak and Grafana logins, Google identity-provider client secret | Keep in cluster Secrets; keep human logins in the password manager. Do not put them in GitHub Actions secrets or public git. |
| T3: infrastructure and recovery credentials | Cloudflare tunnel/DNS tokens, decryption keys, cluster-admin credentials | Operator-only handling, least privilege and recovery storage; never GitHub Actions secrets or public git. A tunnel token belongs only on the marked production cluster. |

Agents never handle values from any tier. Secret dumps, database dumps containing
identity-provider credentials, Terraform state, and recovery keys require the
same protection as the credentials they contain. The agent kubeconfig restriction
is tracked in [ECO-444](https://linear.app/econgraph/issue/ECO-444); this document
does not imply that RBAC isolation has already been implemented.

## Encrypted configuration

SOPS with age is the accepted replacement for the abandoned Sealed Secrets design,
not the deployed workflow. Implementation and status live in
[ECO-437](https://linear.app/econgraph/issue/ECO-437), including its accepted secrets
amendment. Ciphertext will live in a separate private secrets repository, accessed
by Flux through a second GitRepository with a read-only deploy key, without a
public-repository submodule.

The amendment specifies two recipients: a cluster key held in `flux-system/sops-age`
and an operator recovery key protected by a passphrase and backed up in the
password manager and on paper. Agents receive neither private key. Machine-only
passwords remain generated in-cluster, avoiding a second source of truth. The
private repository will check SOPS metadata and scan for leaked plaintext before
commit. Recovery is to be drilled in a fresh kind cluster each quarter. These are
requirements for that future workflow; installing or running SOPS is not part of
today's secret setup.
