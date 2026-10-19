-- ---------------------------------------------------------------------------
-- stats_chains_by_bridge: bridge-qualified per-chain distinct user counters.
--
-- Companion to the globally deduplicated stats_chains, not a replacement:
-- summing these rows would make an unfiltered request WRONG once two bridges
-- see the same address on the same chain. Both snapshots are rebuilt together,
-- in one transaction, by the same periodic worker.
-- ---------------------------------------------------------------------------
CREATE TABLE stats_chains_by_bridge (
  bridge_id                    INTEGER NOT NULL
    REFERENCES bridges(id) ON DELETE CASCADE,
  chain_id                     BIGINT NOT NULL
    REFERENCES chains(id) ON DELETE CASCADE,

  unique_transfer_users_count  BIGINT NOT NULL DEFAULT 0,
  unique_message_users_count   BIGINT NOT NULL DEFAULT 0,

  created_at                   TIMESTAMP NOT NULL DEFAULT now(),
  updated_at                   TIMESTAMP NOT NULL DEFAULT now(),

  PRIMARY KEY (bridge_id, chain_id)
);

-- The bridge-leading PK serves `WHERE bridge_id IN (...)` reads and the bridge
-- FK cascade; this one covers the chain FK cascade.
CREATE INDEX stats_chains_by_bridge_chain_idx
  ON stats_chains_by_bridge (chain_id);

-- ---------------------------------------------------------------------------
-- Keep the recomputation index-only.
--
-- m20260312_175120_add_stats_tables created these four partial indexes for the
-- express purpose of making the stats_chains rebuild an Index Only Scan. Adding
-- bridge_id to the projected distinct tuple takes the query outside that
-- covering set and silently degrades it to a heap/sequential scan of the two
-- largest tables in the schema. Rebuild them with bridge_id as a TRAILING key
-- column: leading columns and partial predicates are unchanged, so any other
-- plan using the (chain_id, address) prefix is unaffected, and each union arm
-- now arrives already ordered on (chain_id, address, bridge_id).
-- ---------------------------------------------------------------------------
DROP INDEX crosschain_messages_src_user_by_chain_idx;
CREATE INDEX crosschain_messages_src_user_by_chain_idx
  ON crosschain_messages (src_chain_id, sender_address, bridge_id)
  WHERE sender_address IS NOT NULL;

DROP INDEX crosschain_messages_dst_user_by_chain_idx;
CREATE INDEX crosschain_messages_dst_user_by_chain_idx
  ON crosschain_messages (dst_chain_id, recipient_address, bridge_id)
  WHERE dst_chain_id IS NOT NULL AND recipient_address IS NOT NULL;

DROP INDEX crosschain_transfers_src_user_by_chain_idx;
CREATE INDEX crosschain_transfers_src_user_by_chain_idx
  ON crosschain_transfers (token_src_chain_id, sender_address, bridge_id)
  WHERE sender_address IS NOT NULL;

DROP INDEX crosschain_transfers_dst_user_by_chain_idx;
CREATE INDEX crosschain_transfers_dst_user_by_chain_idx
  ON crosschain_transfers (token_dst_chain_id, recipient_address, bridge_id)
  WHERE recipient_address IS NOT NULL;
