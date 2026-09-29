-- ============================================================================
-- This down migration is destructive and incomplete. Incomplete, because
-- PostgreSQL has no ALTER TYPE ... DROP VALUE. Destructive, because the
-- cross-asset reversal cannot be lossless: on stats_asset_edges,
-- (xDAI, DAI, 3, 100, 1) and (xDAI, USDS, 3, 100, 1) collapse to the same
-- restored primary key, so the collapse must SUM. Its supported use is
-- `just migrate-fresh`, not a partial roll back of a running system.
-- ============================================================================

-- ---------------------------------------------------------------------------
-- stats_asset_edges: rebuild in its old, single-asset shape.
--
-- Rebuild via CREATE TABLE + INSERT ... SELECT ... GROUP BY rather than
-- ALTERing in place: two source assets reaching one destination asset cannot
-- be expressed in the old shape, so the collapse must sum rather than pick.
-- cumulative_amount is approximate whenever rows with differing decimals or
-- amount_side collapse into the same restored key -- the same approximation
-- the merge fold already accepts (ADR-004 "Scope boundaries").
-- ---------------------------------------------------------------------------
CREATE TABLE stats_asset_edges_old (
  stats_asset_id      BIGINT NOT NULL
    REFERENCES stats_assets(id)
    ON DELETE CASCADE,

  src_chain_id        BIGINT NOT NULL
    REFERENCES chains(id)
    ON DELETE CASCADE,

  dst_chain_id        BIGINT NOT NULL
    REFERENCES chains(id)
    ON DELETE CASCADE,

  transfers_count     BIGINT NOT NULL DEFAULT 0,
  cumulative_amount   NUMERIC(78,0) NOT NULL DEFAULT 0,
  decimals            SMALLINT,
  amount_side         edge_amount_side NOT NULL,

  created_at          TIMESTAMP NOT NULL DEFAULT now(),
  updated_at          TIMESTAMP NOT NULL DEFAULT now(),

  bridge_id           INTEGER NOT NULL
    REFERENCES bridges(id)
    ON DELETE CASCADE,

  PRIMARY KEY (stats_asset_id, bridge_id, src_chain_id, dst_chain_id)
);

INSERT INTO stats_asset_edges_old
  (stats_asset_id, src_chain_id, dst_chain_id, transfers_count,
   cumulative_amount, decimals, amount_side, created_at, updated_at, bridge_id)
SELECT src_stats_asset_id, src_chain_id, dst_chain_id,
       SUM(transfers_count), SUM(cumulative_amount),
       MIN(decimals), MIN(amount_side::text)::edge_amount_side,
       MIN(created_at), MAX(updated_at), bridge_id
FROM   stats_asset_edges
GROUP BY src_stats_asset_id, bridge_id, src_chain_id, dst_chain_id;

DROP TABLE stats_asset_edges;
ALTER TABLE stats_asset_edges_old RENAME TO stats_asset_edges;

-- RENAME TO does not rename constraints created against the old table name;
-- without this, the constraint names carry a stray `_old_` and a later
-- `migrate-up` fails on `DROP CONSTRAINT stats_asset_edges_pkey` (that name
-- no longer exists). Restores the exact names `migrate-fresh` would produce.
ALTER TABLE stats_asset_edges RENAME CONSTRAINT stats_asset_edges_old_pkey TO stats_asset_edges_pkey;
ALTER TABLE stats_asset_edges RENAME CONSTRAINT stats_asset_edges_old_stats_asset_id_fkey TO stats_asset_edges_stats_asset_id_fkey;
ALTER TABLE stats_asset_edges RENAME CONSTRAINT stats_asset_edges_old_src_chain_id_fkey TO stats_asset_edges_src_chain_id_fkey;
ALTER TABLE stats_asset_edges RENAME CONSTRAINT stats_asset_edges_old_dst_chain_id_fkey TO stats_asset_edges_dst_chain_id_fkey;
ALTER TABLE stats_asset_edges RENAME CONSTRAINT stats_asset_edges_old_bridge_id_fkey TO stats_asset_edges_bridge_id_fkey;

CREATE INDEX stats_asset_edges_src_chain_idx
  ON stats_asset_edges (src_chain_id, bridge_id, dst_chain_id, stats_asset_id);
CREATE INDEX stats_asset_edges_dst_chain_idx
  ON stats_asset_edges (dst_chain_id, bridge_id, src_chain_id, stats_asset_id);
CREATE INDEX stats_asset_edges_bridge_idx
  ON stats_asset_edges (bridge_id);

-- ---------------------------------------------------------------------------
-- crosschain_transfers: restore the single stats_asset_id link.
-- src_stats_asset_id is the only defensible choice and the only one the old
-- binary can read -- it never wrote a conversion row.
-- ---------------------------------------------------------------------------
ALTER TABLE crosschain_transfers
  ADD COLUMN stats_asset_id BIGINT
    REFERENCES stats_assets(id)
    ON DELETE SET NULL;

UPDATE crosschain_transfers SET stats_asset_id = src_stats_asset_id;

CREATE INDEX crosschain_transfers_stats_asset_idx
  ON crosschain_transfers (stats_asset_id);

ALTER TABLE crosschain_transfers
  DROP COLUMN src_stats_asset_id,
  DROP COLUMN dst_stats_asset_id,
  DROP COLUMN asset_linkage;

DROP TYPE transfer_asset_linkage;

-- The legacy enum cannot represent mixed endpoints. Restore only homogeneous
-- kinds; mixed or missing token metadata remains NULL on this lossy rollback.
CREATE TYPE transfer_type AS ENUM ('erc20', 'erc721', 'native', 'erc1155');
ALTER TABLE crosschain_transfers ADD COLUMN type transfer_type;
UPDATE crosschain_transfers AS tr SET type = src.type::text::transfer_type
FROM tokens AS src, tokens AS dst
WHERE src.chain_id = tr.token_src_chain_id AND src.address = tr.token_src_address
  AND dst.chain_id = tr.token_dst_chain_id AND dst.address = tr.token_dst_address
  AND src.type = dst.type;
ALTER TABLE stats_asset_tokens DROP COLUMN type;
ALTER TABLE tokens DROP COLUMN type;
DROP TYPE token_type;

-- PostgreSQL cannot remove the 'xdai' bridge enum value. The table comment
-- likewise remains as documentation of the shared confirmations storage.
