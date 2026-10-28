# /k8s-deploy

Apply Kubernetes manifests from `k8s/manifests` (or a provided path).

- path (string, optional): manifests path (default: `k8s/manifests`)
- namespace (string, optional): passed to `kubectl apply --namespace`; must match any explicit `metadata.namespace` in the selected manifests.

If namespace is omitted, leave out `--namespace`. Preserve manifest namespaces and use the current context's namespace (or `default` if unset) for namespaced resources without one.

```bash
/k8s-deploy k8s/manifests econ-graph
```
