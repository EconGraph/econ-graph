# /debug-k8s

Ask the agent to investigate a Kubernetes failure, isolate its cause, and verify one targeted repair.

Include:

- Namespace and deployment names
- Symptoms (crashloop, readiness failing, rollout stuck)

EconGraph uses GatewayClass `econ-graph` and Gateway `econ-graph-gateway` in namespace `econ-graph-gateway`. Application HTTPRoutes live in `econ-graph`; the Envoy Gateway controller and proxy run in `envoy-gateway-system`. Use the [Gateway configuration](../../k8s/README.md#-gateway) for the routing inventory and [Gateway routing troubleshooting](../../docs/deployment/GATEWAY_ROUTING_TROUBLESHOOTING.md) for detailed checks.

Agent loop:

- Verify context/namespace; list all relevant components to build a full picture:
  - Deployments, StatefulSets, DaemonSets, Jobs/CronJobs
  - GatewayClass, Gateway, HTTPRoutes, Services, Endpoints/EndpointSlices; include the Gateway and Envoy namespaces when investigating routing
  - EnvoyProxy and relevant ClientTrafficPolicy, BackendTrafficPolicy and EnvoyPatchPolicy resources
  - ConfigMaps, Secrets, HPAs, PDBs, NetworkPolicies
  - List pods with owner refs to map components → pods
- For each component/pod, gather targeted evidence before hypothesizing:
  - `kubectl describe` for events, conditions, probe failures
  - For routing failures, check GatewayClass `Accepted`, Gateway `Accepted`/`Programmed`, listener conditions and attached-route counts, and HTTPRoute `Accepted`/`ResolvedRefs` for the intended parent. Inspect rejected or unprogrammed Envoy policies before tracing the backend.
  - Collect the last N log lines from the current instance of each container in each relevant pod. For restarted containers, also collect a bounded tail from the previous terminated instance (`kubectl logs --previous --tail=N`).
  - Replicas vs available, restart counts, image tags/digests, resources/limits
  - Probe configs (readiness/liveness/startup) and recent failures
- Bisect the path to isolate the failing layer before forming a hypothesis:
  - Client → Envoy proxy (Gateway listener and HTTPRoute match) → Service → ready EndpointSlices/Pods → Application → external dependencies (DB, APIs)
  - Verify the request's host and path match the intended HTTPRoute and listener; check route namespace admission and backend references
  - Port-forward to probe health/ready endpoints directly; `kubectl exec` to run curl/env checks in-pod
  - Confirm Service selectors match pod labels; confirm EndpointSlices contain ready endpoints
- Only after isolating the failing component, form ONE clear hypothesis
- Make ONE targeted change (small manifest tweak, config fix, rollout restart), apply, and watch rollout to completion
- Collect before/after evidence; if unresolved, revert or iterate with a new focused hypothesis; document findings

Example Cursor invocation (namespace, deployment, symptom):

```text
/debug-k8s econ-graph econ-graph-backend "Readiness probe failing after deploy; check logs, events, and probes; iterate safely."
```
