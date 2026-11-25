ALTER TYPE bridge_type ADD VALUE IF NOT EXISTS 'op_interop';

-- The OP interop event ledger, which is also the message journal (ADR-018,
-- ADR-020). One row per observed SentMessage, RelayedMessage and
-- ExecutingMessage log (SendETH / RelayETH are cross-checks, not rows); rows
-- are never deleted. Every CrossL2Inbox ExecutingMessage has a row whatever
-- its origin, and only some rows are legs of a messenger message. Keyed by
-- block hash, not block number, so two observations of one height can
-- coexist.
-- No foreign key to crosschain_messages by design: message_id links by
-- value, and a row must be able to outlive a deleted message.
CREATE TABLE op_interop_events (
    bridge_id        INTEGER   NOT NULL REFERENCES bridges (id),
    chain_id         BIGINT    NOT NULL REFERENCES chains (id),
    block_hash       BYTEA     NOT NULL,
    log_index        BIGINT    NOT NULL,
    block_number     BIGINT    NOT NULL,
    block_timestamp  TIMESTAMP NOT NULL,
    tx_hash          BYTEA     NOT NULL,
    event_kind       TEXT      NOT NULL,  -- 'sent_message' | 'executing_message' | 'relayed_message'
    -- = crosschain_messages.id. NULL on an executing_message row that
    -- delivered no indexed message: a raw execution. Set by the flush, never
    -- cleared.
    message_id       BIGINT,
    message_hash     BYTEA,               -- messenger messageHash, 32 bytes; NULL exactly when message_id is
    -- The PRD Identifier join key of the source log (Identifier.chainId,
    -- .blockNumber, .logIndex), on executing_message rows only. NULL when the
    -- on-chain uint256 does not fit BIGINT; event_data keeps the verbatim
    -- value. No foreign key: same-chain reads and reads of chains outside the
    -- bridge are legitimate.
    src_chain_id     BIGINT,
    src_block_number BIGINT,
    src_log_index    BIGINT,
    event_data       JSONB     NOT NULL,
    created_at       TIMESTAMP DEFAULT CURRENT_TIMESTAMP,  -- first persisted, by either writer
    updated_at       TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (bridge_id, chain_id, block_hash, log_index)
);

CREATE INDEX idx_op_interop_events_message
    ON op_interop_events (bridge_id, message_id);

-- Every execution of one source log: the raw message of the PRD.
CREATE INDEX idx_op_interop_events_identifier
    ON op_interop_events (bridge_id, src_chain_id, src_block_number, src_log_index)
    WHERE event_kind = 'executing_message';
