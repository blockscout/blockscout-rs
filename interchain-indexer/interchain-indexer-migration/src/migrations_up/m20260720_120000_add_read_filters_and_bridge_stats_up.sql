-- ============================================================================
-- Canonical read-filter indexes
-- ============================================================================

-- bridge-filtered keyset pagination on messages
CREATE INDEX crosschain_messages_bridge_ts_idx
    ON crosschain_messages (bridge_id, init_timestamp, id);

-- bridge filter on transfers (chain filters are served by the leading
-- columns of crosschain_transfers_token_src_idx / _token_dst_idx)
CREATE INDEX crosschain_transfers_bridge_idx
    ON crosschain_transfers (bridge_id);

-- ============================================================================
-- Bridge-qualified stats projection rebuild
--
-- The additive stats aggregates (stats_messages, stats_messages_days,
-- stats_asset_edges) gain a bridge dimension. Existing rows cannot be
-- attributed to a bridge after the fact, so they are cleared and the canonical
-- projection markers are reset to zero: startup backfill then reconstructs the
-- aggregates, bridge-qualified, from canonical rows.
--
-- Clearing the aggregates and resetting the markers are inseparable and run in
-- this single migration transaction. Reversing or partially performing them
-- would either lose historical stats or double-count.
-- ============================================================================

TRUNCATE TABLE stats_messages;
TRUNCATE TABLE stats_messages_days;
TRUNCATE TABLE stats_asset_edges;

-- Reset only the canonical projection markers (leave updated_at and
-- crosschain_transfers.stats_asset_id untouched).
UPDATE crosschain_messages SET stats_processed = 0 WHERE stats_processed <> 0;
UPDATE crosschain_transfers SET stats_processed = 0 WHERE stats_processed <> 0;

-- ---------------------------------------------------------------------------
-- stats_messages: bridge-qualified directional message counts
-- ---------------------------------------------------------------------------
ALTER TABLE stats_messages
  ADD COLUMN bridge_id INTEGER NOT NULL
    REFERENCES bridges(id) ON DELETE CASCADE;

ALTER TABLE stats_messages DROP CONSTRAINT stats_messages_pkey;
ALTER TABLE stats_messages
  ADD CONSTRAINT stats_messages_pkey
  PRIMARY KEY (bridge_id, src_chain_id, dst_chain_id);

-- Directional focal-chain reads, unfiltered (leading chain) and bridge-filtered
-- (leading chain + bridge). The bridge-leading PK covers the bridge FK cascade.
DROP INDEX IF EXISTS stats_messages_src_chain_idx;
DROP INDEX IF EXISTS stats_messages_dst_chain_idx;
CREATE INDEX stats_messages_src_chain_idx
  ON stats_messages (src_chain_id, bridge_id, dst_chain_id);
CREATE INDEX stats_messages_dst_chain_idx
  ON stats_messages (dst_chain_id, bridge_id, src_chain_id);

-- ---------------------------------------------------------------------------
-- stats_messages_days: bridge-qualified daily directional message counts
-- ---------------------------------------------------------------------------
ALTER TABLE stats_messages_days
  ADD COLUMN bridge_id INTEGER NOT NULL
    REFERENCES bridges(id) ON DELETE CASCADE;

ALTER TABLE stats_messages_days DROP CONSTRAINT stats_messages_days_pkey;
ALTER TABLE stats_messages_days
  ADD CONSTRAINT stats_messages_days_pkey
  PRIMARY KEY (date, bridge_id, src_chain_id, dst_chain_id);

-- Retain date-efficient unfiltered daily reads (stats_messages_days_date_idx,
-- stats_messages_days_src_chain_date_idx, stats_messages_days_dst_chain_date_idx
-- created by the stats-tables migration) and add bridge-aware variants.
CREATE INDEX stats_messages_days_src_chain_bridge_date_idx
  ON stats_messages_days (src_chain_id, bridge_id, date);
CREATE INDEX stats_messages_days_dst_chain_bridge_date_idx
  ON stats_messages_days (dst_chain_id, bridge_id, date);
-- Bridge-leading index for the bridge FK cascade (the date-leading PK does not
-- cover it).
CREATE INDEX stats_messages_days_bridge_idx
  ON stats_messages_days (bridge_id);

-- ---------------------------------------------------------------------------
-- stats_asset_edges: bridge-qualified movement counters per asset
-- ---------------------------------------------------------------------------
ALTER TABLE stats_asset_edges
  ADD COLUMN bridge_id INTEGER NOT NULL
    REFERENCES bridges(id) ON DELETE CASCADE;

ALTER TABLE stats_asset_edges DROP CONSTRAINT stats_asset_edges_pkey;
ALTER TABLE stats_asset_edges
  ADD CONSTRAINT stats_asset_edges_pkey
  PRIMARY KEY (stats_asset_id, bridge_id, src_chain_id, dst_chain_id);

-- Directional focal-chain reads, unfiltered and bridge-filtered, covering the
-- opposite chain and asset id for the per-asset aggregate. The asset-leading PK
-- covers the stats_assets FK cascade.
DROP INDEX IF EXISTS stats_asset_edges_src_chain_idx;
DROP INDEX IF EXISTS stats_asset_edges_dst_chain_idx;
CREATE INDEX stats_asset_edges_src_chain_idx
  ON stats_asset_edges (src_chain_id, bridge_id, dst_chain_id, stats_asset_id);
CREATE INDEX stats_asset_edges_dst_chain_idx
  ON stats_asset_edges (dst_chain_id, bridge_id, src_chain_id, stats_asset_id);
-- Bridge-leading index for the bridge FK cascade (the asset-leading PK does not
-- cover it).
CREATE INDEX stats_asset_edges_bridge_idx
  ON stats_asset_edges (bridge_id);
