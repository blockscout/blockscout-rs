# ADR-012: Token Type Belongs To The Chain-Local Token

**Date:** 2026-09-16

**Status:** Accepted

## Context

The first xDai design represented native xDAI using the zero-address storage
key on Gnosis. This let the native endpoint participate in statistics without
making a missing token address mean both "native" and "identity unresolved".
The design extended `crosschain_transfers.type` with mixed ERC-20/native
variants and derived each API token's type from that transfer field.

That choice limited the original implementation scope. Splitting transfer
type into source/destination fields was deferred because it required changes
to existing transfer rows and serving contracts. It was not a constraint that
prevented putting type on the token registry. Transfer-derived types cannot
describe a token returned independently by the statistics API, and the mixed
variants multiply as new token kinds are added. All existing production
transfers are ERC-20; NFT ingestion is not supported.

The xDai migration has not been deployed outside a disposable local development
database, so its schema can be revised before deployment.

## Decision

1. Store token kind in `tokens.type`, using a non-null PostgreSQL `token_type`
   enum (`erc20`, `native`, `erc721`, `erc1155`) with default `erc20`. The
   identity remains `(chain_id, address)`: kind describes that token rather
   than a particular movement of it.
2. Copy kind to non-null `stats_asset_tokens.type` for the statistics read
   model. Projection reads the registry when linking a token; metadata
   propagation updates an existing statistics token's kind from the registry.
3. Remove `crosschain_transfers.type` and its enum. Do not add
   `src_token_type` or `dst_token_type`: the endpoint keys already identify
   both tokens. Future endpoint-type filters can use registry lookups; any
   denormalization or additional indexes need evidence from those queries.
4. Keep `crosschain_transfers.asset_linkage` from [ADR-011](./011-cross-asset-edges-and-per-transfer-linkage.md).
   It describes whether a particular transfer relates representations of one
   asset (`mirror`) or different assets (`conversion`). Token kinds cannot
   determine that relationship: the same pair of kinds can have either
   linkage.
5. Reserve exactly twenty zero bytes as the native coin's internal address
   on each chain. An explicit registry type takes precedence. Before optional
   metadata exists, the shared `TokenType::from_address` fallback identifies
   that exact sentinel as native and other keys as ERC-20, the only contract
   kind currently indexed. Future NFT ingestion must supply an explicit kind;
   the fallback is not contract-standard detection.
6. Expose enum names `ERC20`, `NATIVE`, `ERC721`, `ERC1155` without a
   `TOKEN_TYPE_` prefix. Keep their protobuf numeric values unchanged.
   Transfer and statistics token objects return a null address for native
   tokens; the storage sentinel never becomes a public contract address.
7. Native metadata is seeded by the indexer. A seed failure may leave name,
   symbol or decimals unavailable, but must neither expose the sentinel nor
   trigger ERC-20 contract calls against it. Both request-time lookup and
   statistics enrichment recognize the native storage key without the seed.

## Migration And Consequences

All schema changes are incorporated into
`m20260915_120000_add_xdai_and_cross_asset_stats`. Existing deployed token
contracts and transfers are guaranteed to be ERC-20. The default initializes
both token tables without scanning transfers or reconstructing absent token
metadata. ERC721/ERC1155 enum variants do not imply historical NFT support and
require no backfill. The migration also recognizes the reserved native key in
the small token tables, preserving native identity after a down/up cycle.

The new type columns grow the small registry and statistics mapping tables,
not the multimillion-row transfer table. Removing a PostgreSQL column is not
a promise that existing physical row storage will immediately shrink. The
same migration's separate asset-linkage changes still perform their existing
transfer backfill.

The rollback remains intended for database recreation, as in ADR-011. It
restores legacy transfer kinds only when both registry endpoints exist and
have the same kind; mixed or missing endpoints become null in the restored
field. It cannot losslessly represent the new model.

The statistics copy is deliberately denormalized and must stay aligned with
the registry through projection and metadata propagation. JSON enum strings
change, so clients comparing the former prefixed strings need updating even
though protobuf numbers remain compatible.

## References

- [Migration up](../../interchain-indexer-migration/src/migrations_up/m20260915_120000_add_xdai_and_cross_asset_stats_up.sql)
  and [down](../../interchain-indexer-migration/src/migrations_down/m20260915_120000_add_xdai_and_cross_asset_stats_down.sql).
- [Shared token-kind fallback](../../interchain-indexer-entity/src/manual/mod.rs).
- [Token metadata service](../../interchain-indexer-logic/src/token_info/service.rs),
  [statistics projection](../../interchain-indexer-logic/src/stats/projection.rs),
  and [metadata propagation](../../interchain-indexer-logic/src/database.rs).
- [API schema](../../interchain-indexer-proto/proto/v1/interchain_indexer.proto).
