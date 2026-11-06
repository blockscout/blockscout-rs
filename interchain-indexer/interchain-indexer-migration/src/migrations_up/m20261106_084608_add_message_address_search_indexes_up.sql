-- Keyset-ordered address search on crosschain_messages: each arm of the
-- sender/recipient UNION ALL in get_crosschain_messages is one index range in
-- list order. IF NOT EXISTS lets ops pre-build them CONCURRENTLY out of band
-- (drop an INVALID leftover first).
CREATE INDEX IF NOT EXISTS crosschain_messages_sender_ts_idx
    ON crosschain_messages (sender_address, init_timestamp, id, bridge_id)
    WHERE sender_address IS NOT NULL;

CREATE INDEX IF NOT EXISTS crosschain_messages_recipient_ts_idx
    ON crosschain_messages (recipient_address, init_timestamp, id, bridge_id)
    WHERE recipient_address IS NOT NULL;

-- Superseded by the leading column above. Last: DROP INDEX holds ACCESS
-- EXCLUSIVE until the commit right after.
DROP INDEX IF EXISTS crosschain_messages_sender_idx;
DROP INDEX IF EXISTS crosschain_messages_recipient_idx;
