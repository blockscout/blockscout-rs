-- chains: supported chains list (e.g. Ethereum, Gnosis, Avalanche...)
CREATE TABLE chains (
  id            BIGINT PRIMARY KEY, -- the classic EVM chain_id, 8 bytes long
                                  -- or arbitrary unique number if not applicable
  name          TEXT NOT NULL UNIQUE,
  icon          TEXT,
  explorer      TEXT,
  custom_routes JSON, -- explorer custom route templates [only when differs from defaults]:
                      -- { tx: '/tx/{hash}', address: '/address/{hash}', token: '/token/{hash}' }

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
  docs_url    TEXT,
  
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
  chain_id      BIGINT NOT NULL REFERENCES chains(id),
  address       BYTEA NOT NULL,
  symbol        TEXT,
  name          TEXT,
  token_icon    TEXT,
  decimals      SMALLINT,

  created_at    TIMESTAMP DEFAULT now(),
  updated_at    TIMESTAMP DEFAULT now(),

  PRIMARY KEY (chain_id, address)
);

-- indexer_staging: Indexer Persistent Storage.
-- It's a internal table which can be used by any indexer to store
-- arbitrary data which should be restored in case of service failure
-- or regular rebooting. So it's some kind of in-memory cache persistent snapshot.
-- The content and structure of the item is totally on indexer side.
-- This table just provides a typical fields which could be useful in typycal cases
-- NOTE: Do not use this table intensively since it can affect the database performance
CREATE TABLE indexer_staging (
  created_at       TIMESTAMP DEFAULT now(),
  updated_at       TIMESTAMP DEFAULT now(),
  
  id               BIGINT PRIMARY KEY,
  bridge_id        INTEGER NOT NULL REFERENCES bridges(id),
  --message_id       BIGINT NOT NULL,   -- each bridge transaction should be linked with another one via this field
                                      -- should be extracted by indexer from log or calldata before putting record here
  contract_id      BIGINT REFERENCES bridge_contracts(id), -- optional, pointing to contract where associated item was collected from
  block_number     BIGINT, -- optional, which block contains associated data
  tx_hash          BYTEA,
  data             BYTEA   -- the item
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
  init_timestamp        TIMESTAMP NOT NULL DEFAULT now(), -- when the message appeared in the real world
                                                          -- (blockchain time), not when indexed.
                                                          -- This is a sorting criteria so it SHOULD NOT be changed!
                                                          -- If it's unable to index message originating event,
                                                          -- the indexer should set the fake timestamp here
                                                          -- (e.g. when it finalized on the destination blockchain)
  last_update_timestamp TIMESTAMP DEFAULT now(), -- when the message got his final state in the real world
                                                 -- (blockchain time), not when indexed.
  src_chain_id          BIGINT NOT NULL REFERENCES chains(id),
  dst_chain_id          BIGINT NULL REFERENCES chains(id),
  native_id             BYTEA,  -- optional native ID
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
  message_id          BIGINT NOT NULL,
  bridge_id           INTEGER NOT NULL,
  index               SMALLINT NOT NULL DEFAULT 0, -- index of the transfer in the message (for messages with multiple transfers)
  type                transfer_type, -- erc20/erc721/native/erc1155
  token_src_chain_id  BIGINT NOT NULL REFERENCES chains(id),
  token_dst_chain_id  BIGINT NOT NULL REFERENCES chains(id),
  src_amount          NUMERIC(78,0) NOT NULL, -- store raw integer amount
  dst_amount          NUMERIC(78,0) NOT NULL, -- store raw integer amount
  token_src_address   BYTEA NOT NULL, -- token contract on token_chain_id
  token_dst_address   BYTEA NOT NULL, -- token contract on token_chain_id
  sender_address      BYTEA, -- source address (on src chain)
  recipient_address   BYTEA, -- destination address (on dst chain)
  token_ids           NUMERIC(78,0)[], -- for NFTs

  FOREIGN KEY (message_id, bridge_id)
               REFERENCES crosschain_messages(id, bridge_id)
               ON DELETE CASCADE,

  UNIQUE (bridge_id, message_id, index)
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

-- pending_messages: temporary storage for destination events that arrive before source events
-- This prevents creating crosschain_messages rows with NULL init_timestamp
CREATE TABLE pending_messages (
  message_id   BIGINT NOT NULL,
  bridge_id    INTEGER NOT NULL REFERENCES bridges(id),
  payload      JSONB NOT NULL,  -- full event data from destination
  created_at   TIMESTAMP DEFAULT now(),
  
  PRIMARY KEY (message_id, bridge_id)
);

CREATE INDEX idx_pending_stale ON pending_messages(created_at);

CREATE TABLE avalanche_icm_blockchain_ids (
  -- TODO: create a comment on this field that this is hex encoded 32-byte Avalanche blockchain ID
  blockchain_id      BYTEA PRIMARY KEY,
  chain_id           BIGINT NOT NULL REFERENCES chains(id) ON DELETE CASCADE,
  created_at         TIMESTAMP DEFAULT now(),
  updated_at         TIMESTAMP DEFAULT now(),

  UNIQUE(chain_id)
);
