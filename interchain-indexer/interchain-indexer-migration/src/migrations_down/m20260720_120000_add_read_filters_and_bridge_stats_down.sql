-- ============================================================================
-- Inverse, rebuild-oriented down migration
--
-- Restores the bridge-collapsed stats schema. Truncating the aggregates before
-- dropping bridge_id is mandatory: otherwise rows from different bridges would
-- collide under the restored bridge-collapsed primary keys. As on the up path,
-- clearing the aggregates and resetting the canonical markers are inseparable
-- and run together, so the old binary rebuilds the collapsed projections from
-- canonical rows via startup backfill.
-- ============================================================================

TRUNCATE TABLE stats_messages;
TRUNCATE TABLE stats_messages_days;
TRUNCATE TABLE stats_asset_edges;

UPDATE crosschain_messages SET stats_processed = 0 WHERE stats_processed <> 0;
UPDATE crosschain_transfers SET stats_processed = 0 WHERE stats_processed <> 0;

-- ---------------------------------------------------------------------------
-- stats_asset_edges: restore bridge-collapsed schema
-- ---------------------------------------------------------------------------
DROP INDEX IF EXISTS stats_asset_edges_bridge_idx;
DROP INDEX IF EXISTS stats_asset_edges_src_chain_idx;
DROP INDEX IF EXISTS stats_asset_edges_dst_chain_idx;
ALTER TABLE stats_asset_edges DROP CONSTRAINT stats_asset_edges_pkey;
ALTER TABLE stats_asset_edges DROP COLUMN bridge_id;
ALTER TABLE stats_asset_edges
  ADD CONSTRAINT stats_asset_edges_pkey
  PRIMARY KEY (stats_asset_id, src_chain_id, dst_chain_id);
CREATE INDEX stats_asset_edges_src_chain_idx
  ON stats_asset_edges (src_chain_id);
CREATE INDEX stats_asset_edges_dst_chain_idx
  ON stats_asset_edges (dst_chain_id);

-- ---------------------------------------------------------------------------
-- stats_messages_days: restore bridge-collapsed schema
-- ---------------------------------------------------------------------------
DROP INDEX IF EXISTS stats_messages_days_bridge_idx;
DROP INDEX IF EXISTS stats_messages_days_src_chain_bridge_date_idx;
DROP INDEX IF EXISTS stats_messages_days_dst_chain_bridge_date_idx;
ALTER TABLE stats_messages_days DROP CONSTRAINT stats_messages_days_pkey;
ALTER TABLE stats_messages_days DROP COLUMN bridge_id;
ALTER TABLE stats_messages_days
  ADD CONSTRAINT stats_messages_days_pkey
  PRIMARY KEY (date, src_chain_id, dst_chain_id);

-- ---------------------------------------------------------------------------
-- stats_messages: restore bridge-collapsed schema
-- ---------------------------------------------------------------------------
DROP INDEX IF EXISTS stats_messages_src_chain_idx;
DROP INDEX IF EXISTS stats_messages_dst_chain_idx;
ALTER TABLE stats_messages DROP CONSTRAINT stats_messages_pkey;
ALTER TABLE stats_messages DROP COLUMN bridge_id;
ALTER TABLE stats_messages
  ADD CONSTRAINT stats_messages_pkey
  PRIMARY KEY (src_chain_id, dst_chain_id);
CREATE INDEX stats_messages_src_chain_idx
  ON stats_messages (src_chain_id);
CREATE INDEX stats_messages_dst_chain_idx
  ON stats_messages (dst_chain_id);

-- ============================================================================
-- Drop the canonical read-filter indexes owned by this migration
-- ============================================================================
DROP INDEX IF EXISTS crosschain_messages_bridge_ts_idx;
DROP INDEX IF EXISTS crosschain_transfers_bridge_idx;
