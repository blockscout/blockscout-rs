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
