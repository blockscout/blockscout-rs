# ADR-017: Cluster-Scoped Read API

**Date:** 2026-11-03

**Authors:** @EvgenKor

**Status:** Accepted

## Context

Frontends pass the same filter set (`home_chain_id`, `counterparty_chain_ids`,
`bridge_ids`, `include_unindexed_chains`) on every Read API call, and each one
has to keep that set in sync with the others. `multichain-aggregator` already
exposes clusters (named server-side presets selected by a URL segment), so a
frontend that talks to both services expects the same shape from this one.

The legacy filter vocabulary cannot express an empty set: an empty or absent
list means "no restriction". A preset that must bound a chain directory or a
per-chain statistic to the cluster's chains therefore cannot be written as a
translation into the legacy query params.

## Decision

- **Presets.** Clusters are named presets loaded at startup from an optional
  JSON file plus `INTERCHAIN_INDEXER_CLUSTERS*` environment overrides, and
  validated at startup, before the stats backfill and DB seeding (see the
  README, "`clusters.json`").
- **Routes.** 13 routes under `/api/v1/clusters/{cluster_id}/` (plural, as in
  `multichain-aggregator`): the six message/transfer lists, the chain and bridge
  directories, message details, `stats/chains`, and the per-chain bridged tokens
  and message paths.
- **Universe semantics.** A cluster route never returns rows outside the
  cluster. Chain directories intersect the cluster chain set with the client's
  `chain_ids`; per-chain stats derive the focal and peer chains from the cluster
  and reject a path chain outside it with `InvalidArgument`; an empty
  intersection is an empty list, never "all". Message details apply the cluster
  filter inside the lookup, before the two-candidate ambiguity probe, so a
  message outside the cluster is a 404 and rows outside it never make a lookup
  ambiguous.
- **One core per endpoint.** A typed `ReadScope` (`services/scope.rs`) is shared
  by the legacy and cluster entry points. Each filtered endpoint is a private
  core taking the scope; the legacy trait method and the cluster trait method
  are thin entries that build the same inputs and call it. The cluster lookup
  runs first in every cluster entry point.
- **Two cluster services.** `cluster.proto` defines
  `InterchainClusterService` and `InterchainClusterStatisticsService`, with
  `GetCluster*Request` messages that carry `cluster_id` and never declare
  `home_chain_id`, `counterparty_chain_ids` or `include_unindexed_chains`.
  Every message whose legacy twin has `bridge_ids` declares it too, as a client
  selection narrowed inside the cluster's bridges, because the frontend
  filters its lists by bridge. Two services could share response messages only
  after the unused actix-prost `conversions` feature was dropped.
- **Behavior at the edges.** An unknown cluster is gRPC `NotFound` / HTTP 404
  (`cluster not found: <id>`). The other three cluster fields are not
  parameters, so sent as query params they are silently ignored (the generated
  routes drop unknown params) and the result stays cluster-scoped. A
  `bridge_ids` selection runs as `selection ∩ cluster bridges` (as sent when
  the cluster sets none). A disjoint one returns an empty response, never "all
  bridges", and ids outside the cluster are ignored, not rejected. Page tokens
  are not bound to a cluster. The counts of `/stats/chains` are global or per-bridge, not
  cluster-scoped.

## Alternatives Considered

### Alternative 1: A second HTTP binding of the same RPC

Bind `/api/v1/clusters/{cluster_id}/…` to the existing RPCs as an additional
route.

**Pros:**
- No new messages or services.

**Cons:**
- Impossible in actix-prost 0.3: one RPC takes one HTTP rule.

### Alternative 2: New RPCs that reuse the legacy request messages

**Pros:**
- No new request messages.

**Cons:**
- `cluster_id` would leak onto the legacy routes and their swagger, and the
  four cluster fields would be declared on the cluster routes, so the preset
  would be client-overridable.

### Alternative 3: URL rewrite in middleware or ingress

**Pros:**
- No service code.

**Cons:**
- No swagger entries, no TypeScript types, no gRPC, and the rewrite still
  needs the vocabulary this decision found inexpressible.

### Alternative 4: A translation adapter into the legacy requests

**Pros:**
- The cluster routes are a thin layer over the legacy handlers.

**Cons:**
- Universe semantics do not fit the "empty means all" vocabulary: an empty
  intersection would turn into an unrestricted legacy request.

### Alternative 5: Mechanical semantics

Apply the cluster's fields with the legacy filter semantics, with no universe
derivation and no rejection of a path chain.

**Pros:**
- No derivation table.

**Cons:**
- Correct only by coincidence of the current bridge topology.

### Alternative 6: Extend the `build.rs` dedupe patch, or put the cluster RPCs into the existing services

**Pros:**
- No new service pair.

**Cons:**
- The patch is fragile code that the `conversions` removal made unnecessary,
  and the existing services would mix two request vocabularies and one swagger
  tag.

## Consequences

### Positive

- A frontend selects its slice with the URL alone, and the cluster routes have
  their own swagger entries, TypeScript types and gRPC methods.
- Filter semantics live in one place (`ReadScope`), so the legacy and cluster
  routes cannot drift apart.

### Negative

- A new filtered endpoint must get both entry points, a `GetCluster*Request`
  message and an HTTP rule.
- `conversions` must not come back: re-enabling `conversions` would again emit
  duplicate `*Internal` structs as soon as two services share a response
  message, which the cluster services do.
- The startup check rejects a cluster chain that only a removed bridge
  observed, unless `include_unindexed_chains = true`.

### Neutral

- Cluster configuration is read once at startup; there is no runtime reload.

## References

- `interchain-indexer-server/src/services/scope.rs`
- `interchain-indexer-proto/proto/v1/cluster.proto`
- README, "Cluster-Scoped Read API" and "`clusters.json`"
- `.memory-bank/rules/rust-style.md`, "Filtered Read API Endpoints Have Two Entry Points"
- `.memory-bank/gotchas.md`, "Unknown Query Params Are Silently Ignored by Generated HTTP Routes"
