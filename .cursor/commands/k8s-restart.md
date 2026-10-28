# /k8s-restart

Restart a Kubernetes deployment and wait for rollout.

- deployment (string, required): deployment name
- namespace (string, optional): target namespace; if omitted, use the current kubectl context's configured namespace, or `default` if none is configured

Use the current kubectl context. Before restarting, state the selected context,
namespace, and deployment. Restart only that deployment and wait for its rollout
to complete, using the same context and namespace for both operations.

```text
/k8s-restart econ-graph-backend econ-graph
```
