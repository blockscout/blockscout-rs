DROP INDEX IF EXISTS stats_messages_dst_chain_idx;
DROP INDEX IF EXISTS stats_messages_src_chain_idx;
DROP TABLE IF EXISTS stats_messages;

DROP TABLE IF EXISTS stats_chains;

DROP INDEX IF EXISTS crosschain_transfers_dst_user_by_chain_idx;
DROP INDEX IF EXISTS crosschain_transfers_src_user_by_chain_idx;
DROP INDEX IF EXISTS crosschain_messages_dst_user_by_chain_idx;
DROP INDEX IF EXISTS crosschain_messages_src_user_by_chain_idx;

DROP INDEX IF EXISTS stats_asset_edges_dst_chain_idx;
DROP INDEX IF EXISTS stats_asset_edges_src_chain_idx;

ALTER TABLE crosschain_messages DROP COLUMN IF EXISTS stats_processed;

DROP INDEX IF EXISTS crosschain_transfers_stats_asset_idx;
ALTER TABLE crosschain_transfers
  DROP COLUMN IF EXISTS stats_asset_id,
  DROP COLUMN IF EXISTS stats_processed;

DROP TABLE IF EXISTS stats_asset_edges;
DROP TYPE IF EXISTS edge_amount_side;

DROP TABLE IF EXISTS stats_asset_tokens;
DROP TABLE IF EXISTS stats_assets;
