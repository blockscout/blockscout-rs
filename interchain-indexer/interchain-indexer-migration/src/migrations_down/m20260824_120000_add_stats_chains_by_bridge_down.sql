DROP INDEX crosschain_transfers_dst_user_by_chain_idx;
CREATE INDEX crosschain_transfers_dst_user_by_chain_idx
  ON crosschain_transfers (token_dst_chain_id, recipient_address)
  WHERE recipient_address IS NOT NULL;

DROP INDEX crosschain_transfers_src_user_by_chain_idx;
CREATE INDEX crosschain_transfers_src_user_by_chain_idx
  ON crosschain_transfers (token_src_chain_id, sender_address)
  WHERE sender_address IS NOT NULL;

DROP INDEX crosschain_messages_dst_user_by_chain_idx;
CREATE INDEX crosschain_messages_dst_user_by_chain_idx
  ON crosschain_messages (dst_chain_id, recipient_address)
  WHERE dst_chain_id IS NOT NULL AND recipient_address IS NOT NULL;

DROP INDEX crosschain_messages_src_user_by_chain_idx;
CREATE INDEX crosschain_messages_src_user_by_chain_idx
  ON crosschain_messages (src_chain_id, sender_address)
  WHERE sender_address IS NOT NULL;

DROP TABLE stats_chains_by_bridge;
