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
FROM generate_series(0, 1000) n;

INSERT INTO user_operations (op_hash, sender, nonce, call_data, call_gas_limit, verification_gas_limit,
                             pre_verification_gas, max_fee_per_gas, max_priority_fee_per_gas, signature, entry_point,
                             tx_hash, block_number, block_hash, bundle_index, op_index, user_logs_start_index,
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
FROM generate_series(0, 10000) n;

UPDATE blocks
SET consensus = false
WHERE number = 666;

UPDATE user_operations
SET factory = '\x00000000000000000000000000000000000000f1'
WHERE block_number = 5;

UPDATE user_operations
SET factory = '\x00000000000000000000000000000000000000f2'
WHERE block_number = 6;