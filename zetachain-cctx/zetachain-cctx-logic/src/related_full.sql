WITH seed AS (
 SELECT
    cctx.index,
    cs.status::text as status,
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
    cctx.id,
    cctx.token_id,
    case when cctx.root_id = cctx.id then cctx.id else cctx.root_id end as rid
FROM
    cross_chain_tx cctx
    INNER JOIN token t on t.id = cctx.token_id
    INNER JOIN cctx_status cs ON cctx.id = cs.cross_chain_tx_id
    INNER JOIN inbound_params ip ON cctx.id = ip.cross_chain_tx_id
WHERE
    cctx.id in (6945, 19404, 42494)
), ohash AS (
  SELECT
    seed.id as seed_id,ip.observed_hash
  FROM
    inbound_params ip
    JOIN seed ON ip.cross_chain_tx_id = seed.rid
  LIMIT
    10
), roots AS (
  SELECT
    DISTINCT ip2.cross_chain_tx_id as root_id,
    h.seed_id
  FROM
    ohash h
    JOIN inbound_params ip2 ON ip2.observed_hash = h.observed_hash
)
SELECT
  r.seed_id,
  child.index,
  child.depth,
  ip.sender_chain_id as chain_id,
  cs.status :: text as status,
  cs.created_timestamp,
  ip.amount,
  ip.coin_type :: text,
  ip.asset,
  t.name,
  t.symbol,
  t.decimals,
  t.zrc20_contract_address,
  t.icon_url,
  child.id as related_cctx_id
FROM
  cross_chain_tx child
  JOIN roots r ON child.root_id = r.root_id
  JOIN inbound_params ip on child.id = ip.cross_chain_tx_id
  LEFT JOIN token t on t.id = child.token_id
  JOIN cctx_status cs on child.id = cs.cross_chain_tx_id
ORDER BY
  child.root_id,
  child.depth;