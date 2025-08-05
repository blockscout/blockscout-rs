select
    *
from
    (
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
            t.decimals,
            cctx.id
        FROM
            cross_chain_tx cctx
            INNER JOIN token t on t.id = cctx.token_id
            INNER JOIN cctx_status cs ON cctx.id = cs.cross_chain_tx_id
            INNER JOIN inbound_params ip ON cctx.id = ip.cross_chain_tx_id

    ) d
WHERE
    1 = 1
