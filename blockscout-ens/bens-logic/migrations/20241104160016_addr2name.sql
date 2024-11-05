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
            CREATE TABLE %I.addr2name (
                resolved_address TEXT PRIMARY KEY,
                domain_id TEXT,
                domain_name TEXT
            );
        ', schema_name);
    END LOOP;
END $$;
