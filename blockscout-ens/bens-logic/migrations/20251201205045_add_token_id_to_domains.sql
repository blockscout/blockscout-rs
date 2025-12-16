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
        -- Dynamically construct and execute the SQL statement to add the token_id column
        EXECUTE format('
            ALTER TABLE %I.domain
            ADD COLUMN IF NOT EXISTS token_id NUMERIC;
        ', schema_name);
    END LOOP;
END $$;

