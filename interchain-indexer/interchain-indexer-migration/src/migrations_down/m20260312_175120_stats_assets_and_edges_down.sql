DROP TABLE IF EXISTS stats_chains;

DROP INDEX IF EXISTS stats_asset_edges_dst_chain_idx;
DROP INDEX IF EXISTS stats_asset_edges_src_chain_idx;

DROP INDEX IF EXISTS crosschain_transfers_stats_asset_idx;
ALTER TABLE crosschain_transfers DROP COLUMN IF EXISTS stats_asset_id;

DROP TABLE IF EXISTS stats_asset_edges;
DROP TYPE IF EXISTS edge_decimals_side;

DROP TABLE IF EXISTS stats_asset_tokens;
DROP TABLE IF EXISTS stats_assets;
