-- Keyset-ordered address search on crosschain_messages: each arm of the
-- sender/recipient UNION in get_crosschain_messages is one index range in list
-- order. The INCLUDE columns let an arm evaluate the ChainBridgeFilter (bridge
-- and chain ids only) as an index-only scan, so rows the scope rejects never
-- cost heap reads. IF NOT EXISTS lets ops pre-build them CONCURRENTLY out of band
-- (drop an INVALID leftover first).
CREATE INDEX IF NOT EXISTS crosschain_messages_sender_ts_idx
    ON crosschain_messages (sender_address, init_timestamp, id, bridge_id)
    INCLUDE (src_chain_id, dst_chain_id)
    WHERE sender_address IS NOT NULL;

CREATE INDEX IF NOT EXISTS crosschain_messages_recipient_ts_idx
    ON crosschain_messages (recipient_address, init_timestamp, id, bridge_id)
    INCLUDE (src_chain_id, dst_chain_id)
    WHERE recipient_address IS NOT NULL;

-- Superseded by the leading column above. Last: DROP INDEX holds ACCESS
-- EXCLUSIVE until the commit right after.
DROP INDEX IF EXISTS crosschain_messages_sender_idx;
DROP INDEX IF EXISTS crosschain_messages_recipient_idx;
