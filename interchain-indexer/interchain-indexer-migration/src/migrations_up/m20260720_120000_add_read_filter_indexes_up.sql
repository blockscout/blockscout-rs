-- bridge-filtered keyset pagination on messages
CREATE INDEX crosschain_messages_bridge_ts_idx
    ON crosschain_messages (bridge_id, init_timestamp, id);

-- bridge filter on transfers (chain filters are served by the leading
-- columns of crosschain_transfers_token_src_idx / _token_dst_idx)
CREATE INDEX crosschain_transfers_bridge_idx
    ON crosschain_transfers (bridge_id);
