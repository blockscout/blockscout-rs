ALTER TYPE bridge_type ADD VALUE IF NOT EXISTS 'op_interop';

-- Non-terminal message statuses (ADR-021) written by the OP interop
-- indexer's optimistic overlay. Nothing in this migration uses them: a value
-- added by ALTER TYPE cannot be used before its transaction commits.
ALTER TYPE message_status ADD VALUE IF NOT EXISTS 'pending_finality';
ALTER TYPE message_status ADD VALUE IF NOT EXISTS 'invalidated';

-- The OP interop event ledger, which is also the message journal (ADR-018,
-- ADR-020). One row per observed SentMessage, RelayedMessage and
-- ExecutingMessage log (SendETH / RelayETH are cross-checks, not rows); rows
-- are never deleted. A row's `state` only moves away from 'provisional'; see
-- the column comments. Every CrossL2Inbox ExecutingMessage has a row whatever
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
    -- The message key (equal to crosschain_messages.id once the message row
    -- exists). Set when the leg is read for sent_message / relayed_message
    -- rows, and by the flush for the delivering execution; never cleared. A
    -- leg whose counterpart is never read may point to an id with no message
    -- row (no foreign key). NULL on an executing_message row that delivered
    -- no indexed message: a raw execution.
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
    -- Journal state (ADR-020). 'provisional': read above the confirmation
    -- tag by the optimistic overlay. 'confirmed': delivered by the confirmed
    -- stream. 'retracted': provisional, and the confirmed stream passed its
    -- height without delivering it. Moves only provisional -> confirmed |
    -- retracted; rows are never deleted.
    state             TEXT      NOT NULL DEFAULT 'confirmed',
    -- UTC detection times, each set once, at its transition. A 'confirmed'
    -- row with NULL confirmed_seen_at was confirmed when first seen
    -- (created_at bounds the time).
    confirmed_seen_at TIMESTAMP,
    retracted_seen_at TIMESTAMP,
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

-- The confirmation sweep: provisional rows of a chain at or below its frontier.
CREATE INDEX idx_op_interop_events_provisional
    ON op_interop_events (bridge_id, chain_id, block_number)
    WHERE state = 'provisional';
