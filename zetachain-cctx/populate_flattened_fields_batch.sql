-- Batch version for safer execution on large datasets
-- This script updates records in batches to avoid long-running transactions

-- First, let's see how many records we need to update
SELECT COUNT(*) as total_records FROM cross_chain_tx;

-- Update in batches of 1000 records
DO $$
DECLARE
    batch_size INTEGER := 1000;
    total_updated INTEGER := 0;
    batch_count INTEGER := 0;
    max_id INTEGER;
    current_max_id INTEGER := 0;
BEGIN
    -- Get the maximum ID to process
    SELECT MAX(id) INTO max_id FROM cross_chain_tx;
    
    WHILE current_max_id < max_id LOOP
        -- Update batch
        WITH batch_update AS (
            UPDATE cross_chain_tx 
            SET 
                -- Calculate token_id
                token_id = CASE 
                    -- ERC20 tokens
                    WHEN ip.coin_type::text = 'Erc20' THEN
                        (SELECT id FROM token WHERE asset = ip.asset)
                    -- Gas/Zeta tokens with sender_chain_id != ZetaChain
                    WHEN ip.coin_type::text IN ('Gas', 'Zeta') AND ip.sender_chain_id::integer != 7000 THEN
                        (SELECT id FROM token 
                         WHERE coin_type::text = ip.coin_type::text 
                         AND foreign_chain_id = ip.sender_chain_id::integer)
                    -- Gas/Zeta tokens with sender_chain_id = ZetaChain (use receiver_chain_id from outbound)
                    WHEN ip.coin_type::text IN ('Gas', 'Zeta') AND ip.sender_chain_id::integer = 7000 THEN
                        (SELECT id FROM token 
                         WHERE coin_type::text = ip.coin_type::text 
                         AND foreign_chain_id = op.receiver_chain_id::integer)
                    ELSE NULL
                END,
                
                -- Get receiver and receiver_chain_id from first outbound_params
                receiver = op.receiver,
                receiver_chain_id = op.receiver_chain_id::integer

            FROM inbound_params ip
            LEFT JOIN (
                -- Get the first outbound_params for each cross_chain_tx_id
                SELECT DISTINCT ON (cross_chain_tx_id) 
                       cross_chain_tx_id, receiver, receiver_chain_id
                FROM outbound_params 
                ORDER BY cross_chain_tx_id, id ASC
            ) op ON cross_chain_tx.id = op.cross_chain_tx_id

            WHERE cross_chain_tx.id = ip.cross_chain_tx_id
            AND cross_chain_tx.id > current_max_id
            AND cross_chain_tx.id <= current_max_id + batch_size
            RETURNING id
        )
        SELECT COUNT(*) INTO batch_count FROM batch_update;
        
        total_updated := total_updated + batch_count;
        current_max_id := current_max_id + batch_size;
        
        -- Log progress
        RAISE NOTICE 'Processed batch: % records updated, total: %, current_max_id: %', 
                    batch_count, total_updated, current_max_id;
        
        -- Commit this batch
        COMMIT;
    END LOOP;
    
    RAISE NOTICE 'Migration completed. Total records updated: %', total_updated;
END $$;

-- Verify the results
SELECT 
    COUNT(*) as total_records,
    COUNT(token_id) as records_with_token_id,
    COUNT(receiver) as records_with_receiver,
    COUNT(receiver_chain_id) as records_with_receiver_chain_id
FROM cross_chain_tx;

-- Show some sample results
SELECT 
    cctx.id,
    cctx.token_id,
    cctx.receiver,
    cctx.receiver_chain_id,
    ip.coin_type,
    ip.asset,
    ip.sender_chain_id,
    t.symbol as token_symbol
FROM cross_chain_tx cctx
JOIN inbound_params ip ON cctx.id = ip.cross_chain_tx_id
LEFT JOIN token t ON cctx.token_id = t.id
WHERE cctx.token_id IS NOT NULL
ORDER BY cctx.id DESC
LIMIT 10; 