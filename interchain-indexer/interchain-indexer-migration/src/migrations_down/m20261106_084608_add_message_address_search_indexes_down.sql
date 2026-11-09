CREATE INDEX IF NOT EXISTS crosschain_messages_sender_idx
    ON crosschain_messages (sender_address)
    WHERE sender_address IS NOT NULL;

CREATE INDEX IF NOT EXISTS crosschain_messages_recipient_idx
    ON crosschain_messages (recipient_address)
    WHERE recipient_address IS NOT NULL;

DROP INDEX IF EXISTS crosschain_messages_sender_ts_idx;
DROP INDEX IF EXISTS crosschain_messages_recipient_ts_idx;
