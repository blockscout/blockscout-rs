WITH selected AS (
    SELECT
        cctx.ctid,
        cctx.id
    FROM
        cross_chain_tx cctx
    WHERE
        cctx.processing_status = $1 :: processing_status
        AND cctx.next_poll < NOW()
    ORDER BY
        cctx.last_status_update_timestamp DESC
    LIMIT
        $2 FOR
    UPDATE
        OF cctx SKIP LOCKED
)
UPDATE
    cross_chain_tx AS cctx
SET
    processing_status = 'Locked' :: processing_status,
    last_status_update_timestamp = NOW(),
    retries_number = cctx.retries_number + 1,
    next_poll = NOW() +  $3::bigint * INTERVAL '1 milliseconds' * POWER(2, cctx.retries_number + 1)
FROM
    selected
WHERE
    cctx.ctid = selected.ctid RETURNING cctx.id,
    cctx.index,
    cctx.root_id,
    cctx.depth,
    cctx.retries_number,
    cctx.token_id;