CREATE EXTENSION pg_trgm;

CREATE TYPE token_type AS ENUM ('ERC-20', 'ERC-721', 'ERC-1155', 'ERC-404');
CREATE TYPE hash_type AS ENUM ('block', 'transaction');

CREATE TABLE chains (
  id integer PRIMARY KEY,
  explorer_url varchar,
  icon_url varchar,
  created_at timestamp NOT NULL DEFAULT (now()),
  updated_at timestamp NOT NULL DEFAULT (now())
);

CREATE TABLE addresses (
  hash bytea NOT NULL,
  chain_id integer NOT NULL REFERENCES chains (id),
  ens_name text,
  contract_name text,
  token_name text,
  token_type token_type,
  is_contract boolean NOT NULL DEFAULT false,
  is_verified_contract boolean NOT NULL DEFAULT false,
  is_token boolean NOT NULL DEFAULT false,
  created_at timestamp NOT NULL DEFAULT (now()),
  updated_at timestamp NOT NULL DEFAULT (now()),
  PRIMARY KEY (hash, chain_id)
);
CREATE INDEX addresses_contract_name_trgm_idx ON addresses USING GIN (to_tsvector('english', contract_name));
CREATE INDEX addresses_ens_name_trgm_idx ON addresses USING GIN (to_tsvector('english', ens_name));
CREATE INDEX addresses_token_name_trgm_idx ON addresses USING GIN (to_tsvector('english', token_name));

CREATE TABLE block_ranges (
  min_block_number integer NOT NULL,
  max_block_number integer NOT NULL,
  chain_id integer PRIMARY KEY NOT NULL REFERENCES chains (id),
  created_at timestamp NOT NULL DEFAULT (now()),
  updated_at timestamp NOT NULL DEFAULT (now())
);

CREATE TABLE dapps (
  chain_id integer NOT NULL REFERENCES chains (id),
  name varchar NOT NULL,
  description varchar NOT NULL,
  link varchar NOT NULL,
  created_at timestamp NOT NULL DEFAULT (now()),
  updated_at timestamp NOT NULL DEFAULT (now()),
  PRIMARY KEY (chain_id, name)
);

CREATE TABLE hashes (
  hash bytea NOT NULL,
  chain_id integer NOT NULL REFERENCES chains (id),
  hash_type hash_type NOT NULL,
  created_at timestamp NOT NULL DEFAULT (now()),
  PRIMARY KEY (hash, chain_id)
)
