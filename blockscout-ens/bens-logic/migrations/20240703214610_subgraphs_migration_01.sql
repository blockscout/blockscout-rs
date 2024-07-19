DO $$
DECLARE
    schema_name text;
BEGIN
    -- Loop through schemas that match the pattern 'sgd%'
    FOR schema_name IN
        SELECT schemata.schema_name
        FROM information_schema.schemata AS schemata
        WHERE schemata.schema_name LIKE 'sgd%'
    LOOP
        -- Dynamically construct and execute the SQL statement to add the columns
        EXECUTE format('
            ALTER TABLE %I.domain
            ADD COLUMN IF NOT EXISTS stored_offchain BOOLEAN NOT NULL DEFAULT FALSE,
            ADD COLUMN IF NOT EXISTS resolved_with_wildcard BOOLEAN NOT NULL DEFAULT FALSE;
        ', schema_name);
    END LOOP;
END $$;
