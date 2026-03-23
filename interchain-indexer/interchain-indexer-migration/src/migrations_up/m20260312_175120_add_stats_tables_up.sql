-- stats_assets: one logical cross-chain asset for analytics
CREATE TABLE stats_assets (
  id            BIGSERIAL PRIMARY KEY,
  name          TEXT,
  symbol        TEXT,
  icon_url      TEXT,
  created_at    TIMESTAMP NOT NULL DEFAULT now(),
  updated_at    TIMESTAMP NOT NULL DEFAULT now()
);

-- stats_asset_tokens: map chain-local token contract to stats asset (no FK to tokens)
CREATE TABLE stats_asset_tokens (
  stats_asset_id      BIGINT NOT NULL
    REFERENCES stats_assets(id)
    ON DELETE CASCADE,

  chain_id            BIGINT NOT NULL
    REFERENCES chains(id)
    ON DELETE CASCADE,

  token_address       BYTEA NOT NULL,

  created_at          TIMESTAMP NOT NULL DEFAULT now(),
  updated_at          TIMESTAMP NOT NULL DEFAULT now(),

  PRIMARY KEY (stats_asset_id, chain_id),

  CONSTRAINT stats_asset_tokens_unique_token
    UNIQUE (chain_id, token_address)
);

CREATE TYPE edge_amount_side AS ENUM ('source', 'destination');

-- stats_asset_edges: aggregated movement counters per asset per (src_chain, dst_chain)
CREATE TABLE stats_asset_edges (
  stats_asset_id      BIGINT NOT NULL
    REFERENCES stats_assets(id)
    ON DELETE CASCADE,

  src_chain_id        BIGINT NOT NULL
    REFERENCES chains(id)
    ON DELETE CASCADE,

  dst_chain_id        BIGINT NOT NULL
    REFERENCES chains(id)
    ON DELETE CASCADE,

  transfers_count     BIGINT NOT NULL DEFAULT 0,
  cumulative_amount   NUMERIC(78,0) NOT NULL DEFAULT 0,
  decimals            SMALLINT,
  amount_side         edge_amount_side NOT NULL, -- which side's amounts are summed into cumulative_amount; immutable after edge creation

  created_at          TIMESTAMP NOT NULL DEFAULT now(),
  updated_at          TIMESTAMP NOT NULL DEFAULT now(),

  PRIMARY KEY (stats_asset_id, src_chain_id, dst_chain_id)
);

CREATE INDEX stats_asset_edges_src_chain_idx
  ON stats_asset_edges (src_chain_id);

CREATE INDEX stats_asset_edges_dst_chain_idx
  ON stats_asset_edges (dst_chain_id);


-- crosschain_messages.stats_processed: counters for incremental stats processing
-- it reflects how many times the row was processed for stats calculations
-- (to support future stats re-processing)
ALTER TABLE crosschain_messages
  ADD COLUMN stats_processed SMALLINT NOT NULL DEFAULT 0;

-- crosschain_transfers: stats processing counter and optional link to stats asset
ALTER TABLE crosschain_transfers
ADD COLUMN stats_processed SMALLINT NOT NULL DEFAULT 0,
ADD COLUMN stats_asset_id BIGINT
  REFERENCES stats_assets(id)
  ON DELETE SET NULL;

CREATE INDEX crosschain_transfers_stats_asset_idx
  ON crosschain_transfers (stats_asset_id);

-- stats_chains: periodically refreshed per-chain user counters
CREATE TABLE stats_chains (
  chain_id                     BIGINT PRIMARY KEY
    REFERENCES chains(id)
    ON DELETE CASCADE,

  unique_transfer_users_count   BIGINT NOT NULL DEFAULT 0,
  unique_message_users_count   BIGINT NOT NULL DEFAULT 0,

  created_at                   TIMESTAMP NOT NULL DEFAULT now(),
  updated_at                   TIMESTAMP NOT NULL DEFAULT now()
);

CREATE INDEX crosschain_messages_src_user_by_chain_idx
  ON crosschain_messages (src_chain_id, sender_address)
  WHERE sender_address IS NOT NULL;

CREATE INDEX crosschain_messages_dst_user_by_chain_idx
  ON crosschain_messages (dst_chain_id, recipient_address)
  WHERE dst_chain_id IS NOT NULL AND recipient_address IS NOT NULL;

CREATE INDEX crosschain_transfers_src_user_by_chain_idx
  ON crosschain_transfers (token_src_chain_id, sender_address)
  WHERE sender_address IS NOT NULL;

CREATE INDEX crosschain_transfers_dst_user_by_chain_idx
  ON crosschain_transfers (token_dst_chain_id, recipient_address)
  WHERE recipient_address IS NOT NULL;

-- stats_messages: directional chain-to-chain message counts for diagrams (sent/received paths)
CREATE TABLE stats_messages (
  src_chain_id   BIGINT NOT NULL
    REFERENCES chains(id)
    ON DELETE CASCADE,

  dst_chain_id   BIGINT NOT NULL
    REFERENCES chains(id)
    ON DELETE CASCADE,

  messages_count BIGINT NOT NULL DEFAULT 0,

  created_at     TIMESTAMP NOT NULL DEFAULT now(),
  updated_at     TIMESTAMP NOT NULL DEFAULT now(),

  PRIMARY KEY (src_chain_id, dst_chain_id)
);

CREATE INDEX stats_messages_src_chain_idx
  ON stats_messages (src_chain_id);

CREATE INDEX stats_messages_dst_chain_idx
  ON stats_messages (dst_chain_id);

-- stats_messages_days: daily directional chain-to-chain message counts for bounded path stats
CREATE TABLE stats_messages_days (
  date           DATE NOT NULL,

  src_chain_id   BIGINT NOT NULL
    REFERENCES chains(id)
    ON DELETE CASCADE,

  dst_chain_id   BIGINT NOT NULL
    REFERENCES chains(id)
    ON DELETE CASCADE,

  messages_count BIGINT NOT NULL DEFAULT 0,

  created_at     TIMESTAMP NOT NULL DEFAULT now(),
  updated_at     TIMESTAMP NOT NULL DEFAULT now(),

  PRIMARY KEY (date, src_chain_id, dst_chain_id)
);

CREATE INDEX stats_messages_days_date_idx
  ON stats_messages_days (date);

CREATE INDEX stats_messages_days_src_chain_date_idx
  ON stats_messages_days (src_chain_id, date);

CREATE INDEX stats_messages_days_dst_chain_date_idx
  ON stats_messages_days (dst_chain_id, date);
