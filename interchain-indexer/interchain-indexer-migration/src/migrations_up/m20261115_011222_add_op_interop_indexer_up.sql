ALTER TYPE bridge_type ADD VALUE IF NOT EXISTS 'op_interop';

-- One row per observed OP interop log. Keyed by block hash, not block
-- number, so two observations of one height can coexist later.
-- No foreign key to crosschain_messages by design: message_id links by
-- value, and a leg row must be able to outlive a deleted message.
CREATE TABLE op_interop_legs (
    bridge_id        INTEGER   NOT NULL REFERENCES bridges (id),
    chain_id         BIGINT    NOT NULL REFERENCES chains (id),
    block_hash       BYTEA     NOT NULL,
    log_index        BIGINT    NOT NULL,
    block_number     BIGINT    NOT NULL,
    block_timestamp  TIMESTAMP NOT NULL,
    tx_hash          BYTEA     NOT NULL,
    event_kind       TEXT      NOT NULL,  -- 'sent_message' | 'executing_message' | 'relayed_message'
    message_id       BIGINT    NOT NULL,  -- = crosschain_messages.id
    message_hash     BYTEA     NOT NULL,  -- messenger messageHash, 32 bytes
    event_data       JSONB     NOT NULL,
    created_at       TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at       TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (bridge_id, chain_id, block_hash, log_index)
);

CREATE INDEX idx_op_interop_legs_message
    ON op_interop_legs (bridge_id, message_id);
