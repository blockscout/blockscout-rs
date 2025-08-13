-- Initial migration: create tables to store SmartContract and its sources
-- SmartContract struct fields:
-- chain_id: String
-- address: alloy_primitives::Address (20 bytes)
-- blockscout_url: url::Url
-- sources: BTreeMap<String, String> (file_name -> content)

CREATE TABLE IF NOT EXISTS smart_contracts (
    id              BIGSERIAL PRIMARY KEY,
    chain_id        TEXT NOT NULL,
    -- Store address as 20-byte binary value
    address         BYTEA NOT NULL,
    blockscout_url  TEXT NOT NULL,

    -- Ensure address length is exactly 20 bytes
    CONSTRAINT smart_contracts_address_len CHECK (octet_length(address) = 20),

    -- A contract is uniquely identified by chain_id + address
    CONSTRAINT smart_contracts_chain_address_uniq UNIQUE (chain_id, address)
);

-- Sources of a smart contract: a mapping from file name to the content
CREATE TABLE IF NOT EXISTS smart_contract_sources (
    id           BIGSERIAL PRIMARY KEY,
    contract_id  BIGINT NOT NULL REFERENCES smart_contracts(id) ON DELETE CASCADE,
    file_name    TEXT NOT NULL,
    content      TEXT NOT NULL,

    CONSTRAINT smart_contract_sources_unique_file UNIQUE (contract_id, file_name)
);

-- Helpful index to lookup contracts by chain_id quickly (optional but cheap)
CREATE INDEX IF NOT EXISTS idx_smart_contracts_chain_id ON smart_contracts(chain_id);
