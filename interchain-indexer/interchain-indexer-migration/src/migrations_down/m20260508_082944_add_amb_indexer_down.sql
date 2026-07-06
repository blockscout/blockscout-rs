-- Restoring NOT NULL requires backfilling the rows the nullable era allowed.
-- Fill each unknown side with a neutral sentinel (zero address / zero amount)
-- rather than mirroring the known side: mirroring is exactly the incorrect
-- representation this migration removed (it conflated source/destination tokens
-- and corrupted stats projection), so a rollback must not reintroduce it.
UPDATE crosschain_transfers
SET token_src_address = COALESCE(token_src_address, '\x0000000000000000000000000000000000000000'::bytea),
    token_dst_address = COALESCE(token_dst_address, '\x0000000000000000000000000000000000000000'::bytea),
    src_amount        = COALESCE(src_amount, 0),
    dst_amount        = COALESCE(dst_amount, 0)
WHERE token_src_address IS NULL
   OR token_dst_address IS NULL
   OR src_amount IS NULL
   OR dst_amount IS NULL;

ALTER TABLE crosschain_transfers
    ALTER COLUMN src_amount        SET NOT NULL,
    ALTER COLUMN dst_amount        SET NOT NULL,
    ALTER COLUMN token_src_address SET NOT NULL,
    ALTER COLUMN token_dst_address SET NOT NULL;

DROP TABLE IF EXISTS amb_message_anomalies;
DROP TABLE amb_messages_confirmations;
ALTER TABLE bridge_contracts DROP COLUMN IF EXISTS kind;
