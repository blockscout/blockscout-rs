# ADR-016: Operator Write API v1

**Date:** 2026-10-02

**Authors:** @EvgenKor

**Status:** Accepted

## Context

The service was read-only and had no authentication at all. Operational fixes
(a wrong bridged-token icon, a block range that must be scanned again) were made
with manual SQL. That needs standing broad database access, skips domain
validation, leaves no attributable trail, and races the running in-memory state
that owns several tables (for example the token cache and the failure ledger).

Operators need a few targeted corrections, made through code that knows about
that state, by a caller who can be identified afterwards. The first write
endpoint also sets the precedents for authentication, audit and swagger exposure
that later methods inherit. The options and the monorepo precedents are in
`.memory-bank/research/service-write-api.md`.

Three methods are in the v1 API: `SetStatsAssetIcon`, `SetTokenIcon` and
`RescanBlockRanges`. The conventions below are decided once for all of them.
`SetStatsAssetIcon` ships with the foundation; the other two are added by
follow-up changes, so their decisions are recorded here to keep the contract and
audit conventions fixed.

## Decision

### 1. Authentication

- The key travels in the `x-api-key` header, never in a URL.
- Keys are configured as **named hex SHA-256 digests** in env:
  `INTERCHAIN_INDEXER__WRITE_API__KEYS_SHA256__<NAME>=<digest>`. The key name
  (lowercased by the env loader) is the **actor** recorded in the audit log.
- The check runs in the handler: `WriteApiAuth::authenticate` returns an
  `Actor`. There is no middleware, as in every other service in the monorepo.
- **Fail-closed:** an empty catalogue is valid configuration but rejects every
  write request, with a startup warning.
- **One implicit role, "operator".** Any valid key may call any write method.
  There is no 403.
- **One 401 for every failure**, with the fixed message
  `missing or invalid x-api-key`. The reason (`missing_key`, `unknown_key`,
  `no_keys_configured`) is only in the `write api authentication failed` trace
  event.
- The catalogue is validated at startup, and any error fails the start: empty
  name, a value that is not 64 hex characters, one digest under two names. The
  messages carry key names only, never the value (it may be the key itself).
- Digests are compared with `==`, over every entry. Hashing first removes the
  timing signal, so no constant-time comparison is needed.

### 2. Audit

- Table `write_api_audit_log`: one row per **applied** change, with `actor`,
  `method`, `reason`, the normalized `request`, and a `result` that holds the
  before/after values. Dry runs and rejections are not stored; they only produce
  trace events.
- The row is written **in the same transaction** as the change. `AuditedTx`
  inserts the audit row and then commits; a failed audit insert rolls the change
  back, and so does dropping it without a commit.
- No foreign keys to the target tables: targets live in JSONB, and an FK to
  `stats_assets` would turn every later merge of an audited asset into a failing
  maintenance transaction.
- The guarantee is stated narrowly: **an audited commit requires an `Actor`.**
  It holds for writes made through `AuditedTx`. The logic functions accept any
  `&DatabaseTransaction`, and dry-run paths never reach `AuditedTx`, so every
  method has negative authentication tests, dry-run included.
- Every attempt that reaches a handler produces a structured trace event with a
  static message (`write api change applied`, `write api request rejected`,
  `write api request failed`).

### 3. Accepted deviation: pre-handler rejects are not traced

Requests that actix-prost rejects before the handler produce no trace event: the
body is not JSON, an int64 is a JSON number instead of a string, a required
field is missing, or `Content-Type: application/json` is absent. The client gets
400, the key is not checked, and nothing leaks. "A trace event for every
attempt" therefore holds for attempts that reach a handler. The launcher's
`JsonConfig` is deliberately not overridden: it is app-wide, and replacing it
would trade shared behaviour for a log line (KISS). Confirmed by the human on
2026-10-02.

### 4. Contract

- A separate proto service, `InterchainAdminService`, in the same package and
  the same public swagger file. Every operation is marked `ApiKeyAuth` and
  tagged `Admin`.
- Custom `POST` methods under `/api/v1/admin/…`, with a JSON body. int64 fields
  are JSON strings.
- Identical on HTTP and gRPC. The gRPC listener is disabled by default.
- Every method takes a mandatory `reason` and returns the before/after values,
  so the caller can verify the change and revert it.

### 5. Icons

- **Token icon (`SetTokenIcon`)** is a patch until the source supplies a value:
  TokenInfo (Blockscout) wins, through
  `COALESCE(EXCLUDED.token_icon, tokens.token_icon)`. The token cache is kept
  consistent by one per-key mutex shared by every writer.
- **Asset icon (`SetStatsAssetIcon`)** is permanent except in two cases: the
  asset loses a merge, and the accepted one-round-trip race with the
  fill-if-empty writers (see the gotcha on `stats_assets.icon_url`). The row is
  locked `FOR NO KEY UPDATE` with a `lock_timeout`, not `FOR UPDATE`, so it does
  not block stats maintenance.
- The two methods never touch each other's data: the asset method does not read
  or write `tokens`, and the token method does not write `stats_assets`.

### 6. Rescan (`RescanBlockRanges`)

- It writes the ranges into the ledger of ADR-005 as an external writer.
- Precondition: the `note_open` and `resolved_since_snapshot` rules in the retry
  tick are deployed everywhere.
- A request is rejected when a range overlaps or is adjacent to any open row of
  its pair.
- The noise in `failed_blocks`, `catchup_complete` and hole-age signals while
  the ranges drain is accepted.
- The contract is "scan these blocks again", not "repair the data".
- There is no check of the indexer's state: a request for a bridge whose indexer
  is not running waits as `failed_blocks` until the next successful start.

## Alternatives Considered

### Alternative 1: Override column for the token icon

A separate manual-icon column that wins over the derived one.

**Pros:**
- A manual value survives any later enrichment.

**Cons:**
- Contradicts the rule that the source wins, and needs a precedence rule in
  every reader.

### Alternative 2: Separate table of rescan requests

A pending-command table polled by the scheduler.

**Pros:**
- Survives restarts, and doubles as an audit trail.

**Cons:**
- A much wider scheduler change than writing to the existing ledger.

### Alternative 3: Separate admin listener

**Pros:**
- A network boundary between the public API and the write API.

**Cons:**
- The launcher supports exactly one HTTP listener. The prefix `/api/v1/admin/`
  can still be blocked at the ingress if desired. The key check stays mandatory
  either way.

### Alternative 4: Keys in the database

**Pros:**
- Runtime key management without a restart.

**Cons:**
- Needs key-management methods, or it ends in manual SQL, which is what the API
  replaces.

### Alternative 5: Constant-time comparison (`subtle`)

**Pros:**
- Defends against a timing side channel on the comparison.

**Cons:**
- Not needed: digests, not keys, are compared, so a timing signal is useless.
  It would add a dependency the repository does not otherwise have.

## Consequences

### Positive

- No manual SQL for these operations, and an attributable audit trail.
- A leaked environment dump contains digests, not replayable keys.

### Negative

- Rotating a key needs a restart (add the new name, deploy, remove the old one).
- The admin methods are visible in the public swagger.
- A manual asset icon can be lost when its asset loses a merge.
- Requests rejected before the handler are not logged.

### Neutral

- One process per bridge is assumed, as elsewhere in the service. The token
  cache invalidation and the ledger cache are process-local.

## References

- `.memory-bank/research/service-write-api.md` — options and precedents
- `.memory-bank/runbooks/write-api.md` — issuing keys, calling the API, rollback
- ADR-005 — failed-range ledger (rescan writes into it)
- `interchain-indexer-server/src/auth.rs`, `interchain-indexer-server/src/services/admin/`,
  `interchain-indexer-logic/src/write_api.rs`,
  `interchain-indexer-proto/proto/v1/admin.proto`
