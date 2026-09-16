ALTER TYPE bridge_type   ADD VALUE IF NOT EXISTS 'xdai';
-- Token kind belongs to a chain-local token, independently of any transfer.
-- This migration precedes all xDai ingestion; existing production tokens are
-- ERC-20. Preserve explicit NFT classifications from the legacy transfer field.
CREATE TYPE token_type AS ENUM ('erc20', 'native', 'erc721', 'erc1155');
ALTER TABLE tokens ADD COLUMN type token_type NOT NULL DEFAULT 'erc20';
ALTER TABLE stats_asset_tokens ADD COLUMN type token_type NOT NULL DEFAULT 'erc20';

UPDATE tokens SET type = 'native'
WHERE address = decode(repeat('00', 20), 'hex');
UPDATE stats_asset_tokens SET type = 'native'
WHERE token_address = decode(repeat('00', 20), 'hex');

-- Fail visibly on contradictory historical kinds rather than choosing a type
-- arbitrarily. This scan does not rewrite the large transfer table.
CREATE TEMP TABLE legacy_token_kinds ON COMMIT DROP AS
SELECT DISTINCT chain_id, address, type::text::token_type AS type
FROM (
  SELECT token_src_chain_id AS chain_id, token_src_address AS address, type
  FROM crosschain_transfers WHERE type IN ('erc721', 'erc1155')
  UNION ALL
  SELECT token_dst_chain_id, token_dst_address, type
  FROM crosschain_transfers WHERE type IN ('erc721', 'erc1155')
) endpoints
WHERE address IS NOT NULL;
CREATE UNIQUE INDEX ON legacy_token_kinds (chain_id, address);
INSERT INTO tokens (chain_id, address, type)
SELECT chain_id, address, type FROM legacy_token_kinds
ON CONFLICT (chain_id, address) DO UPDATE SET type = EXCLUDED.type;
UPDATE stats_asset_tokens AS sat SET type = t.type
FROM tokens AS t WHERE t.chain_id = sat.chain_id AND t.address = sat.token_address;

ALTER TABLE crosschain_transfers DROP COLUMN type;
DROP TYPE transfer_type;

-- The table name is AMB-flavoured; its contents are not. Record the shared
-- ownership in the database itself, where the next reader of \d+ will see it.
COMMENT ON TABLE amb_messages_confirmations IS
  'Per-validator signature confirmations. Shared by the AMB/Omnibridge and '
  'xDai indexers: every column is protocol-agnostic and rows are keyed by '
  '(message_id, bridge_id), so bridge_id disambiguates. The amb_ prefix is '
  'historical.';

-- ============================================================================
-- Cross-asset stats edges
--
-- stats_asset_edges carried a single stats_asset_id, so an edge could only say
-- "this asset moved from chain X to chain Y". True for lock/mint bridges
-- (AMB, ICTT); false for a converting bridge, where DAI@1 -> xDAI@100 is a
-- route *between two assets*. Split both the edge and the transfer link into a
-- source side and a destination side, and let the indexer declare which kind of
-- transfer it built.
-- ============================================================================

CREATE TYPE transfer_asset_linkage AS ENUM ('mirror', 'conversion');

-- ---------------------------------------------------------------------------
-- crosschain_transfers
-- ---------------------------------------------------------------------------
ALTER TABLE crosschain_transfers
  ADD COLUMN src_stats_asset_id BIGINT REFERENCES stats_assets(id) ON DELETE SET NULL,
  ADD COLUMN dst_stats_asset_id BIGINT REFERENCES stats_assets(id) ON DELETE SET NULL,
  ADD COLUMN asset_linkage      transfer_asset_linkage;

-- Every row that exists at this point was written by AMB or Avalanche, both
-- lock/mint, so src = dst = the old link and the linkage is 'mirror'. The xDai
-- bridge type is created by this same migration, so no xDai row can predate
-- this statement. Do NOT qualify this by bridge type -- referencing the 'xdai'
-- enum value here would fail, since this transaction is the one that added it.
UPDATE crosschain_transfers
SET    src_stats_asset_id = stats_asset_id,
       dst_stats_asset_id = stats_asset_id,
       asset_linkage      = 'mirror';

-- Implicitly drops crosschain_transfers_stats_asset_idx and the old FK.
ALTER TABLE crosschain_transfers DROP COLUMN stats_asset_id;

-- Built AFTER the UPDATE on purpose: building them first would add ~1.9M index
-- inserts to the update and rule out HOT by construction.
CREATE INDEX crosschain_transfers_src_stats_asset_idx
  ON crosschain_transfers (src_stats_asset_id);
CREATE INDEX crosschain_transfers_dst_stats_asset_idx
  ON crosschain_transfers (dst_stats_asset_id);
-- Serves both the deferral predicate and the operational "stuck rows" query.
CREATE INDEX crosschain_transfers_unclassified_idx
  ON crosschain_transfers (id) WHERE asset_linkage IS NULL;

-- ---------------------------------------------------------------------------
-- stats_asset_edges
-- ---------------------------------------------------------------------------
ALTER TABLE stats_asset_edges
  ADD COLUMN src_stats_asset_id BIGINT REFERENCES stats_assets(id) ON DELETE CASCADE,
  ADD COLUMN dst_stats_asset_id BIGINT REFERENCES stats_assets(id) ON DELETE CASCADE;

UPDATE stats_asset_edges
SET    src_stats_asset_id = stats_asset_id,
       dst_stats_asset_id = stats_asset_id;

ALTER TABLE stats_asset_edges
  ALTER COLUMN src_stats_asset_id SET NOT NULL,
  ALTER COLUMN dst_stats_asset_id SET NOT NULL;

ALTER TABLE stats_asset_edges DROP CONSTRAINT stats_asset_edges_pkey;
ALTER TABLE stats_asset_edges DROP COLUMN stats_asset_id;
ALTER TABLE stats_asset_edges
  ADD CONSTRAINT stats_asset_edges_pkey
  PRIMARY KEY (src_stats_asset_id, dst_stats_asset_id, bridge_id,
               src_chain_id, dst_chain_id);

-- Directional focal-chain reads, covering the opposite chain and the asset id
-- that side of the aggregate projects.
DROP INDEX IF EXISTS stats_asset_edges_src_chain_idx;
DROP INDEX IF EXISTS stats_asset_edges_dst_chain_idx;
CREATE INDEX stats_asset_edges_src_chain_idx
  ON stats_asset_edges (src_chain_id, bridge_id, dst_chain_id, src_stats_asset_id);
CREATE INDEX stats_asset_edges_dst_chain_idx
  ON stats_asset_edges (dst_chain_id, bridge_id, src_chain_id, dst_stats_asset_id);
-- FK-cascade cover for the non-leading asset column: the new PK leads with
-- src_stats_asset_id, so a stats_assets delete would otherwise seq-scan for
-- dst_stats_asset_id. Same reasoning as stats_asset_edges_bridge_idx.
CREATE INDEX stats_asset_edges_dst_asset_idx
  ON stats_asset_edges (dst_stats_asset_id);
