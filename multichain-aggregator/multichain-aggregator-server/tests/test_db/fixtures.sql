INSERT INTO chains (id, explorer_url, icon_url)
SELECT
    n,
    'https://chain-' || n || '.blockscout.com',
    'https://chain-' || n || '.blockscout.com/icon.png'
FROM generate_series(1, 5) n;

-- Generate addresses with different types
INSERT INTO addresses (
    hash,
    chain_id,
    ens_name,
    contract_name,
    token_name,
    token_type,
    is_contract,
    is_verified_contract,
    is_token
)
SELECT
    decode(lpad(to_hex(n * 256), 40, '0'), 'hex'),
    (mod(n, 5) + 1), -- distribute across chains 1-5
    CASE
        WHEN mod(n, 4) = 0 THEN 'test-' || n || '.eth'
        ELSE NULL
    END,
    CASE
        WHEN mod(n, 3) = 0 THEN 'Test Contract' || n
        ELSE NULL
    END,
    CASE
        WHEN mod(n, 5) = 0 THEN 'Test Token' || n
        ELSE NULL
    END,
    CASE
        WHEN mod(n, 7) > 4 THEN NULL
        ELSE CAST(CASE mod(n, 4)
            WHEN 0 THEN 'ERC-20'
            WHEN 1 THEN 'ERC-721'
            WHEN 2 THEN 'ERC-1155'
            ELSE 'ERC-404'
        END AS token_type)
    END,
    mod(n, 2) = 0,
    mod(n, 4) = 0,
    CASE
        WHEN mod(n, 7) = 6 THEN false
        ELSE mod(n, 3) = 0
    END
FROM generate_series(0, 999) n;

-- Generate hashes table
INSERT INTO hashes (
    hash,
    chain_id,
    hash_type
)
SELECT 
    decode(lpad(to_hex(n * 256), 64, '0'), 'hex'),
    (mod(n, 5) + 1), -- distribute across chains 1-5
    CAST(CASE mod(n, 2)
        WHEN 0 THEN 'block'
        ELSE 'transaction'
    END AS hash_type)
FROM generate_series(0, 999) n;
