# Write API

Read this when you operate the operator write API: issuing and rotating keys,
calling a method, reading its errors, rolling a change back, and finding the
audit trail. The design and its trade-offs are in
[ADR-016](../adr/016-operator-write-api-v1.md).

Every method requires the `x-api-key` header and a JSON body sent with
`Content-Type: application/json`. int64 fields are JSON strings (`"42"`, not
`42`). A request without the content type, with a number for an int64, or
without a required field is rejected with 400 before the key is checked, and is
not logged.

Paths below assume no `server.http.base_path`. If the deployment sets one, the
admin paths carry the prefix.

## Keys

- **Generate** a key and its digest. Use `printf %s`, not `echo`: `echo` appends
  a newline, and the digest would not match the key you send.

  ```sh
  KEY=$(openssl rand -hex 32)
  printf %s "$KEY" | sha256sum | cut -d' ' -f1   # the digest to configure
  ```

  The key can be any string; the digest is what the service stores.
- **Configure** the digest, never the key, as one variable per key:

  ```sh
  INTERCHAIN_INDEXER__WRITE_API__KEYS_SHA256__OPS_ALICE=<64-hex digest>
  ```

  The name after `KEYS_SHA256__` (lowercased by the env loader: `ops_alice`) is
  the **actor** written to the audit log. Do not put `__` inside a name: it
  becomes a nested key and the settings fail to deserialize.
- **Never set the map variable itself**
  (`INTERCHAIN_INDEXER__WRITE_API__KEYS_SHA256=<value>`). The settings error that
  follows prints the value, and the value may be a key.
- **Startup validation.** The service refuses to start on an empty name, a value
  that is not 64 hex characters (upper or lower case), or one digest under two
  names. The message names the key and never repeats the value.
- **Rotate** by adding a new name, deploying, moving callers to the new key, then
  removing the old name and deploying again. Keys are read at startup, so a
  change needs a restart.
- **Fail-closed.** With no keys configured the service starts, logs
  `write api has no keys configured; every write method will reject requests`,
  and answers every write request with 401.

## Calling the API

Set `BASE` to the service URL and `KEY` to the key (not the digest).

```sh
curl -sS -X POST "$BASE/api/v1/admin/stats/assets:setIcon" \
  -H 'content-type: application/json' -H "x-api-key: $KEY" \
  -d '{"stats_asset_id":"42","icon_url":"https://example.com/usdc.png","reason":"TICKET-123"}'

curl -sS -X POST "$BASE/api/v1/admin/stats/assets:setIcon" \
  -H 'content-type: application/json' -H "x-api-key: $KEY" \
  -d '{"stats_asset_id":"42","clear":true,"reason":"TICKET-123 revert"}'
```

The same methods are available on gRPC (`InterchainAdminService`) when the gRPC
listener is enabled; it is disabled by default.

Send exactly one of `icon_url` and `clear`. `reason` is required (1 to 1000
characters after trimming), for example a ticket link. `icon_url` must be an
`https` URL without credentials, at most 2048 bytes; it is stored in its
normalized form and is never fetched by the service.

## Responses And Errors

The error body is JSON: `{"code": <gRPC code>, "message": "..."}`.

| HTTP | Meaning |
|---|---|
| 200 | Applied. The icon methods return `audit_id` and the before/after values. `RescanBlockRanges` returns `audit_id`, the scheduled ranges and the estimates; its dry run also returns 200, with `audit_id: null` (nothing is written and no audit row exists). |
| 401 | Missing, empty, unknown key, or no keys configured. One message for all of them. |
| 400 | Validation failed (code 3), a state precondition refused a well-formed request (code 9, `RescanBlockRanges`), or the body is not decodable. |
| 404 | No such asset, or no `tokens` row for the token. |
| 409 | Stats maintenance holds the row. Retry. |
| 500 | Internal error. The client sees no details; the server log has them. |

## Asset Icon (`SetStatsAssetIcon`)

Sets or clears `icon_url` of one `stats_assets` row: the `icon_url` of a row of
`GET /api/v1/stats/chain/{chain_id}/bridged-tokens`. Take `stats_asset_id` from
that response. The method does not touch `tokens`: token icons are separate.

- **Roll back** by repeating the call with `icon_url_before` from the response
  (or from the audit row, `result->'icon_url_before'`), or with `clear` when it
  was `null`.
- **Permanence.** The writers that derive an asset icon (stats projection, asset
  merge, token-info propagation) only fill an empty icon, so a manual value
  stays. Two exceptions:
  - **A lost merge.** When two assets merge, the one with more tokens wins. If
    the winner already has an icon, the loser's icon, including a manual one, is
    gone.
  - **A one-round-trip race.** Those writers read the whole row and write it
    back without locking. An operator write inside that window is reverted.
    Repeat the call if the response is not reflected in the list.
- **Re-apply after a merge.** This read-only query finds audited asset changes
  whose asset no longer exists, with the member tokens it had:

  ```sql
  SELECT a.id, a.occurred_at, a.request->>'stats_asset_id' AS old_asset_id,
         a.result->'icon_url_after' AS icon, a.result->'member_tokens' AS members
  FROM write_api_audit_log a
  WHERE a.method = 'SetStatsAssetIcon'
    AND NOT EXISTS (SELECT 1 FROM stats_assets s
                    WHERE s.id = (a.request->>'stats_asset_id')::bigint)
  ORDER BY a.occurred_at;
  ```

  Find the current asset of a member token. Pass `$2` exactly as the audit row
  shows `token_address`, with its `0x` prefix. The audit shows a native member as
  `null`; it is stored as 20 zero bytes, so pass
  `0x0000000000000000000000000000000000000000`:

  ```sql
  SELECT stats_asset_id FROM stats_asset_tokens
  WHERE chain_id = $1 AND token_address = decode(substr($2, 3), 'hex');
  ```

  Then apply the icon to that asset through the API.
- **A migration that clears `stats_assets`** loses manual icons the same way and
  needs the same re-application.

## Token Icon (`SetTokenIcon`)

Sets or clears `tokens.token_icon` of one chain-local token that already has a
`tokens` row. This is the icon `/api/v1/interchain/transfers` and `/messages`
show for the token (`icon_url`), and the `tokens[].icon_url` nested in the
bridged-tokens list.

```sh
# the native token of a chain (shown with a null address in the API)
curl -sS -X POST "$BASE/api/v1/admin/tokens:setIcon" \
  -H 'content-type: application/json' -H "x-api-key: $KEY" \
  -d '{"chain_id":"100","native":true,"icon_url":"https://example.com/xdai.png","reason":"TICKET-1"}'

# an ERC-20 token by its address
curl -sS -X POST "$BASE/api/v1/admin/tokens:setIcon" \
  -H 'content-type: application/json' -H "x-api-key: $KEY" \
  -d '{"chain_id":"1","address":"0x6b175474e89094c44da98b954eedeac495271d0f","icon_url":"https://example.com/dai.png","reason":"TICKET-2"}'
```

Send exactly one of `address` and `native` (the zero address is rejected: use
`native`), and exactly one of `icon_url` and `clear`. `chain_id` is a JSON
string.

- **Semantics.** The token-info source (Blockscout) is the source of truth.
  - An icon Blockscout later provides **replaces** the value set here.
  - A Blockscout miss or error does **not** erase it.
  - The xDai native seed that runs at every start does not erase it.
  - The method does not change `stats_assets`. The existing fill-if-empty
    derivation may later copy the token icon into an **empty** asset icon.
- **404** when the token has no `tokens` row. A row appears only after the
  token's metadata has been fetched successfully (after the token showed up in a
  transfer), or, for the xDai native token, when the indexer seeds it at start. A
  token whose metadata fetch keeps failing has no row. The method never inserts
  one.
- **Latency.** A call can take up to about 15 s when it waits behind a
  request-time Blockscout lookup for the same token. If the client times out or
  the connection drops, check `write_api_audit_log` for the call and re-issue it:
  the call is idempotent (it records another audit row and drops the process's
  cached entry again).
- **Roll back** by repeating the call with `icon_url_before` from the response
  (or from the audit row, `result->'icon_url_before'`), or with `clear` when it
  was `null`. After `clear`, Blockscout can fill the icon again.
- **Visibility.** In the process that handled the call, the next request after
  the call returns reads the new value, in `/transfers`, `/messages` and the
  bridged-tokens list, without a restart.
- **The cache reset is local to the process.** If several processes serve the API
  from one database, the others keep showing the old icon until they restart:
  restart them.
- **After a code rollback** the old rule comes back: the next xDai start erases
  the manual icon of the native token, and Blockscout misses erase ERC-20 icons.
  Re-apply the icons from the audit log; the last row per token wins:

  ```sql
  SELECT request, result, occurred_at FROM write_api_audit_log
  WHERE method = 'SetTokenIcon' ORDER BY occurred_at;
  ```

## Block Range Rescan (`RescanBlockRanges`)

Queues block ranges of `(bridge_id, chain_id)` pairs for the running indexer to
scan again (`POST /api/v1/admin/indexing:rescanBlockRanges`, dry run supported).
It rescans; it does not repair stored data. What it can fix, what must be true
before the call and how to watch it drain are in
[block-range-rescan.md](block-range-rescan.md).

## Audit Queries

Every applied change is one row of `write_api_audit_log`, written in the same
transaction as the change. Rejected requests are not stored; they are in the
service log (`write api authentication failed`, `write api request rejected`,
`write api request failed`).

Recent changes by actor and method:

```sql
SELECT id, occurred_at, actor, method, reason, request, result
FROM write_api_audit_log
WHERE actor = 'ops_alice' AND method = 'SetStatsAssetIcon'
ORDER BY occurred_at DESC
LIMIT 50;
```

The `audit_id` of a response is the row `id`, and the `write api change applied`
log event carries the same `audit_id` next to the `request_id` of the HTTP log.
