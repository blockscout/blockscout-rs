SELECT transaction_id, block_number, array_agg(table_name) as actions
FROM (
    SELECT distinct on (transaction_id, table_name) *
    FROM (
        SELECT 'transfer' as table_name, block_number, transaction_id
        FROM sgd1.transfer
        WHERE domain = $1
            UNION ALL
        SELECT 'new_owner' as table_name, block_number, transaction_id
        FROM sgd1.new_owner
        WHERE domain = $1
            UNION ALL
        SELECT 'new_resolver' as table_name, block_number, transaction_id
        FROM sgd1.new_resolver
        WHERE domain = $1
            UNION ALL
        SELECT 'new_ttl' as table_name, block_number, transaction_id
        FROM sgd1.new_ttl
        WHERE domain = $1
            UNION ALL
        SELECT 'wrapped_transfer' as table_name, block_number, transaction_id
        FROM sgd1.wrapped_transfer
        WHERE domain = $1
            UNION ALL
        SELECT 'name_wrapped' as table_name, block_number, transaction_id
        FROM sgd1.name_wrapped
        WHERE domain = $1
            UNION ALL
        SELECT 'name_unwrapped' as table_name, block_number, transaction_id
        FROM sgd1.name_unwrapped
        WHERE domain = $1
            UNION ALL
        SELECT 'fuses_set' as table_name, block_number, transaction_id
        FROM sgd1.fuses_set
        WHERE domain = $1
            UNION ALL
        SELECT 'expiry_extended' as table_name, block_number, transaction_id
        FROM sgd1.expiry_extended
        WHERE domain = $1
            UNION ALL
        SELECT 'addr_changed' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.addr_changed t
        JOIN sgd1.resolver r 
        ON t.resolver = r.id
        WHERE r.domain = $1
            UNION ALL
        SELECT 'multicoin_addr_changed' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.multicoin_addr_changed t
        JOIN sgd1.resolver r 
        ON t.resolver = r.id
        WHERE r.domain = $1
            UNION ALL
        SELECT 'name_changed' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.name_changed t
        JOIN sgd1.resolver r 
        ON t.resolver = r.id
        WHERE r.domain = $1
            UNION ALL
        SELECT 'abi_changed' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.abi_changed t
        JOIN sgd1.resolver r 
        ON t.resolver = r.id
        WHERE r.domain = $1
            UNION ALL
        SELECT 'pubkey_changed' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.pubkey_changed t
        JOIN sgd1.resolver r 
        ON t.resolver = r.id
        WHERE r.domain = $1
            UNION ALL
        SELECT 'text_changed' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.text_changed t
        JOIN sgd1.resolver r 
        ON t.resolver = r.id
        WHERE r.domain = $1
            UNION ALL
        SELECT 'contenthash_changed' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.contenthash_changed t
        JOIN sgd1.resolver r 
        ON t.resolver = r.id
        WHERE r.domain = $1
            UNION ALL
        SELECT 'interface_changed' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.interface_changed t
        JOIN sgd1.resolver r 
        ON t.resolver = r.id
        WHERE r.domain = $1
            UNION ALL
        SELECT 'authorisation_changed' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.authorisation_changed t
        JOIN sgd1.resolver r 
        ON t.resolver = r.id
        WHERE r.domain = $1
            UNION ALL
        SELECT 'version_changed' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.version_changed t
        JOIN sgd1.resolver r 
        ON t.resolver = r.id
        WHERE r.domain = $1
            UNION ALL
        SELECT 'name_registered' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.name_registered t
        JOIN sgd1.registration r
        ON t.registration = r.id
        WHERE r.domain = $1
        UNION ALL
        SELECT 'name_renewed' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.name_renewed t
        JOIN sgd1.registration r
        ON t.registration = r.id
        WHERE r.domain = $1
        UNION ALL
        SELECT 'name_transferred' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.name_transferred t
        JOIN sgd1.registration r
        ON t.registration = r.id
        WHERE r.domain = $1
        ) all_events
    ORDER BY transaction_id
) unique_events
GROUP BY transaction_id, block_number
ORDER BY block_number asc

