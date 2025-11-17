-- chains: supported chains list (e.g. Ethereum, Gnosis, Avalanche...)
CREATE TABLE chains (
  id          BIGINT PRIMARY KEY, -- the classic EVM chain_id, 8 bytes long
                                  -- or arbitrary unique number if not applicable
  name        TEXT NOT NULL UNIQUE,
  native_id   TEXT, -- the optional original network identifier,
                    -- for cases where the chain_id is not applicable
                    -- e.g. Avalanche chains has 32-byte blockchainID
  icon        TEXT,

  created_at  TIMESTAMP DEFAULT now(),
  updated_at  TIMESTAMP DEFAULT now()
);

CREATE TYPE bridge_type AS ENUM ('lockmint', 'avalanche_native');

-- bridges: supported bridges list (OmniBridge, LayerZero, Wormhole ...)
CREATE TABLE bridges (
  id          SERIAL PRIMARY KEY,
  name        TEXT NOT NULL UNIQUE,
  type        bridge_type,  -- lockmint (native, .... TBD)
  enabled     BOOLEAN NOT NULL DEFAULT TRUE,
  api_url     TEXT,
  ui_url      TEXT,
  
  created_at  TIMESTAMP DEFAULT now(),
  updated_at  TIMESTAMP DEFAULT now()
);

-- bridge_contracts: bridge contracts per chain (if available)
CREATE TABLE bridge_contracts (
  id                BIGSERIAL PRIMARY KEY,
  bridge_id         INTEGER NOT NULL REFERENCES bridges(id),
  chain_id          BIGINT NOT NULL REFERENCES chains(id),
  address           BYTEA NOT NULL,
  version           SMALLINT NOT NULL DEFAULT 1, -- supporting contract changes
  abi               JSON, -- optional
  started_at_block  BIGINT, -- optional, needed to select proper contract for the concrete block

  created_at        TIMESTAMP DEFAULT now(),
  updated_at        TIMESTAMP DEFAULT now(),

  UNIQUE(bridge_id, chain_id, address, version)
);

-- tokens: token registry
CREATE TABLE tokens (
  id            BIGSERIAL PRIMARY KEY,
  chain_id      BIGINT NOT NULL REFERENCES chains(id),
  address       BYTEA NOT NULL,
  symbol        TEXT,
  name          TEXT,
  token_icon    TEXT,
  decimals      SMALLINT,

  created_at    TIMESTAMP DEFAULT now(),
  updated_at    TIMESTAMP DEFAULT now(),

  UNIQUE(chain_id, address)
);

-- bridge_txs: bridge txs (containing raw logs). Indexer workers will put data here
--             and may remove rows which are not needed anymore
-- [!] This table has a padding-optimized field order.
CREATE TABLE bridge_txs (
  created_at       TIMESTAMP DEFAULT now(),
  updated_at       TIMESTAMP DEFAULT now(),
  
  id               BIGSERIAL PRIMARY KEY,
  message_id       BIGINT NOT NULL,   -- each bridge transaction should be linked with another one via this field
                                      -- should be extracted by indexer from log or calldata before putting record here
  bridge_id        INTEGER NOT NULL REFERENCES bridges(id),
  contract_id      BIGINT REFERENCES bridge_contracts(id),
  block_number     BIGINT NOT NULL,
  timestamp        TIMESTAMP NOT NULL,
  tx_hash          BYTEA NOT NULL,
  data             BYTEA   -- we will store needed log data here regarding to the concrete indexer logic
);

CREATE TYPE message_status AS ENUM ('initiated', 'completed', 'failed');

-- crosschain_messages: canonical cross-chain messages, constructed by indexer workers during collecting bridge transactions
-- [!] This table has a padding-optimized field order.
CREATE TABLE crosschain_messages (
  created_at            TIMESTAMP DEFAULT now(),
  updated_at            TIMESTAMP DEFAULT now(),
  
  id                    BIGINT NOT NULL,
  bridge_id             INTEGER NOT NULL REFERENCES bridges(id),
  status                message_status NOT NULL DEFAULT 'initiated', -- initiated, completed, failed
  init_timestamp        TIMESTAMP DEFAULT now(), -- in real world (blockchain time), not when indexed
  last_update_timestamp TIMESTAMP DEFAULT now(), -- in real world (blockchain time), not when indexed
  src_chain_id          BIGINT NOT NULL REFERENCES chains(id),
  dst_chain_id          BIGINT NULL REFERENCES chains(id),
  src_tx_hash           BYTEA,  -- can be NULL, because we may not index source chain
  dst_tx_hash           BYTEA,  -- can be NULL, because we may not index destination chain
  sender_address        BYTEA, -- source address (on src chain)
  recipient_address     BYTEA, -- destination address (on dst chain)
  payload               BYTEA, -- raw message payload, bridge-specific fields

  PRIMARY KEY (id, bridge_id)
);

CREATE TYPE transfer_type AS ENUM ('erc20', 'erc721', 'native', 'erc1155');

-- transfers: semantic transfer records extracted from messages (token transfers)
-- [!] This table has a padding-optimized field order.
CREATE TABLE crosschain_transfers (
  created_at          TIMESTAMP DEFAULT now(),
  updated_at          TIMESTAMP DEFAULT now(),
  
  id                  BIGSERIAL PRIMARY KEY,
  message_id          BIGINT NOT NULL, -- the linkage with message is optional
  bridge_id           INTEGER NOT NULL,
  type                transfer_type, -- erc20/erc721/native/erc1155  
  token_src_chain_id  BIGINT NOT NULL REFERENCES chains(id),
  token_dst_chain_id  BIGINT NOT NULL REFERENCES chains(id),
  src_decimals        SMALLINT NOT NULL, -- token decimals (from the any side of interaction)
  dst_decimals        SMALLINT NOT NULL, -- token decimals (from the any side of interaction)
  src_amount          NUMERIC(78,0) NOT NULL, -- store raw integer amount
  dst_amount          NUMERIC(78,0) NOT NULL, -- store raw integer amount
  token_src_address   BYTEA NOT NULL, -- token contract on token_chain_id
  token_dst_address   BYTEA NOT NULL, -- token contract on token_chain_id
  sender_address      BYTEA, -- source address (on src chain)
  recipient_address   BYTEA, -- destination address (on dst chain)
  token_ids           NUMERIC(78,0)[], -- for NFTs

  FOREIGN KEY (message_id, bridge_id)
               REFERENCES crosschain_messages(id, bridge_id)
               ON DELETE CASCADE
);

-- indexer_checkpoints: for progress tracking per chain/worker
CREATE TABLE indexer_checkpoints (
  bridge_id         BIGINT NOT NULL REFERENCES bridges(id),
  chain_id          BIGINT NOT NULL REFERENCES chains(id),
  
  -- sync checkpoints
  catchup_min_block BIGINT NOT NULL,
  catchup_max_block BIGINT NOT NULL,
  finality_cursor   BIGINT NOT NULL,
  realtime_cursor   BIGINT NOT NULL,
  
  created_at        TIMESTAMP DEFAULT now(),
  updated_at        TIMESTAMP DEFAULT now(),

  PRIMARY KEY (bridge_id, chain_id)
);

-- indexer_failures: storing failed intervals for indexer
CREATE TABLE indexer_failures (
  id           BIGSERIAL PRIMARY KEY,
  bridge_id    BIGINT NOT NULL REFERENCES bridges(id),
  chain_id     BIGINT NOT NULL REFERENCES chains(id),
  
  from_block   BIGINT NOT NULL,
  to_block     BIGINT NOT NULL,
  attempts     INTEGER NOT NULL DEFAULT 1,
  reason       TEXT,
  
  created_at   TIMESTAMP DEFAULT now(),
  updated_at   TIMESTAMP DEFAULT now()
);

