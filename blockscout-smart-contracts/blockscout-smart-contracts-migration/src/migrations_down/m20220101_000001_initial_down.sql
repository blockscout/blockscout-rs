-- Down migration: drop tables created by the initial migration
DROP INDEX IF EXISTS idx_smart_contracts_chain_id;
DROP TABLE IF EXISTS smart_contract_sources;
DROP TABLE IF EXISTS smart_contracts;
