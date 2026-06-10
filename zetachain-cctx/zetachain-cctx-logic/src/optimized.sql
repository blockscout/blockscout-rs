SELECT
    cctx.index,
    cs.status :: text,
    cs.last_update_timestamp,
    ip.amount,
    ip.sender_chain_id,
    cctx.receiver_chain_id AS receiver_chain_id,
    cs.created_timestamp AS created_timestamp,
    ip.sender AS sender_address,
    ip.asset AS asset,
    cctx.receiver :: text AS receiver_address,
    ip.coin_type :: text AS coin_type,
    (
        CASE
            WHEN cs.status :: text IN (
                'PendingOutbound',
                'PendingInbound',
                'PendingRevert'
            ) THEN 'Pending'
            WHEN cs.status :: text = 'OutboundMined' THEN 'Success'
            WHEN cs.status :: text IN ('Aborted', 'Reverted') THEN 'Failed'
            ELSE cs.status :: text
        END
    ) as status_reduced,
    t.symbol as token_symbol,
    t.zrc20_contract_address,
    t.decimals
FROM
    cross_chain_tx cctx
    join cctx_status cs on cs.cross_chain_tx_id = cctx.id
    INNER JOIN inbound_params ip ON cs.cross_chain_tx_id = ip.cross_chain_tx_id
    join token t on t.id = cctx.token_id
WHERE
    1=1

