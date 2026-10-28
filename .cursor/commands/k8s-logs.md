# /k8s-logs

Tail logs for a deployment's pods (last 200 lines, follow) in the current kubectl context's cluster.
Find pods using `app=<deployment>`; if none match, try `app.kubernetes.io/name=<deployment>`.

- deployment (string, required): deployment name used as the label value
- namespace (string, optional): target namespace; when omitted, use the current kubectl context's namespace, or `default` if unset

```text
/k8s-logs econ-graph-backend econ-graph
```
