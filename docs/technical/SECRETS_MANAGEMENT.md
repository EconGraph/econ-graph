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

Confirm the intended cluster before running either script. `create-secrets.sh`
uses the selected kubectl context and namespace (`econ-graph` by default). For a
direct run, cached kind copies are the default when available and no kubectl,
kubeconfig, or pinned context is specified. The interactive wrapper selects the
kind cluster's kubectl and context.

For crawler API keys, from the repository root:

```bash
scripts/deploy/set-crawler-keys.sh
```

The script prompts with hidden input for FRED, BLS, BEA, and Census keys and passes
them to `create-secrets.sh` through its environment. Enter keeps the existing
value. Values are never printed or passed as command arguments. Only the selected
mode's credentials are inherited by `create-secrets.sh`.

In crawler mode, an existing crawler worker deployment is scaled to one replica
and restarted afterwards. Use `--no-start-crawler` to enter keys without starting
it. `--google` also prompts for the Google identity-provider client. If credentials
are entered and Keycloak is deployed, the script restarts Keycloak and reruns its
realm import.

`--tunnel` is a separate run that prompts only for the Cloudflare Tunnel connector
token and leaves the crawler alone. It refuses before prompting unless the cluster
is marked production. After a token is entered, it starts or restarts an existing
`cloudflared` deployment for the cluster's public host, rechecking the production
marker and token Secret. Without a public host, it leaves the connector at zero
replicas; without a deployment, startup is left to `deploy.sh`. See the
[public-site guide](../../k8s/README.md#-public-site-cloudflare-tunnel).

`set-crawler-keys.sh --dry-run` reads key presence and shows the intended actions,
without prompting or writing secrets. It still queries the cluster. It is a
human-run check, not an offline mode. `create-secrets.sh --dry-run` also performs
real cluster checks and resolves existing values without printing them or writing
Secrets.

`create-secrets.sh` generates missing database, Keycloak, and Grafana passwords, preserving
existing values on later runs. Optional unset or empty inputs keep the existing
value, or leave a missing key absent. Obvious placeholders are refused. Values
reach kubectl through a private temporary directory and server-side apply; they
do not enter command arguments or the last-applied-configuration annotation.
`create-secrets.sh` stops if the cluster cannot be queried or if it finds a database volume
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
same protection as the credentials they contain. This is a handling rule: the
scripts use the selected kubeconfig's Kubernetes permissions and do not enforce a
separate agent access boundary.
