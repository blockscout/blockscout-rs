ALTER TYPE bridge_type ADD VALUE IF NOT EXISTS 'amb';
ALTER TYPE message_status ADD VALUE IF NOT EXISTS 'ready_to_claim';

ALTER TABLE bridge_contracts
    ADD COLUMN IF NOT EXISTS kind text;

CREATE TABLE amb_messages_confirmations (
    message_id          BIGINT      NOT NULL,
    bridge_id           INTEGER     NOT NULL,
    validator_address   BYTEA       NOT NULL,
    tx_hash             BYTEA       NOT NULL,
    block_number        BIGINT      NOT NULL,
    block_timestamp     TIMESTAMP   NOT NULL,
    created_at          TIMESTAMP   DEFAULT CURRENT_TIMESTAMP,
    updated_at          TIMESTAMP   DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (message_id, bridge_id, validator_address),
    FOREIGN KEY (message_id, bridge_id)
        REFERENCES crosschain_messages (id, bridge_id)
        ON DELETE CASCADE
);

CREATE INDEX idx_amb_messages_confirmations_message_block
    ON amb_messages_confirmations (message_id, bridge_id, block_number);

-- Internal-only capture of AMB messageId collisions: bodies that share a
-- structured messageId (same version+bridgeId+nonce) but belong to different
-- messages. The executed body wins the canonical crosschain_messages row; the
-- displaced body lands here instead of being silently dropped. No FK to
-- crosschain_messages by design — the anomalous record intentionally lacks a
-- consistent canonical row.
CREATE TABLE amb_message_anomalies (
    id                BIGSERIAL  PRIMARY KEY,
    bridge_id         INTEGER    NOT NULL,
    buffer_key        BIGINT     NOT NULL,  -- = crosschain_messages.id it collided on
    native_id         BYTEA      NOT NULL,  -- raw 32-byte AMB messageId
    event_kind        TEXT       NOT NULL,  -- 'source_request' | 'destination_execution'
    chain_id          BIGINT     NOT NULL,
    tx_hash           BYTEA      NOT NULL,
    log_index         BIGINT,
    block_number      BIGINT     NOT NULL,
    block_timestamp   TIMESTAMP  NOT NULL,
    sender            BYTEA,
    executor          BYTEA,
    src_chain_id      BIGINT,
    dst_chain_id      BIGINT,
    encoded_data      BYTEA,                -- full body for source_request; NULL for destination_execution
    conflict_sender   BYTEA,
    conflict_executor BYTEA,
    conflict_tx_hash  BYTEA,
    detail            TEXT,
    created_at        TIMESTAMP  DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_amb_message_anomalies_native_id
    ON amb_message_anomalies (bridge_id, native_id);
