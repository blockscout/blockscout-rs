INSERT INTO blocks (consensus, gas_limit, gas_used, hash, miner_hash, nonce, number, parent_hash, "timestamp",
                    inserted_at, updated_at)
SELECT true,
       0,
       0,
       decode(lpad(to_hex(n * 256), 64, '0'), 'hex'),
       '\x',
       '\x',
       n,
       '\x',
       '2024-01-01 00:00:00'::timestamp + interval '12 seconds' * n, now(), now()
FROM generate_series(0, 999) n;

INSERT INTO user_operations (hash, sender, nonce, call_data, call_gas_limit, verification_gas_limit,
                             pre_verification_gas, max_fee_per_gas, max_priority_fee_per_gas, signature, entry_point,
                             transaction_hash, block_number, block_hash, bundle_index, index, user_logs_start_index,
                             user_logs_count, bundler, status, gas, gas_price, gas_used, sponsor_type)
SELECT decode(lpad(to_hex(n * 256 + 1), 64, '0'), 'hex'),
       decode(lpad(to_hex(mod(n, 100) * 256 + 2), 40, '0'), 'hex'),
       decode(lpad(to_hex(n * 256 + 3), 64, '0'), 'hex'),
       '\x',
       1000000 + n,
       2000000 + n,
       3000000 + n,
       4000000 + n,
       5000000 + n,
       '\x',
       '\x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789',
       decode(lpad(to_hex(n * 256 + 4), 64, '0'), 'hex'),
       n / 10,
       decode(lpad(to_hex(n / 10 * 256), 64, '0'), 'hex'),
       0,
       0,
       mod(n, 100),
       5,
       decode(lpad(to_hex(mod(n, 100) * 256 + 5), 40, '0'), 'hex'),
       true,
       6000000 + n,
       7000000 + n,
       8000000 + n,
       'wallet_deposit'
FROM generate_series(0, 9999) n;

UPDATE blocks
SET consensus = false
WHERE number = 666;

DELETE
FROM blocks
WHERE number = 667;

UPDATE user_operations
SET factory = '\x00000000000000000000000000000000000000f1'
WHERE block_number = 5;

UPDATE user_operations
SET factory = '\x00000000000000000000000000000000000000f2'
WHERE block_number = 6;

UPDATE user_operations
SET index            = 1,
    block_hash       = '\x0000000000000000000000000000000000000000000000000000000000000000',
    block_number     = 0,
    transaction_hash = '\x0000000000000000000000000000000000000000000000000000000000000504'
WHERE transaction_hash = '\x0000000000000000000000000000000000000000000000000000000000006904';

UPDATE user_operations
SET paymaster    = '\x00000000000000000000000000000000000000e1',
    sponsor_type = 'paymaster_sponsor'
WHERE block_number = 20;

UPDATE user_operations
SET paymaster    = '\x00000000000000000000000000000000000000e2',
    sponsor_type = 'paymaster_sponsor'
WHERE block_number = 21;

INSERT INTO logs (data, index, type, first_topic, second_topic, third_topic, fourth_topic, inserted_at, updated_at,
                  address_hash, transaction_hash, block_hash, block_number)
VALUES ('\x', 0, NULL, '0x49628fd1471006c1482da88028e9ce4dbb080b815c9b0344d39e5a8e6ec1419f', NULL, NULL, NULL, now(),
        now(), '\x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789',
        '\x000000000000000000000000000000000000000000000000000000000000ffff',
        '\x000000000000000000000000000000000000000000000000000000000000ff00', 123);
