# Service Write API: Approaches, Access Control, and Audit

## Scope

This note covers how to design a **write API** for this service: endpoints that
let a trusted outside party change the service's data or state without connecting
to its database directly. It is a catalogue of options and their trade-offs, plus
the precedents found in the monorepo and the wider Blockscout ecosystem. The
catalogue itself does not choose defaults; interchain-indexer's v1 choices are
recorded in ADR-016 and summarized in "Chosen for interchain-indexer (v1)".

The primary caller is assumed to be a **human operator** handling a class of
operational problems. Such calls are rare and privileged, and a key is enough to
authenticate them. Methods for **automation** may be added to the same API later,
so the catalogue also covers what keeps that extension cheap.

In scope:

- where write endpoints live and how callers reach them
- the shape of the contract: commands vs resources, validation, idempotency, concurrency
- authentication, key management, and authorization granularity
- how a change is applied relative to the process that owns the data
- audit and observability of write actions
- precedents in other blockscout-rs services and in the Blockscout backend
- properties of interchain-indexer today that constrain the choice

Out of scope:

- concrete write methods and use cases (examples such as token metadata
  overrides or checkpoint adjustments are illustrations only)
- runtime topology changes (chains/bridges/contracts hot reload), see
  `config-loading-and-validation.md`
- the indexing data path itself

Statements are marked where it matters: **fact** means verified in code or in a
cited document, **inference** means derived from code but not exercised, and
**assessment** means a judgement drawn from external guidance.

## Short Answer

- **Writes go through the owning service.** Every source agrees that an outside
  party should not write to the service's database directly. In this service it
  is also concretely unsafe: several tables are owned by running in-memory state
  whose upserts silently revert outside edits (see Failure Modes).
- **The ecosystem convention for a trusted write is a static key.** It is sent in
  the `x-api-key` header and configured as a named map in env. The check happens
  in the handler, and a failure returns `UNAUTHENTICATED`/401. For an
  operator-first API this is sufficient. The known weaknesses of the existing
  implementations are all cheap to avoid:
  - key names are discarded, so the caller cannot be identified
  - keys are stored and compared in plaintext, and the comparison is not
    constant-time
  - no other service logs which key acted; interchain-indexer now records the
    key name as the actor of every applied change (ADR-016)
- **Design choices that change cost later:**
  - keep the key's name as the caller's identity
  - allow an optional role or scope per key
  - write an audit row in the same transaction as the mutation
  - put write methods in a separate proto service
- **Caller class decides the authorization model:**
  - operator: none, or a single role
  - control-plane automation: per-key role
  - data-source ingestion: per-key data scope, as in multichain-aggregator's
    per-chain keys

## Why This Matters

- Without a write API, operational fixes are made with manual SQL. That gives
  standing broad DB access, skips domain validation, leaves no attributable audit
  trail, and races running in-memory state.
- The service was read-only, with no auth at all, until ADR-016. The first write
  endpoint sets precedents for authentication, logging, and swagger exposure
  that later methods inherit.
- The four existing blockscout-rs write paths diverge in where the key travels,
  where it is stored, how it is scoped, and which status codes they return.
  Copying any one of them without reviewing it also copies its gaps.

## Source-of-Truth Files

Monorepo precedents:

| File | Role |
|---|---|
| `stats/stats-server/src/auth.rs` | `AuthorizationProvider`: `x-api-key`, any-of-values match, `unauthenticated` error |
| `stats/stats-server/src/server.rs` (`init_authorization`) | Fail-closed with a warning when no keys are configured |
| `stats/stats-server/src/read_service.rs` (`batch_update_charts`) | Check inside the handler; the command is queued to the in-process update service |
| `stats/stats-server/tests/it/mock_blockscout_reindex.rs` | The only negative auth tests in the monorepo (missing key and wrong key → 401) |
| `eth-bytecode-db/eth-bytecode-db-server/src/services/mod.rs` (`is_key_authorized`) | `x-api-key` checked against a `HashSet`; a non-ASCII value returns 400 |
| `eth-bytecode-db/eth-bytecode-db/verifier-alliance-migration/src/migrations/initialize_schema_v1_up.sql` | `created_by`/`updated_by` default to `current_user`, maintained by triggers (provenance by DB role) |
| `multichain-aggregator/multichain-aggregator-logic/src/services/api_key_manager.rs` | Per-chain key lookup, and a global metadata key |
| `multichain-aggregator/multichain-aggregator-migration/src/initial/up.sql` | `api_keys` table: `key UUID UNIQUE`, `chain_id` FK, `created_at` |
| `service-template/{{project-name}}-proto/proto/v1/{{project_name}}.proto` | Template default: `ApiKeyAuth` on `x-api-key`, attached to the example `Create` RPC (the generated handler does not check it) |
| `libs/blockscout-service-launcher/src/launcher/launch.rs` | `launch(settings, http, grpc)`: one HTTP listener, one gRPC listener, and the metrics listener |
| `libs/blockscout-service-launcher/src/launcher/span_builder.rs` | HTTP root span; the fields come from `blockscout-tracing-actix-web` `root_span!` |
| `libs/blockscout-service-launcher/src/launcher/settings.rs` | `max_body_size` default of 2 MiB; CORS settings |

interchain-indexer:

| File | Role |
|---|---|
| `interchain-indexer-proto/proto/v1/interchain_indexer.proto` | Declares `ApiKeyAuth` (`x-api-key`, header); the operations of `admin.proto` reference it, no read RPC does |
| `interchain-indexer-proto/proto/v1/api_config_http.yaml` | Every HTTP route is a `get`, except the admin `post` routes |
| `interchain-indexer-server/src/server.rs` (`Router::grpc_router`, `register_routes`) | Services are registered whole; the gRPC builder has no tracing layer or interceptor |
| `interchain-indexer-logic/src/secret.rs` | `Secret<T>` (redacting `Debug`, no `Display`/`Serialize`), `redact_urls`, `sanitize_transport_error` |
| `interchain-indexer-logic/src/message_buffer/persistence.rs` (`upsert_cursors`) | Monotonic `GREATEST`/`LEAST` conflict rules on `indexer_checkpoints` |
| `interchain-indexer-logic/src/database.rs` (`upsert_chains`, `upsert_bridges`, `mark_catchup_complete`) | Config seeding overwrites rows at startup; `upsert_bridges` disables every bridge before re-enabling those in config |

## Key Types / Tables / Contracts

### Caller Classes

| Class | Example | Profile | What a key is bound to |
|---|---|---|---|
| Operator | the future interchain-indexer write API | rare, manual, privileged | nothing, or an `operator` role |
| Control-plane automation | a provisioning platform registering instances | rare, programmatic | a role or source tag per key |
| Data-source ingestion | multichain-aggregator `import:batch` from Blockscout instances | continuous, high volume | a data scope, e.g. `chain_id` |

The ingestion class is a different problem from admin writes. multichain-aggregator
binds each key to a chain so that one Blockscout instance cannot import data
attributed to another chain by mistake. This is object-level authorization (OWASP
API1), not a privilege tier.

### Precedent Catalogue

| Service | Write endpoint(s) | Class | Key transport | Key storage | Scope | Failure | Who acted, recorded? |
|---|---|---|---|---|---|---|---|
| stats | `POST /api/v1/charts/batch-update` | operator / automation command | `x-api-key` header | env map `STATS__API_KEYS__<NAME>`; names unused at check time | none | 401; fail-closed with a startup warning when no keys exist | no; the request payload is logged |
| eth-bytecode-db | alliance `…:batch-import` (hard gate); `verify-*` (soft gate: a key makes caller-supplied deployment data trusted) | automation | `x-api-key` header | env `ETH_BYTECODE_DB__AUTHORIZED_KEYS__<name>__KEY`, collapsed to a `HashSet` | none | 401; non-ASCII value returns 400 | `is_authorized` flag in logs; verifier-alliance rows carry the DB role |
| multichain-aggregator | `POST /api/v1/import:batch` | source ingestion | `api_key` body field | DB table `api_keys` (plaintext UUID; no revoke, expiry, or last-used columns; no management tooling) | per chain | 403 (`PERMISSION_DENIED`); malformed UUID returns 400 | no; metric per `(chain_id, entity_type)` only |
| multichain-aggregator | `POST /api/v1/import:poor-reputation-tokens` | metadata automation | `api_key` body field | single env setting `service.metadata_import_api_key` | global | 403 | no |
| Blockscout backend (Elixir) | `/api/v2/import/*` (token info, smart contracts, audit reports) and admin refetch-style endpoints | admin services | mixed: `x-api-key` header on some routes, `api_key` param (query or body) on others | single env `API_SENSITIVE_ENDPOINTS_KEY` | global | 401 for a wrong key; 403 when no key is configured | no audit table; curated token info sets an override flag (see below) |
| sig-provider | `POST /api/v1/signatures` | anyone | none | — | — | — | no |

Blockscout's internal admin services, which live outside this repo, follow the
same `x-api-key` convention and go further in four ways. They keep the key's name
as the caller identity. They attach a role or level, and optionally a data scope,
to each key. They stamp the writing key's name on the rows they own, so a
lower-level writer cannot overwrite a higher-level writer's rows. They also
distinguish 401 from 403.

De-facto conventions across all of the above (fact):

1. Header `x-api-key`, documented in proto as a `security_definitions` entry
   named `ApiKeyAuth`.
2. Keys are configured as a named env map. Names are cosmetic today: they are
   discarded before the comparison.
3. The check runs inside each handler. There is no middleware or tonic
   interceptor anywhere.
4. Comparison uses plain `==` or `HashSet::contains`; nothing uses a
   constant-time compare (no `subtle` crate in the repo). interchain-indexer
   follows this too: it compares SHA-256 digests with `==`, which leaves no
   timing signal about the key.
5. When no keys are configured, the endpoint rejects every call.
6. No other service records which key performed an action. interchain-indexer
   records the key name in `write_api_audit_log` (ADR-016).

## Step-by-Step Flow

A write request passes through the steps below. Each step is a decision axis,
listed with its options and trade-offs.

### 1. Reachability

| Option | Trade-offs |
|---|---|
| Public listener and ingress, key only | What every precedent does. Cheapest. The key is the only barrier, and write methods are as discoverable as read methods. |
| Same listener, admin path prefix (e.g. `/api/v1/admin/…`) blocked at the public ingress | Cheap. The protection depends on ingress configuration that lives outside the repo. gRPC exposes whole services regardless of HTTP paths, so the gRPC listener needs the same treatment. |
| Separate internal listener (admin port), not routed by the public ingress | The separation recommended for management endpoints (Envoy admin, Spring `management.server.port`, Dropwizard admin port; OWASP REST cheat sheet). **Fact:** the launcher supports exactly one HTTP listener plus gRPC and metrics, and no monorepo service runs a second one. This needs a custom actix server sharing graceful shutdown, or a launcher extension. |
| Internal-only ingress or VPN | Network policy outside the service. Depends on what the infrastructure offers. |
| No ingress at all; operators use `kubectl port-forward` to the internal port | Kubernetes RBAC (`pods/portforward`) becomes the outer gate. The Kubernetes audit log records the port-forward, not the requests sent through it, so in-app audit remains necessary. |

Assessment: a network boundary is defense in depth, not access control. OWASP
API5 and NIST SP 800-207 both reject trust based on URL or network location. The
key check stays mandatory whichever option is chosen.

### 2. Authentication

| Option | Trade-offs |
|---|---|
| Static API key in a header | Ecosystem convention; trivial for operators (curl, swagger). Must never travel in a URL (RFC 6750 §2.3: URLs get logged by proxies and access logs). OWASP API2: acceptable for client authentication, not for user authentication. |
| HMAC request signing (RFC 9421; AWS SigV4; GitHub webhooks) | The secret never travels, and the body is integrity-protected. Adds canonicalisation work and clock-skew handling. Overkill for an operator. |
| OAuth2 client credentials / short-lived JWT (RFC 6749 §4.4, RFC 9700) | Short-lived tokens with central revocation. Requires an identity provider, which no ecosystem service uses today. |
| mTLS or mesh identity (RFC 8705; Istio SPIFFE) | The application holds no secret. Requires a mesh or PKI, and the caller identity must be forwarded to the app for audit. |
| Kubernetes ServiceAccount token + `TokenReview` | Automation only, and only in-cluster. The app must re-read the rotated token file. |
| Human SSO in front (oauth2-proxy, IAP) | Gives a real human identity for audit. Requires a proxy deployment. Fits if an admin UI ever appears. |
| Basic auth | A random secret sent over TLS is effectively an API key, but the scheme invites human-password habits and has no scoping (RFC 7617 §4). |

### 3. Key Management

| Decision | Options and trade-offs |
|---|---|
| Granularity | One shared operator key: simple, but the audit trail can only say "operator". Personal keys: per-person attribution, but keys must be revoked when people leave. A named env map supports both without code changes, provided the name survives into the request context. |
| Storage location | Env (Kubernetes Secret): simple; rotation needs a restart. DB table: runtime management, but needs management methods. Without them, keys are inserted with manual SQL, which is the opposite of this API's purpose; multichain-aggregator is in this state. |
| Storage form | Plaintext: what all precedents do. SHA-256 digest: NIST SP 800-63B-4 §3.1.2.2 requires hashed storage of look-up secrets and requires a slow hash only for secrets under 112 bits. A leaked config then cannot be replayed. |
| Format | ≥128 bits from a CSPRNG. An identifying prefix (GitHub `ghp_`, Stripe `sk_live_`) enables secret scanning and tells you which service a leaked key belongs to. |
| Rotation | At least two active keys per caller, so a new key can be rolled out before the old one is removed (Stripe's guidance). A named map already allows this. |
| Comparison | Constant-time compare (CWE-208). Matters most for a single global key. Hashing first and comparing digests also removes the timing signal. |
| Unconfigured state | Fail-closed, with a startup warning (stats does this). |

### 4. Authorization

| Option | Trade-offs |
|---|---|
| Any valid key may call any write method | Enough for an operator-only API. Adding automation later means retrofitting scopes. |
| Optional role or level per key (absent means operator, full access) | Small schema addition that keeps automation cheap to add. Precedent: the role or source tag in the ecosystem admin services. Deny by default for methods not granted to a role (OWASP API5). |
| Data scope per key (e.g. allowed `chain_id`s or `bridge_id`s) | Needed only for the ingestion class. Every object touched must be checked (OWASP API1). multichain-aggregator shows how scoping leaks when payloads carry their own chain IDs (see Edge Cases). |

Status codes: 401 / `UNAUTHENTICATED` when the key is missing or invalid; 403 /
`PERMISSION_DENIED` when the key is valid but the method or scope is not granted
(RFC 9110 §15.5.2 and §15.5.4). A strict 401 also requires a `WWW-Authenticate`
header.

### 5. Contract Shape and Validation

- **Commands vs resources.** Use custom methods (`POST /api/v1/<resource>:<verb>`,
  Google AIP-136) for operational actions, and resource-style `PUT`/`PATCH` for
  curated values. Both styles already exist in the monorepo (`import:batch`,
  `charts/batch-update`).
- **Separate proto service** for write methods. It gets its own swagger file,
  which keeps write methods out of the public API docs while still documenting
  them (OWASP API9). The service can then be registered on a different listener,
  or not at all, without touching read services. **Fact:** tonic `add_service`
  exposes every RPC of a service, so mixing write RPCs into a read service would
  publish them on gRPC as well.
- **Validate before commit.** A `validate_only` / dry-run flag (AIP-163;
  Kubernetes server-side dry-run) runs the same checks and returns the would-be
  result. This is especially valuable for operators.
- **Concurrency.** Expected-old-value compare-and-set or a version column
  (optimistic offline lock). On mismatch, return 409 / `ABORTED`. This protects
  the operator's write only. It does not protect against the owner process's
  next blind write; step 6 does.
- **Idempotency.** Natural keys with upsert semantics need no extra machinery.
  An `Idempotency-Key` header (Stripe; IETF draft) or `request_id` (AIP-155)
  needs a store of recent responses.
- **Mandatory `reason` field.** Recorded in the audit trail, ideally with a
  ticket reference.
- **Response carries before/after values**, so the caller can verify the change
  and revert it.
- **Limits.** Body size (launcher default 2 MiB), array lengths, and
  PostgreSQL's bind-parameter limit for batched writes (see `gotchas.md`).
- **Server-side request forgery.** If a method accepts URLs the service will
  later call (e.g. RPC endpoints), allowlist schemes, hosts, and ports, and
  disable redirects (OWASP API7).

### 6. Applying the Change

| Option | Trade-offs |
|---|---|
| Direct DB mutation inside the handler | Fine for data no running component holds in memory. For data with an in-process owner, it races that owner (see Failure Modes). |
| Route the command to the owning in-process component, which applies it at a safe point | Single-writer principle. Precedent: stats queues `BatchUpdateCharts` to its update service. Needs a handle to the component; interchain-indexer has no indexer supervisor or registry today. |
| Pause or stop the owner, confirm it stopped, edit, resume | Kafka resets consumer-group offsets only while the group is inactive; Kafka Connect rejects offset edits unless the connector is `STOPPED`. The stop must be acknowledged, for example through a per-owner advisory lock. Graph Node's `graphman rewind` sleeps and hopes. |
| Pending-command table polled by the owner (Debezium signalling table) | Survives restarts, works across replicas, doubles as an audit trail. Adds latency and a poll loop. |
| Override layer instead of mutating derived data | Store manual values in a separate table or column that takes precedence over derived values (`COALESCE(override, derived)`). Enrichment never clobbers the override, and deleting the override reverts to the derived value. Blockscout's backend uses a flag variant: `tokens.is_verified_via_admin_panel` makes its token import keep the curated icon. That variant loses the original derived value. |
| Fix upstream instead | If the value is derived from an upstream service that already has a review workflow, correcting it there fixes every consumer. A local override is the escape hatch for values specific to this service. |

### 7. Audit and Observability

| Layer | What it gives | Gaps |
|---|---|---|
| Application audit table, written in the same transaction as the mutation | Consistency: the change and its audit row commit or roll back together (transactional-outbox reasoning). Recommended fields (NIST SP 800-53 AU-3, OWASP Logging Cheat Sheet): `occurred_at`, actor key name and auth method, source IP, `request_id`, action, target identifiers, before/after (redacted), `reason`, outcome, `dry_run`. Enough to revert from. | Misses failed authentication, which never reaches the transaction. Misses manual SQL. Lives on the same system it audits (NIST AU-9(2) prefers separate storage); mitigate by revoking `UPDATE`/`DELETE` on the table from the app's role. |
| Structured `tracing` event per attempt | Covers authentication and authorization failures and validation rejects. Ships to the log pipeline, which acts as the off-system copy. | Not transactional; retention is set by the log pipeline. |
| Metrics | Count attempts by method and outcome; alert on any authentication failure. With few legitimate callers, any failure is signal. | Never assert exact deltas on process-wide metrics in tests (`rules/testing.md`). |
| DB-level audit (triggers like `audit.logged_actions`, or `pgaudit`) | Catches edits that bypass the API, including break-glass psql. | Identifies the DB role, not the person. Triggers cost write throughput, so attach them only to low-write tables. |
| Kubernetes API audit | Records `exec` / `port-forward` by person. | Not what was done inside the session. |

Never log the key or tokens, connection strings, or URLs with embedded
credentials. Log the key's name, and only the host of a URL. `Secret<T>` and
`redact_urls` already exist for this.

### 8. Documentation and Tests

- Attach `security_requirement: ApiKeyAuth` to each write RPC so swagger shows
  that it is protected.
- Write negative tests: missing key, wrong key, valid key without permission
  (403), unconfigured keys (fail-closed). Only stats has any negative auth tests
  today.

## Invariants

Constraints the sources agree on, whichever options are chosen:

1. An outside party never writes to the service's tables directly. Writes go
   through code that validates them and knows about in-memory state.
2. The credential never appears in a URL, a log line, an error message, or a
   `Debug` output.
3. When no credential is configured, write methods reject every request
   (fail-closed).
4. Every write method is denied by default and explicitly granted, even if the
   only grant today is "operator".
5. Whatever mutation commits, its audit row commits with it. Rejected attempts
   are still logged.
6. A write method is exposed on the same terms on HTTP and gRPC, or deliberately
   left unregistered on one of them.

## Failure Modes / Observability

- **Silent revert of outside edits to checkpoint cursors (fact + inference).**
  `upsert_cursors` and `mark_catchup_complete` use monotonic conflict rules:
  `realtime_cursor = GREATEST(...)`, `catchup_min_cursor = GREATEST(...)`,
  `catchup_max_cursor = LEAST(...)`. An outside edit against that direction
  (lowering `realtime_cursor`, raising `catchup_max_cursor`) is undone by the
  next flush. An edit along it persists in the DB but is not seen by the running
  stream's in-memory cursor until restart.
- **Startup seeding overwrites rows (fact).**
  - `upsert_chains` overwrites name, icon, explorer, and `custom_routes`.
  - `upsert_bridges` disables every bridge, then re-enables those in config.
  - Any API-written value in a config-seeded table needs a precedence rule or a
    provenance column, or it is lost on the next restart.
- **Replica divergence (fact).**
  - The service assumes one replica per bridge (`gotchas.md`, "The Failure
    Ledger's Healthy Path Is DB-Free Only Because One Process Owns A Bridge").
  - There is no leader election or advisory lock.
  - A command routed to in-process state reaches only the pod that received
    the request.
- **gRPC writes would be unlogged (fact).** The gRPC router has no tracing layer,
  so per-request logs exist only for HTTP.
- **What the HTTP span records (fact).** `method`, `endpoint` (the matched route
  pattern, not the raw path or query string), `client_ip`, a server-generated
  `request_id`, `status`, and `exception.*` on error. A key passed in the query
  string would therefore not leak through this span. It would still leak through
  ingress and proxy access logs, which is why the URL ban stands.
- **`client_ip` can be spoofed (inference).** It comes from actix
  `realip_remote_addr()`, which trusts `Forwarded`/`X-Forwarded-For`. Treat it as
  advisory in audit records unless the ingress overwrites those headers.

## Edge Cases / Gotchas

- **Key names exist only in env var names (fact).** stats and eth-bytecode-db
  discard them before comparing, so neither can attribute an action to a key.
  The name must be carried into the request context deliberately.
- **Key transport differs per service, and docs don't always match code.**
  - multichain-aggregator takes the key in the JSON body.
  - Parts of the Blockscout backend accept it as a generic parameter, so it is
    also accepted in the query string, although their docstrings say
    `x-api-key`.
  - **Copy the header convention, not those implementations.**
- **Status codes are inconsistent.** stats and eth-bytecode-db return 401.
  multichain-aggregator returns 403 for a wrong key. The Blockscout backend
  returns 403 when no key is configured.
- **Payload chain IDs can escape a data scope (multichain-aggregator, inference).**
  The key is checked against the request's `chain_id`, but interop `Relay`/`Init`
  messages carry the other side's chain ID. That lets one chain's key update
  relay status and token type on another chain. Data-scope checks must cover
  every object a payload touches, not only the envelope field.
- **Non-ASCII header values.** stats treats them as unauthorized.
  eth-bytecode-db returns 400. A handler that `expect`s ASCII would panic on
  them.
- **The service template declares auth but does not check it.** Its generated
  handler ignores the metadata, so a protected RPC copied from the template is
  unprotected until the check is added.
- **Separate swagger also needs a separate proto service.** Before
  `admin.proto`, the `interchain_indexer.proto` `ApiKeyAuth` definition was
  file-level and unused.
  Attaching it to an RPC documents protection; it does not enforce it.

## Chosen for interchain-indexer (v1)

The v1 choices are recorded in
[ADR-016](../adr/016-operator-write-api-v1.md); operation is in
`runbooks/write-api.md`. By axis of the flow above:

| Axis | Choice |
|---|---|
| Reachability | The shared HTTP listener, with the `/api/v1/admin/` prefix, which can optionally be blocked at the ingress. gRPC is registered too and disabled by default. No separate listener: the launcher supports one |
| Authentication | A static key in the `x-api-key` header, never in a URL. One 401 for every failure; the reason is only in a trace event |
| Key storage | Named SHA-256 digests in env (`INTERCHAIN_INDEXER__WRITE_API__KEYS_SHA256__<NAME>`); the name is the actor. Fail-closed when empty. Rotation: new name, deploy, remove the old one |
| Authorization | One implicit operator role: any valid key may call any write method. No 403 |
| Contract | A separate proto service in the public swagger, `POST` custom methods, mandatory `reason`, before/after values in the response |
| Audit | A table written in the same transaction, for **applied** changes only, plus a structured trace event for every attempt that reaches a handler. Requests rejected before the handler are neither stored nor traced |
| Applying changes | Through code that knows the in-memory state, not by editing tables: the asset icon with a row lock and `lock_timeout`, the token icon through the token cache protocol, and rescans through the failure ledger of ADR-005 |

The methods are `SetStatsAssetIcon`, `SetTokenIcon` and `RescanBlockRanges`;
which of them exist yet is visible in `admin.proto`.

## Change Triggers

Update this note when:

- interchain-indexer gains its first write endpoint, auth check, or audit table
  (record what was chosen and link the ADR)
- the launcher gains support for a second (admin) HTTP listener or for gRPC
  tracing or interceptors
- another monorepo service adds a write API or changes its key handling
- multi-replica operation of interchain-indexer becomes supported (this changes
  the "apply via owner" options)
- the stats, eth-bytecode-db, or multichain-aggregator auth code changes

## Open Questions

1. Which defaults to adopt per axis (reachability, key storage and form,
   granularity, audit layers): **resolved** by ADR-016 (see "Chosen for
   interchain-indexer (v1)").
2. What does the deployment infrastructure offer for reachability (internal
   ingress, VPN, port-forward rights for operators)? This decides whether a
   separate listener is worth its cost.
3. Should a separate admin listener be built once in
   `libs/blockscout-service-launcher` for every service, rather than per service?
4. Do the expected automation callers belong to the control-plane class (roles)
   or the ingestion class (data scopes)? The key schema should leave room for
   whichever comes first.
5. For state owned by running indexers, interchain-indexer has no supervisor or
   command channel. Does that need to exist before operational write methods are
   possible, or do restart-time commands suffice?
