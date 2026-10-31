# Independent data access through an explicit operation-routing proxy

Status: selected for ECO-186 before implementation.

The app and persistent data services keep their existing GraphQL contracts. Select
an explicit same-origin HTTP proxy with `/graphql` for app operations and
`/data/graphql` for data operations, rather than introducing federation entities or
a gateway that rewrites selections. The existing frontend already classifies
operations and sends each to its owning service. Consumers compose an app read and
a data read in their application; a single cross-plane GraphQL selection is not
silently split or given partial authorization. This preserves Train 3's public
operations and avoids cross-plane SQL joins or duplicating observation logic.

The proxy is not an authorization boundary. Each backend verifies its own tokens.
An explicitly configured independent data service requires the data audience and
a catalog capability, fails closed if the issuer is unavailable, and executes only
allowlisted read roots. Mutations, crawler control, app administration and MCP are
unavailable through this entry point even if a token carries staff capabilities.
Ordinary app/combined deployments keep their current defaults; enabling this
profile is an explicit deployment choice.

A development-only read client receives only the data audience and catalog read
capabilities. It can use the persistent data service while the app connects to a
fresh local app database. It receives no shared database credential, app audience,
crawler capability or service-account privilege. Production browser clients may
receive both audiences only through an explicitly installed realm overlay.

The working Postgres observation implementation remains behind TimeSeriesStore;
the proxy never reads tables or implements observation transformations itself.
Deployment acceptance against the new self-hosted platform is a sibling check and
must be reported separately from local HTTP, token and Postgres tests.

The production candidate preserves its current combined public facade and browser
routes. The additional signed data entry is `/independent-data/graphql`; it does
not replace `/data/graphql`. Strict app mode is an explicit independent-consumer
choice, not a default browser rollout. The public facade retains request-scoped
catalog authorization while the independent service refuses anonymous access.
