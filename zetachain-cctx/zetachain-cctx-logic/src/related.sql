WITH seed AS (
  SELECT
    id,
    CASE
      WHEN root_id = id THEN id
      ELSE root_id
    END AS rid
  FROM
    cross_chain_tx
  WHERE
    index = $1
  LIMIT
    1
), ohash AS (
  SELECT
    ip.observed_hash
  FROM
    inbound_params ip
    JOIN seed ON ip.cross_chain_tx_id = seed.rid
  LIMIT
    1
), roots AS (
  SELECT
    DISTINCT ip2.cross_chain_tx_id as root_id
  FROM
    ohash h
    JOIN inbound_params ip2 ON ip2.observed_hash = h.observed_hash
)
SELECT
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
  JOIN token t on t.id = child.token_id
  JOIN cctx_status cs on child.id = cs.cross_chain_tx_id
ORDER BY
  child.root_id,
  child.depth;