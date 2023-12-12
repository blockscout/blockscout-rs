CREATE TABLE public.blocks
(
    consensus        boolean                     NOT NULL,
    difficulty       numeric(50, 0),
    gas_limit        numeric(100, 0)             NOT NULL,
    gas_used         numeric(100, 0)             NOT NULL,
    hash             bytea                       NOT NULL,
    miner_hash       bytea                       NOT NULL,
    nonce            bytea                       NOT NULL,
    number           bigint                      NOT NULL,
    parent_hash      bytea                       NOT NULL,
    size             integer,
    "timestamp"      timestamp without time zone NOT NULL,
    total_difficulty numeric(50, 0),
    inserted_at      timestamp without time zone NOT NULL,
    updated_at       timestamp without time zone NOT NULL,
    refetch_needed   boolean DEFAULT false,
    base_fee_per_gas numeric(100, 0),
    is_empty         boolean
);

CREATE TABLE public.logs
(
    data             bytea                       NOT NULL,
    index            integer                     NOT NULL,
    type             character varying(255),
    first_topic      character varying(255),
    second_topic     character varying(255),
    third_topic      character varying(255),
    fourth_topic     character varying(255),
    inserted_at      timestamp without time zone NOT NULL,
    updated_at       timestamp without time zone NOT NULL,
    address_hash     bytea,
    transaction_hash bytea                       NOT NULL,
    block_hash       bytea                       NOT NULL,
    block_number     integer
);
