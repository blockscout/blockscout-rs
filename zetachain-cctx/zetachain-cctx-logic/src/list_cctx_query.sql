select
    *
from
    (
        WITH 
        gas_tokens AS (
            SELECT
                DISTINCT foreign_chain_id,
                symbol,
                zrc20_contract_address,
                decimals
            FROM
                token
            WHERE
                coin_type = 'Gas'
        ),
        first_outbound AS (
            SELECT
                DISTINCT ON (cross_chain_tx_id) cross_chain_tx_id,
                receiver,
                receiver_chain_id
            FROM
                outbound_params
            ORDER BY
                cross_chain_tx_id,
                id ASC
        )
        SELECT
            cctx.index,
            cs.status :: text,
            cs.last_update_timestamp,
            ip.amount,
            ip.sender_chain_id,
            fo.receiver_chain_id AS receiver_chain_id,
            cs.created_timestamp AS created_timestamp,
            ip.sender AS sender_address,
            ip.asset AS asset,
            fo.receiver :: text AS receiver_address,
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
            COALESCE(t.symbol, gt.symbol) as token_symbol,
            COALESCE(
                t.zrc20_contract_address,
                gt.zrc20_contract_address
            ) as zrc20_contract_address,
            COALESCE(t.decimals, gt.decimals) as decimals,
            cctx.id
        FROM
            cross_chain_tx cctx
            INNER JOIN cctx_status cs ON cctx.id = cs.cross_chain_tx_id
            INNER JOIN inbound_params ip ON cctx.id = ip.cross_chain_tx_id
            INNER JOIN first_outbound fo ON cctx.id = fo.cross_chain_tx_id
            LEFT JOIN token t ON ip.asset = t.asset
            LEFT JOIN gas_tokens gt ON gt.foreign_chain_id = ip.sender_chain_id
    ) d
WHERE
    1 = 1