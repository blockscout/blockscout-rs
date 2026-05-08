--
-- PostgreSQL database dump
--

-- Dumped from database version 17.2 (Ubuntu 17.2-1.pgdg22.04+1)
-- Dumped by pg_dump version 17.9

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET transaction_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: public; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA public;


--
-- Name: SCHEMA public; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON SCHEMA public IS 'standard public schema';


--
-- Name: beacon_deposits_status; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.beacon_deposits_status AS ENUM (
    'invalid',
    'pending',
    'completed'
);


--
-- Name: entry_point_version; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.entry_point_version AS ENUM (
    'v0.6',
    'v0.7',
    'v0.8'
);


--
-- Name: internal_transactions_call_type; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.internal_transactions_call_type AS ENUM (
    'call',
    'callcode',
    'delegatecall',
    'staticcall',
    'invalid'
);


--
-- Name: metadata_tag_record; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.metadata_tag_record AS (
	id integer,
	address_hash bytea,
	metadata jsonb,
	addresses_index integer
);


--
-- Name: multichain_search_counter_type; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.multichain_search_counter_type AS ENUM (
    'global'
);


--
-- Name: multichain_search_hash_type; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.multichain_search_hash_type AS ENUM (
    'block',
    'transaction',
    'address'
);


--
-- Name: multichain_search_token_data_type; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.multichain_search_token_data_type AS ENUM (
    'metadata',
    'total_supply',
    'counters',
    'market_data'
);


--
-- Name: oban_job_state; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.oban_job_state AS ENUM (
    'available',
    'scheduled',
    'executing',
    'retryable',
    'completed',
    'discarded',
    'cancelled'
);


--
-- Name: proxy_type; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.proxy_type AS ENUM (
    'eip1167',
    'eip1967',
    'eip1822',
    'eip1967_oz',
    'master_copy',
    'basic_implementation',
    'basic_get_implementation',
    'comptroller',
    'eip2535',
    'clone_with_immutable_arguments',
    'eip7702',
    'resolved_delegate_proxy',
    'erc7760',
    'eip1967_beacon'
);


--
-- Name: signed_authorization_status; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.signed_authorization_status AS ENUM (
    'ok',
    'invalid_chain_id',
    'invalid_signature',
    'invalid_nonce'
);


--
-- Name: sponsor_type; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.sponsor_type AS ENUM (
    'wallet_deposit',
    'wallet_balance',
    'paymaster_sponsor',
    'paymaster_hybrid'
);


SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: address_coin_balances; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.address_coin_balances (
    address_hash bytea NOT NULL,
    block_number bigint NOT NULL,
    value numeric(100,0) DEFAULT NULL::numeric,
    value_fetched_at timestamp without time zone,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: address_coin_balances_daily; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.address_coin_balances_daily (
    address_hash bytea NOT NULL,
    day date NOT NULL,
    value numeric(100,0) DEFAULT NULL::numeric,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: address_contract_code_fetch_attempts; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.address_contract_code_fetch_attempts (
    address_hash bytea NOT NULL,
    retries_number smallint,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: address_current_token_balances; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.address_current_token_balances (
    id bigint NOT NULL,
    address_hash bytea NOT NULL,
    block_number bigint NOT NULL,
    token_contract_address_hash bytea NOT NULL,
    value numeric,
    value_fetched_at timestamp without time zone,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    old_value numeric,
    token_id numeric(78,0),
    token_type character varying(255),
    refetch_after timestamp without time zone,
    retries_count smallint
);


--
-- Name: address_current_token_balances_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.address_current_token_balances_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: address_current_token_balances_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.address_current_token_balances_id_seq OWNED BY public.address_current_token_balances.id;


--
-- Name: address_ids_to_address_hashes; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.address_ids_to_address_hashes (
    address_id bigint NOT NULL,
    address_hash bytea NOT NULL
);


--
-- Name: address_ids_to_address_hashes_address_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.address_ids_to_address_hashes_address_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: address_ids_to_address_hashes_address_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.address_ids_to_address_hashes_address_id_seq OWNED BY public.address_ids_to_address_hashes.address_id;


--
-- Name: address_names; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.address_names (
    address_hash bytea NOT NULL,
    name character varying(255) NOT NULL,
    "primary" boolean DEFAULT false NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    metadata jsonb
);


--
-- Name: address_tags; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.address_tags (
    id integer NOT NULL,
    label character varying(255) NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    display_name character varying(255)
);


--
-- Name: address_tags_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.address_tags_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: address_tags_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.address_tags_id_seq OWNED BY public.address_tags.id;


--
-- Name: address_to_tags; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.address_to_tags (
    id bigint NOT NULL,
    address_hash bytea NOT NULL,
    tag_id integer NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: address_to_tags_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.address_to_tags_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: address_to_tags_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.address_to_tags_id_seq OWNED BY public.address_to_tags.id;


--
-- Name: address_token_balances; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.address_token_balances (
    id bigint NOT NULL,
    address_hash bytea NOT NULL,
    block_number bigint NOT NULL,
    token_contract_address_hash bytea NOT NULL,
    value numeric,
    value_fetched_at timestamp without time zone,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    token_id numeric(78,0),
    token_type character varying(255),
    refetch_after timestamp without time zone,
    retries_count smallint
);


--
-- Name: address_token_balances_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.address_token_balances_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: address_token_balances_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.address_token_balances_id_seq OWNED BY public.address_token_balances.id;


--
-- Name: addresses; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.addresses (
    fetched_coin_balance numeric(100,0),
    fetched_coin_balance_block_number bigint,
    hash bytea NOT NULL,
    contract_code bytea,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    nonce integer,
    decompiled boolean,
    verified boolean,
    gas_used bigint,
    transactions_count integer,
    token_transfers_count integer
);


--
-- Name: administrators; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.administrators (
    id bigint NOT NULL,
    role character varying(255) NOT NULL,
    user_id bigint NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: administrators_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.administrators_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: administrators_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.administrators_id_seq OWNED BY public.administrators.id;


--
-- Name: beacon_blobs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.beacon_blobs (
    hash bytea NOT NULL,
    blob_data bytea,
    kzg_commitment bytea,
    kzg_proof bytea,
    inserted_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: beacon_blobs_transactions; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.beacon_blobs_transactions (
    hash bytea NOT NULL,
    max_fee_per_blob_gas numeric(100,0) NOT NULL,
    blob_gas_price numeric(100,0) NOT NULL,
    blob_gas_used numeric(100,0) NOT NULL,
    blob_versioned_hashes bytea[] NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: beacon_deposits; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.beacon_deposits (
    pubkey bytea NOT NULL,
    withdrawal_credentials bytea NOT NULL,
    amount numeric(100,0) NOT NULL,
    signature bytea NOT NULL,
    index bigint NOT NULL,
    block_number bigint NOT NULL,
    block_timestamp timestamp without time zone NOT NULL,
    log_index integer NOT NULL,
    status public.beacon_deposits_status NOT NULL,
    from_address_hash bytea NOT NULL,
    block_hash bytea NOT NULL,
    transaction_hash bytea NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: block_rewards; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.block_rewards (
    address_hash bytea NOT NULL,
    address_type character varying(255) NOT NULL,
    block_hash bytea NOT NULL,
    reward numeric(100,0),
    inserted_at timestamp(0) without time zone NOT NULL,
    updated_at timestamp(0) without time zone NOT NULL
);


--
-- Name: block_second_degree_relations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.block_second_degree_relations (
    nephew_hash bytea NOT NULL,
    uncle_hash bytea NOT NULL,
    uncle_fetched_at timestamp without time zone,
    index integer
);


--
-- Name: blocks; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.blocks (
    consensus boolean NOT NULL,
    difficulty numeric(50,0),
    gas_limit numeric(100,0) NOT NULL,
    gas_used numeric(100,0) NOT NULL,
    hash bytea NOT NULL,
    miner_hash bytea NOT NULL,
    nonce bytea NOT NULL,
    number bigint NOT NULL,
    parent_hash bytea NOT NULL,
    size integer,
    "timestamp" timestamp without time zone NOT NULL,
    total_difficulty numeric(50,0),
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    refetch_needed boolean DEFAULT false,
    base_fee_per_gas numeric(100,0),
    is_empty boolean,
    blob_gas_used numeric(100,0),
    excess_blob_gas numeric(100,0)
);


--
-- Name: bridged_tokens; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.bridged_tokens (
    foreign_chain_id numeric NOT NULL,
    foreign_token_contract_address_hash bytea NOT NULL,
    exchange_rate numeric,
    custom_metadata character varying(255),
    lp_token boolean,
    custom_cap numeric,
    type character varying(255),
    home_token_contract_address_hash bytea NOT NULL,
    inserted_at timestamp(0) without time zone NOT NULL,
    updated_at timestamp(0) without time zone NOT NULL
);


--
-- Name: constants; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.constants (
    key character varying(255) NOT NULL,
    value character varying(255),
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: contract_methods; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.contract_methods (
    id bigint NOT NULL,
    identifier integer NOT NULL,
    abi jsonb NOT NULL,
    type character varying(255) NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: contract_methods_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.contract_methods_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: contract_methods_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.contract_methods_id_seq OWNED BY public.contract_methods.id;


--
-- Name: csv_export_requests; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.csv_export_requests (
    id uuid NOT NULL,
    remote_ip_hash bytea NOT NULL,
    file_id character varying(255),
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    status character varying(255) DEFAULT 'pending'::character varying NOT NULL,
    expires_at timestamp(0) without time zone
);


--
-- Name: deleted_internal_transactions_address_placeholders; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.deleted_internal_transactions_address_placeholders (
    address_id bigint NOT NULL,
    block_number bigint NOT NULL,
    count_tos smallint NOT NULL,
    count_froms smallint NOT NULL
);


--
-- Name: emission_rewards; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.emission_rewards (
    block_range int8range NOT NULL,
    reward numeric
);


--
-- Name: event_notifications; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.event_notifications (
    id bigint NOT NULL,
    data text NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: event_notifications_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.event_notifications_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: event_notifications_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.event_notifications_id_seq OWNED BY public.event_notifications.id;


--
-- Name: fhe_operations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.fhe_operations (
    transaction_hash bytea NOT NULL,
    log_index integer NOT NULL,
    block_hash bytea NOT NULL,
    block_number bigint NOT NULL,
    operation character varying(50) NOT NULL,
    operation_type character varying(20) NOT NULL,
    fhe_type character varying(10) NOT NULL,
    is_scalar boolean NOT NULL,
    hcu_cost integer NOT NULL,
    hcu_depth integer NOT NULL,
    caller bytea,
    result_handle bytea NOT NULL,
    input_handles jsonb,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: hot_smart_contracts_daily; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.hot_smart_contracts_daily (
    date date NOT NULL,
    contract_address_hash bytea NOT NULL,
    transactions_count integer NOT NULL,
    total_gas_used numeric(100,0) NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: internal_transactions; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.internal_transactions (
    call_type character varying(255),
    created_contract_code bytea,
    gas numeric(100,0),
    gas_used numeric(100,0),
    index integer NOT NULL,
    init bytea,
    input bytea,
    output bytea,
    trace_address integer[],
    type character varying(255) NOT NULL,
    value numeric(100,0),
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    created_contract_address_hash bytea,
    from_address_hash bytea,
    to_address_hash bytea,
    block_number integer NOT NULL,
    transaction_index integer NOT NULL,
    call_type_enum public.internal_transactions_call_type,
    error_id smallint,
    from_address_id bigint,
    to_address_id bigint,
    created_contract_address_id bigint
);


--
-- Name: internal_transactions_delete_queue; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.internal_transactions_delete_queue (
    block_number bigint NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: last_fetched_counters; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.last_fetched_counters (
    counter_type character varying(255) NOT NULL,
    value numeric(100,0),
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: logs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.logs (
    data bytea NOT NULL,
    index integer NOT NULL,
    first_topic bytea,
    second_topic bytea,
    third_topic bytea,
    fourth_topic bytea,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    address_hash bytea,
    transaction_hash bytea NOT NULL,
    block_hash bytea NOT NULL,
    block_number integer
);


--
-- Name: market_history; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.market_history (
    id bigint NOT NULL,
    date date NOT NULL,
    closing_price numeric,
    opening_price numeric,
    market_cap numeric,
    tvl numeric,
    secondary_coin boolean DEFAULT false
);


--
-- Name: market_history_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.market_history_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: market_history_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.market_history_id_seq OWNED BY public.market_history.id;


--
-- Name: massive_blocks; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.massive_blocks (
    number bigint NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: migrations_status; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.migrations_status (
    migration_name character varying(255) NOT NULL,
    status character varying(255),
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    meta jsonb
);


--
-- Name: missing_balance_of_tokens; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.missing_balance_of_tokens (
    token_contract_address_hash bytea NOT NULL,
    block_number bigint,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    currently_implemented boolean
);


--
-- Name: missing_block_ranges; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.missing_block_ranges (
    id bigint NOT NULL,
    from_number integer,
    to_number integer,
    priority smallint
);


--
-- Name: missing_block_ranges_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.missing_block_ranges_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: missing_block_ranges_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.missing_block_ranges_id_seq OWNED BY public.missing_block_ranges.id;


--
-- Name: multichain_search_db_export_balances_queue; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.multichain_search_db_export_balances_queue (
    id integer NOT NULL,
    address_hash bytea NOT NULL,
    token_contract_address_hash_or_native bytea NOT NULL,
    value numeric(100,0),
    token_id numeric(78,0),
    retries_number smallint,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: multichain_search_db_export_balances_queue_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.multichain_search_db_export_balances_queue_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: multichain_search_db_export_balances_queue_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.multichain_search_db_export_balances_queue_id_seq OWNED BY public.multichain_search_db_export_balances_queue.id;


--
-- Name: multichain_search_db_export_counters_queue; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.multichain_search_db_export_counters_queue (
    "timestamp" timestamp without time zone NOT NULL,
    counter_type public.multichain_search_counter_type NOT NULL,
    data jsonb NOT NULL,
    retries_number smallint,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: multichain_search_db_export_token_info_queue; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.multichain_search_db_export_token_info_queue (
    address_hash bytea NOT NULL,
    data_type public.multichain_search_token_data_type NOT NULL,
    data jsonb NOT NULL,
    retries_number smallint,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: multichain_search_db_main_export_queue; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.multichain_search_db_main_export_queue (
    hash bytea NOT NULL,
    hash_type public.multichain_search_hash_type NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    block_range int8range,
    retries_number smallint
);


--
-- Name: oban_jobs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.oban_jobs (
    id bigint NOT NULL,
    state public.oban_job_state DEFAULT 'available'::public.oban_job_state NOT NULL,
    queue text DEFAULT 'default'::text NOT NULL,
    worker text NOT NULL,
    args jsonb DEFAULT '{}'::jsonb NOT NULL,
    errors jsonb[] DEFAULT ARRAY[]::jsonb[] NOT NULL,
    attempt integer DEFAULT 0 NOT NULL,
    max_attempts integer DEFAULT 20 NOT NULL,
    inserted_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    scheduled_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    attempted_at timestamp without time zone,
    completed_at timestamp without time zone,
    attempted_by text[],
    discarded_at timestamp without time zone,
    priority integer DEFAULT 0 NOT NULL,
    tags text[] DEFAULT ARRAY[]::text[],
    meta jsonb DEFAULT '{}'::jsonb,
    cancelled_at timestamp without time zone,
    CONSTRAINT attempt_range CHECK (((attempt >= 0) AND (attempt <= max_attempts))),
    CONSTRAINT positive_max_attempts CHECK ((max_attempts > 0)),
    CONSTRAINT queue_length CHECK (((char_length(queue) > 0) AND (char_length(queue) < 128))),
    CONSTRAINT worker_length CHECK (((char_length(worker) > 0) AND (char_length(worker) < 128)))
);


--
-- Name: TABLE oban_jobs; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.oban_jobs IS '13';


--
-- Name: oban_jobs_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.oban_jobs_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: oban_jobs_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.oban_jobs_id_seq OWNED BY public.oban_jobs.id;


--
-- Name: oban_peers; Type: TABLE; Schema: public; Owner: -
--

CREATE UNLOGGED TABLE public.oban_peers (
    name text NOT NULL,
    node text NOT NULL,
    started_at timestamp without time zone NOT NULL,
    expires_at timestamp without time zone NOT NULL
);


--
-- Name: pending_block_operations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.pending_block_operations (
    block_hash bytea NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    block_number integer
);


--
-- Name: pending_transaction_operations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.pending_transaction_operations (
    transaction_hash bytea NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: proxy_implementations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.proxy_implementations (
    proxy_address_hash bytea NOT NULL,
    address_hashes bytea[] NOT NULL,
    names character varying(255)[] NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    proxy_type public.proxy_type,
    conflicting_proxy_types public.proxy_type[],
    conflicting_address_hashes bytea[]
);


--
-- Name: proxy_smart_contract_verification_statuses; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.proxy_smart_contract_verification_statuses (
    uid character varying(64) NOT NULL,
    status smallint NOT NULL,
    contract_address_hash bytea,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: scam_address_badge_mappings; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.scam_address_badge_mappings (
    address_hash bytea NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: schema_migrations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.schema_migrations (
    version bigint NOT NULL,
    inserted_at timestamp(0) without time zone
);


--
-- Name: signed_authorizations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.signed_authorizations (
    transaction_hash bytea NOT NULL,
    index integer NOT NULL,
    chain_id numeric(78,0) NOT NULL,
    address bytea NOT NULL,
    nonce numeric(20,0) NOT NULL,
    v integer NOT NULL,
    r numeric(100,0) NOT NULL,
    s numeric(100,0) NOT NULL,
    authority bytea,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    status public.signed_authorization_status
);


--
-- Name: smart_contract_audit_reports; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.smart_contract_audit_reports (
    id bigint NOT NULL,
    address_hash bytea NOT NULL,
    is_approved boolean DEFAULT false,
    submitter_name character varying(255) NOT NULL,
    submitter_email character varying(255) NOT NULL,
    is_project_owner boolean DEFAULT false,
    project_name character varying(255) NOT NULL,
    project_url character varying(255) NOT NULL,
    audit_company_name character varying(255) NOT NULL,
    audit_report_url character varying(255) NOT NULL,
    audit_publish_date date NOT NULL,
    request_id character varying(255),
    comment text,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: smart_contract_audit_reports_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.smart_contract_audit_reports_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: smart_contract_audit_reports_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.smart_contract_audit_reports_id_seq OWNED BY public.smart_contract_audit_reports.id;


--
-- Name: smart_contract_verification_statuses; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.smart_contract_verification_statuses (
    uid character varying(64) NOT NULL,
    status smallint NOT NULL,
    contract_address_hash bytea NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: smart_contracts; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.smart_contracts (
    id bigint NOT NULL,
    name character varying(255) NOT NULL,
    compiler_version character varying(255) NOT NULL,
    optimization boolean NOT NULL,
    contract_source_code text NOT NULL,
    abi jsonb,
    address_hash bytea NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    constructor_arguments text,
    optimization_runs bigint,
    evm_version character varying(255),
    external_libraries jsonb[] DEFAULT ARRAY[]::jsonb[],
    verified_via_sourcify boolean,
    partially_verified boolean,
    file_path text,
    is_changed_bytecode boolean DEFAULT false,
    bytecode_checked_at timestamp without time zone DEFAULT ((now() AT TIME ZONE 'utc'::text) - '1 day'::interval),
    contract_code_md5 character varying(255) NOT NULL,
    compiler_settings jsonb,
    verified_via_eth_bytecode_db boolean,
    license_type smallint DEFAULT 1 NOT NULL,
    verified_via_verifier_alliance boolean,
    certified boolean,
    is_blueprint boolean,
    language smallint
);


--
-- Name: smart_contracts_additional_sources; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.smart_contracts_additional_sources (
    id bigint NOT NULL,
    file_name character varying(255) NOT NULL,
    contract_source_code text NOT NULL,
    address_hash bytea NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: smart_contracts_additional_sources_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.smart_contracts_additional_sources_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: smart_contracts_additional_sources_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.smart_contracts_additional_sources_id_seq OWNED BY public.smart_contracts_additional_sources.id;


--
-- Name: smart_contracts_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.smart_contracts_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: smart_contracts_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.smart_contracts_id_seq OWNED BY public.smart_contracts.id;


--
-- Name: token_instance_metadata_refetch_attempts; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.token_instance_metadata_refetch_attempts (
    token_contract_address_hash bytea NOT NULL,
    token_id numeric(78,0) NOT NULL,
    retries_number smallint,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: token_instances; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.token_instances (
    token_id numeric(78,0) NOT NULL,
    token_contract_address_hash bytea NOT NULL,
    metadata jsonb,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    error character varying(255),
    owner_address_hash bytea,
    owner_updated_at_block bigint,
    owner_updated_at_log_index integer,
    refetch_after timestamp without time zone,
    retries_count smallint DEFAULT 0 NOT NULL,
    thumbnails jsonb,
    media_type character varying(255),
    cdn_upload_error character varying(255),
    is_banned boolean DEFAULT false,
    metadata_url character varying(2048),
    skip_metadata_url boolean
);


--
-- Name: token_transfer_token_id_migrator_progress; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.token_transfer_token_id_migrator_progress (
    id bigint NOT NULL,
    last_processed_block_number integer,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: token_transfer_token_id_migrator_progress_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.token_transfer_token_id_migrator_progress_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: token_transfer_token_id_migrator_progress_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.token_transfer_token_id_migrator_progress_id_seq OWNED BY public.token_transfer_token_id_migrator_progress.id;


--
-- Name: token_transfers; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.token_transfers (
    transaction_hash bytea NOT NULL,
    log_index integer NOT NULL,
    from_address_hash bytea NOT NULL,
    to_address_hash bytea NOT NULL,
    amount numeric,
    token_contract_address_hash bytea NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    block_number integer,
    block_hash bytea NOT NULL,
    amounts numeric[],
    token_ids numeric(78,0)[],
    token_type character varying(255),
    block_consensus boolean DEFAULT true
);


--
-- Name: tokens; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.tokens (
    name text,
    symbol text,
    total_supply numeric,
    decimals numeric,
    type character varying(255) NOT NULL,
    cataloged boolean DEFAULT false,
    contract_address_hash bytea NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    holder_count integer,
    skip_metadata boolean,
    fiat_value numeric,
    circulating_market_cap numeric,
    total_supply_updated_at_block bigint,
    icon_url text,
    is_verified_via_admin_panel boolean DEFAULT false,
    bridged boolean,
    volume_24h numeric,
    metadata_updated_at timestamp without time zone,
    transfer_count integer
);


--
-- Name: transaction_errors; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.transaction_errors (
    id smallint NOT NULL,
    message character varying(255) NOT NULL,
    inserted_at timestamp without time zone NOT NULL
);


--
-- Name: transaction_errors_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.transaction_errors_id_seq
    AS smallint
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: transaction_errors_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.transaction_errors_id_seq OWNED BY public.transaction_errors.id;


--
-- Name: transaction_forks; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.transaction_forks (
    hash bytea NOT NULL,
    index integer NOT NULL,
    uncle_hash bytea NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: transaction_stats; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.transaction_stats (
    id bigint NOT NULL,
    date date,
    number_of_transactions integer,
    gas_used numeric(100,0),
    total_fee numeric(100,0)
);


--
-- Name: transaction_stats_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.transaction_stats_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: transaction_stats_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.transaction_stats_id_seq OWNED BY public.transaction_stats.id;


--
-- Name: transactions; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.transactions (
    cumulative_gas_used numeric(100,0),
    error character varying(255),
    gas numeric(100,0) NOT NULL,
    gas_price numeric(100,0),
    gas_used numeric(100,0),
    hash bytea NOT NULL,
    index integer,
    input bytea NOT NULL,
    nonce integer NOT NULL,
    r numeric(100,0) NOT NULL,
    s numeric(100,0) NOT NULL,
    status integer,
    v numeric(100,0) NOT NULL,
    value numeric(100,0) NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    block_hash bytea,
    block_number integer,
    from_address_hash bytea NOT NULL,
    to_address_hash bytea,
    created_contract_address_hash bytea,
    created_contract_code_indexed_at timestamp without time zone,
    earliest_processing_start timestamp without time zone,
    old_block_hash bytea,
    revert_reason text,
    max_priority_fee_per_gas numeric(100,0),
    max_fee_per_gas numeric(100,0),
    type integer,
    has_error_in_internal_transactions boolean,
    block_consensus boolean DEFAULT true,
    block_timestamp timestamp without time zone,
    has_token_transfers boolean,
    fhe_operations_count integer DEFAULT 0 NOT NULL,
    CONSTRAINT collated_block_number CHECK (((block_hash IS NULL) OR (block_number IS NOT NULL))),
    CONSTRAINT collated_cumalative_gas_used CHECK (((block_hash IS NULL) OR (cumulative_gas_used IS NOT NULL))),
    CONSTRAINT collated_gas_price CHECK (((block_hash IS NULL) OR (gas_price IS NOT NULL))),
    CONSTRAINT collated_gas_used CHECK (((block_hash IS NULL) OR (gas_used IS NOT NULL))),
    CONSTRAINT collated_index CHECK (((block_hash IS NULL) OR (index IS NOT NULL))),
    CONSTRAINT error CHECK (((status = 0) OR ((status <> 0) AND (error IS NULL)))),
    CONSTRAINT pending_block_number CHECK (((block_hash IS NOT NULL) OR (block_number IS NULL))),
    CONSTRAINT pending_cumalative_gas_used CHECK (((block_hash IS NOT NULL) OR (cumulative_gas_used IS NULL))),
    CONSTRAINT pending_gas_used CHECK (((block_hash IS NOT NULL) OR (gas_used IS NULL))),
    CONSTRAINT pending_index CHECK (((block_hash IS NOT NULL) OR (index IS NULL))),
    CONSTRAINT status CHECK ((((block_hash IS NULL) AND (status IS NULL)) OR (block_hash IS NOT NULL) OR ((status = 0) AND ((error)::text = 'dropped/replaced'::text))))
);


--
-- Name: user_contacts; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.user_contacts (
    id bigint NOT NULL,
    email public.citext NOT NULL,
    user_id bigint NOT NULL,
    "primary" boolean DEFAULT false,
    verified boolean DEFAULT false,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: user_contacts_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.user_contacts_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: user_contacts_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.user_contacts_id_seq OWNED BY public.user_contacts.id;


--
-- Name: user_operations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.user_operations (
    hash bytea NOT NULL,
    sender bytea NOT NULL,
    nonce bytea NOT NULL,
    init_code bytea,
    call_data bytea NOT NULL,
    call_gas_limit numeric(100,0) NOT NULL,
    verification_gas_limit numeric(100,0) NOT NULL,
    pre_verification_gas numeric(100,0) NOT NULL,
    max_fee_per_gas numeric(100,0) NOT NULL,
    max_priority_fee_per_gas numeric(100,0) NOT NULL,
    paymaster_and_data bytea,
    signature bytea NOT NULL,
    aggregator bytea,
    aggregator_signature bytea,
    entry_point bytea NOT NULL,
    transaction_hash bytea NOT NULL,
    block_number integer NOT NULL,
    block_hash bytea NOT NULL,
    bundle_index integer NOT NULL,
    index integer NOT NULL,
    user_logs_start_index integer NOT NULL,
    user_logs_count integer NOT NULL,
    bundler bytea NOT NULL,
    factory bytea,
    paymaster bytea,
    status boolean NOT NULL,
    revert_reason bytea,
    gas numeric(100,0) NOT NULL,
    gas_price numeric(100,0) NOT NULL,
    gas_used numeric(100,0) NOT NULL,
    sponsor_type public.sponsor_type NOT NULL,
    inserted_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL,
    entry_point_version public.entry_point_version DEFAULT 'v0.6'::public.entry_point_version NOT NULL
);


--
-- Name: user_ops_indexer_migrations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.user_ops_indexer_migrations (
    version character varying NOT NULL,
    applied_at bigint NOT NULL
);


--
-- Name: users; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.users (
    id bigint NOT NULL,
    username public.citext NOT NULL,
    password_hash character varying(255) NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: users_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.users_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: users_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.users_id_seq OWNED BY public.users.id;


--
-- Name: validators; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.validators (
    address_hash bytea NOT NULL,
    is_validator boolean,
    payout_key_hash bytea,
    info_updated_at_block bigint,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: withdrawals; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.withdrawals (
    index integer NOT NULL,
    validator_index integer NOT NULL,
    amount numeric(100,0) NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    address_hash bytea NOT NULL,
    block_hash bytea NOT NULL
);


--
-- Name: address_current_token_balances id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.address_current_token_balances ALTER COLUMN id SET DEFAULT nextval('public.address_current_token_balances_id_seq'::regclass);


--
-- Name: address_ids_to_address_hashes address_id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.address_ids_to_address_hashes ALTER COLUMN address_id SET DEFAULT nextval('public.address_ids_to_address_hashes_address_id_seq'::regclass);


--
-- Name: address_tags id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.address_tags ALTER COLUMN id SET DEFAULT nextval('public.address_tags_id_seq'::regclass);


--
-- Name: address_to_tags id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.address_to_tags ALTER COLUMN id SET DEFAULT nextval('public.address_to_tags_id_seq'::regclass);


--
-- Name: address_token_balances id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.address_token_balances ALTER COLUMN id SET DEFAULT nextval('public.address_token_balances_id_seq'::regclass);


--
-- Name: administrators id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.administrators ALTER COLUMN id SET DEFAULT nextval('public.administrators_id_seq'::regclass);


--
-- Name: contract_methods id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.contract_methods ALTER COLUMN id SET DEFAULT nextval('public.contract_methods_id_seq'::regclass);


--
-- Name: event_notifications id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.event_notifications ALTER COLUMN id SET DEFAULT nextval('public.event_notifications_id_seq'::regclass);


--
-- Name: market_history id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.market_history ALTER COLUMN id SET DEFAULT nextval('public.market_history_id_seq'::regclass);


--
-- Name: missing_block_ranges id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.missing_block_ranges ALTER COLUMN id SET DEFAULT nextval('public.missing_block_ranges_id_seq'::regclass);


--
-- Name: multichain_search_db_export_balances_queue id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.multichain_search_db_export_balances_queue ALTER COLUMN id SET DEFAULT nextval('public.multichain_search_db_export_balances_queue_id_seq'::regclass);


--
-- Name: oban_jobs id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.oban_jobs ALTER COLUMN id SET DEFAULT nextval('public.oban_jobs_id_seq'::regclass);


--
-- Name: smart_contract_audit_reports id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.smart_contract_audit_reports ALTER COLUMN id SET DEFAULT nextval('public.smart_contract_audit_reports_id_seq'::regclass);


--
-- Name: smart_contracts id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.smart_contracts ALTER COLUMN id SET DEFAULT nextval('public.smart_contracts_id_seq'::regclass);


--
-- Name: smart_contracts_additional_sources id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.smart_contracts_additional_sources ALTER COLUMN id SET DEFAULT nextval('public.smart_contracts_additional_sources_id_seq'::regclass);


--
-- Name: token_transfer_token_id_migrator_progress id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.token_transfer_token_id_migrator_progress ALTER COLUMN id SET DEFAULT nextval('public.token_transfer_token_id_migrator_progress_id_seq'::regclass);


--
-- Name: transaction_errors id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transaction_errors ALTER COLUMN id SET DEFAULT nextval('public.transaction_errors_id_seq'::regclass);


--
-- Name: transaction_stats id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transaction_stats ALTER COLUMN id SET DEFAULT nextval('public.transaction_stats_id_seq'::regclass);


--
-- Name: user_contacts id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_contacts ALTER COLUMN id SET DEFAULT nextval('public.user_contacts_id_seq'::regclass);


--
-- Name: users id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users ALTER COLUMN id SET DEFAULT nextval('public.users_id_seq'::regclass);


--
-- Name: address_coin_balances_daily address_coin_balances_daily_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.address_coin_balances_daily
    ADD CONSTRAINT address_coin_balances_daily_pkey PRIMARY KEY (address_hash, day);


--
-- Name: address_coin_balances address_coin_balances_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.address_coin_balances
    ADD CONSTRAINT address_coin_balances_pkey PRIMARY KEY (address_hash, block_number);


--
-- Name: address_contract_code_fetch_attempts address_contract_code_fetch_attempts_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.address_contract_code_fetch_attempts
    ADD CONSTRAINT address_contract_code_fetch_attempts_pkey PRIMARY KEY (address_hash);


--
-- Name: address_current_token_balances address_current_token_balances_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.address_current_token_balances
    ADD CONSTRAINT address_current_token_balances_pkey PRIMARY KEY (id);


--
-- Name: address_ids_to_address_hashes address_ids_to_address_hashes_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.address_ids_to_address_hashes
    ADD CONSTRAINT address_ids_to_address_hashes_pkey PRIMARY KEY (address_id);


--
-- Name: address_tags address_tags_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.address_tags
    ADD CONSTRAINT address_tags_pkey PRIMARY KEY (label);


--
-- Name: address_to_tags address_to_tags_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.address_to_tags
    ADD CONSTRAINT address_to_tags_pkey PRIMARY KEY (id);


--
-- Name: address_token_balances address_token_balances_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.address_token_balances
    ADD CONSTRAINT address_token_balances_pkey PRIMARY KEY (id);


--
-- Name: addresses addresses_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.addresses
    ADD CONSTRAINT addresses_pkey PRIMARY KEY (hash);


--
-- Name: administrators administrators_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.administrators
    ADD CONSTRAINT administrators_pkey PRIMARY KEY (id);


--
-- Name: beacon_blobs beacon_blobs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.beacon_blobs
    ADD CONSTRAINT beacon_blobs_pkey PRIMARY KEY (hash);


--
-- Name: beacon_blobs_transactions beacon_blobs_transactions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.beacon_blobs_transactions
    ADD CONSTRAINT beacon_blobs_transactions_pkey PRIMARY KEY (hash);


--
-- Name: beacon_deposits beacon_deposits_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.beacon_deposits
    ADD CONSTRAINT beacon_deposits_pkey PRIMARY KEY (index);


--
-- Name: block_rewards block_rewards_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.block_rewards
    ADD CONSTRAINT block_rewards_pkey PRIMARY KEY (address_hash, block_hash, address_type);


--
-- Name: block_second_degree_relations block_second_degree_relations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.block_second_degree_relations
    ADD CONSTRAINT block_second_degree_relations_pkey PRIMARY KEY (nephew_hash, uncle_hash);


--
-- Name: blocks blocks_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.blocks
    ADD CONSTRAINT blocks_pkey PRIMARY KEY (hash);


--
-- Name: internal_transactions call_has_call_type_enum; Type: CHECK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE public.internal_transactions
    ADD CONSTRAINT call_has_call_type_enum CHECK ((((type)::text <> 'call'::text) OR (call_type IS NOT NULL) OR (call_type_enum IS NOT NULL))) NOT VALID;


--
-- Name: constants constants_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.constants
    ADD CONSTRAINT constants_pkey PRIMARY KEY (key);


--
-- Name: contract_methods contract_methods_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.contract_methods
    ADD CONSTRAINT contract_methods_pkey PRIMARY KEY (id);


--
-- Name: internal_transactions create_has_error_id_or_result; Type: CHECK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE public.internal_transactions
    ADD CONSTRAINT create_has_error_id_or_result CHECK ((((type)::text <> 'create'::text) OR ((gas IS NOT NULL) AND (((error_id IS NULL) AND (created_contract_address_id IS NOT NULL) AND (created_contract_code IS NOT NULL) AND (gas_used IS NOT NULL)) OR ((error_id IS NOT NULL) AND (created_contract_address_id IS NULL) AND (created_contract_code IS NULL) AND (gas_used IS NULL)))))) NOT VALID;


--
-- Name: internal_transactions create_has_init; Type: CHECK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE public.internal_transactions
    ADD CONSTRAINT create_has_init CHECK ((((type)::text <> 'create'::text) OR (init IS NOT NULL))) NOT VALID;


--
-- Name: csv_export_requests csv_export_requests_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.csv_export_requests
    ADD CONSTRAINT csv_export_requests_pkey PRIMARY KEY (id);


--
-- Name: deleted_internal_transactions_address_placeholders deleted_internal_transactions_address_placeholders_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.deleted_internal_transactions_address_placeholders
    ADD CONSTRAINT deleted_internal_transactions_address_placeholders_pkey PRIMARY KEY (address_id, block_number);


--
-- Name: emission_rewards emission_rewards_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.emission_rewards
    ADD CONSTRAINT emission_rewards_pkey PRIMARY KEY (block_range);


--
-- Name: event_notifications event_notifications_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.event_notifications
    ADD CONSTRAINT event_notifications_pkey PRIMARY KEY (id);


--
-- Name: fhe_operations fhe_operations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.fhe_operations
    ADD CONSTRAINT fhe_operations_pkey PRIMARY KEY (transaction_hash, log_index);


--
-- Name: hot_smart_contracts_daily hot_smart_contracts_daily_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.hot_smart_contracts_daily
    ADD CONSTRAINT hot_smart_contracts_daily_pkey PRIMARY KEY (date, contract_address_hash);


--
-- Name: internal_transactions internal_transactions_block_number_not_null; Type: CHECK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE public.internal_transactions
    ADD CONSTRAINT internal_transactions_block_number_not_null CHECK ((block_number IS NOT NULL)) NOT VALID;


--
-- Name: internal_transactions_delete_queue internal_transactions_delete_queue_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.internal_transactions_delete_queue
    ADD CONSTRAINT internal_transactions_delete_queue_pkey PRIMARY KEY (block_number);


--
-- Name: internal_transactions internal_transactions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.internal_transactions
    ADD CONSTRAINT internal_transactions_pkey PRIMARY KEY (block_number, transaction_index, index);


--
-- Name: internal_transactions internal_transactions_transaction_index_not_null; Type: CHECK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE public.internal_transactions
    ADD CONSTRAINT internal_transactions_transaction_index_not_null CHECK ((transaction_index IS NOT NULL)) NOT VALID;


--
-- Name: last_fetched_counters last_fetched_counters_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.last_fetched_counters
    ADD CONSTRAINT last_fetched_counters_pkey PRIMARY KEY (counter_type);


--
-- Name: logs logs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.logs
    ADD CONSTRAINT logs_pkey PRIMARY KEY (transaction_hash, block_hash, index);


--
-- Name: market_history market_history_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.market_history
    ADD CONSTRAINT market_history_pkey PRIMARY KEY (id);


--
-- Name: massive_blocks massive_blocks_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.massive_blocks
    ADD CONSTRAINT massive_blocks_pkey PRIMARY KEY (number);


--
-- Name: migrations_status migrations_status_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.migrations_status
    ADD CONSTRAINT migrations_status_pkey PRIMARY KEY (migration_name);


--
-- Name: missing_balance_of_tokens missing_balance_of_tokens_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.missing_balance_of_tokens
    ADD CONSTRAINT missing_balance_of_tokens_pkey PRIMARY KEY (token_contract_address_hash);


--
-- Name: missing_block_ranges missing_block_ranges_no_overlap; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.missing_block_ranges
    ADD CONSTRAINT missing_block_ranges_no_overlap EXCLUDE USING gist (int4range(to_number, from_number, '[]'::text) WITH &&);


--
-- Name: missing_block_ranges missing_block_ranges_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.missing_block_ranges
    ADD CONSTRAINT missing_block_ranges_pkey PRIMARY KEY (id);


--
-- Name: multichain_search_db_export_balances_queue multichain_search_db_export_balances_queue_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.multichain_search_db_export_balances_queue
    ADD CONSTRAINT multichain_search_db_export_balances_queue_pkey PRIMARY KEY (id);


--
-- Name: multichain_search_db_export_counters_queue multichain_search_db_export_counters_queue_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.multichain_search_db_export_counters_queue
    ADD CONSTRAINT multichain_search_db_export_counters_queue_pkey PRIMARY KEY ("timestamp", counter_type);


--
-- Name: multichain_search_db_export_token_info_queue multichain_search_db_export_token_info_queue_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.multichain_search_db_export_token_info_queue
    ADD CONSTRAINT multichain_search_db_export_token_info_queue_pkey PRIMARY KEY (address_hash, data_type);


--
-- Name: multichain_search_db_main_export_queue multichain_search_db_main_export_queue_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.multichain_search_db_main_export_queue
    ADD CONSTRAINT multichain_search_db_main_export_queue_pkey PRIMARY KEY (hash, hash_type);


--
-- Name: emission_rewards no_overlapping_ranges; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.emission_rewards
    ADD CONSTRAINT no_overlapping_ranges EXCLUDE USING gist (block_range WITH &&);


--
-- Name: oban_jobs non_negative_priority; Type: CHECK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE public.oban_jobs
    ADD CONSTRAINT non_negative_priority CHECK ((priority >= 0)) NOT VALID;


--
-- Name: oban_jobs oban_jobs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.oban_jobs
    ADD CONSTRAINT oban_jobs_pkey PRIMARY KEY (id);


--
-- Name: oban_peers oban_peers_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.oban_peers
    ADD CONSTRAINT oban_peers_pkey PRIMARY KEY (name);


--
-- Name: pending_block_operations pending_block_operations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.pending_block_operations
    ADD CONSTRAINT pending_block_operations_pkey PRIMARY KEY (block_hash);


--
-- Name: pending_transaction_operations pending_transaction_operations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.pending_transaction_operations
    ADD CONSTRAINT pending_transaction_operations_pkey PRIMARY KEY (transaction_hash);


--
-- Name: proxy_implementations proxy_implementations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.proxy_implementations
    ADD CONSTRAINT proxy_implementations_pkey PRIMARY KEY (proxy_address_hash);


--
-- Name: proxy_smart_contract_verification_statuses proxy_smart_contract_verification_statuses_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.proxy_smart_contract_verification_statuses
    ADD CONSTRAINT proxy_smart_contract_verification_statuses_pkey PRIMARY KEY (uid);


--
-- Name: scam_address_badge_mappings scam_address_badge_mappings_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.scam_address_badge_mappings
    ADD CONSTRAINT scam_address_badge_mappings_pkey PRIMARY KEY (address_hash);


--
-- Name: schema_migrations schema_migrations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.schema_migrations
    ADD CONSTRAINT schema_migrations_pkey PRIMARY KEY (version);


--
-- Name: internal_transactions selfdestruct_has_from_and_to_address; Type: CHECK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE public.internal_transactions
    ADD CONSTRAINT selfdestruct_has_from_and_to_address CHECK ((((type)::text <> 'selfdestruct'::text) OR ((from_address_id IS NOT NULL) AND (gas IS NULL) AND (to_address_id IS NOT NULL)))) NOT VALID;


--
-- Name: signed_authorizations signed_authorizations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.signed_authorizations
    ADD CONSTRAINT signed_authorizations_pkey PRIMARY KEY (transaction_hash, index);


--
-- Name: smart_contract_audit_reports smart_contract_audit_reports_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.smart_contract_audit_reports
    ADD CONSTRAINT smart_contract_audit_reports_pkey PRIMARY KEY (id);


--
-- Name: smart_contract_verification_statuses smart_contract_verification_statuses_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.smart_contract_verification_statuses
    ADD CONSTRAINT smart_contract_verification_statuses_pkey PRIMARY KEY (uid);


--
-- Name: smart_contracts_additional_sources smart_contracts_additional_sources_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.smart_contracts_additional_sources
    ADD CONSTRAINT smart_contracts_additional_sources_pkey PRIMARY KEY (id);


--
-- Name: smart_contracts smart_contracts_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.smart_contracts
    ADD CONSTRAINT smart_contracts_pkey PRIMARY KEY (id);


--
-- Name: token_instance_metadata_refetch_attempts token_instance_metadata_refetch_attempts_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.token_instance_metadata_refetch_attempts
    ADD CONSTRAINT token_instance_metadata_refetch_attempts_pkey PRIMARY KEY (token_contract_address_hash, token_id);


--
-- Name: token_instances token_instances_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.token_instances
    ADD CONSTRAINT token_instances_pkey PRIMARY KEY (token_id, token_contract_address_hash);


--
-- Name: token_transfer_token_id_migrator_progress token_transfer_token_id_migrator_progress_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.token_transfer_token_id_migrator_progress
    ADD CONSTRAINT token_transfer_token_id_migrator_progress_pkey PRIMARY KEY (id);


--
-- Name: token_transfers token_transfers_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.token_transfers
    ADD CONSTRAINT token_transfers_pkey PRIMARY KEY (transaction_hash, block_hash, log_index);


--
-- Name: tokens tokens_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.tokens
    ADD CONSTRAINT tokens_pkey PRIMARY KEY (contract_address_hash);


--
-- Name: transaction_errors transaction_errors_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transaction_errors
    ADD CONSTRAINT transaction_errors_pkey PRIMARY KEY (id);


--
-- Name: transaction_forks transaction_forks_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transaction_forks
    ADD CONSTRAINT transaction_forks_pkey PRIMARY KEY (uncle_hash, index);


--
-- Name: transaction_stats transaction_stats_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transaction_stats
    ADD CONSTRAINT transaction_stats_pkey PRIMARY KEY (id);


--
-- Name: transactions transactions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transactions
    ADD CONSTRAINT transactions_pkey PRIMARY KEY (hash);


--
-- Name: address_names unique_address_names; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.address_names
    ADD CONSTRAINT unique_address_names PRIMARY KEY (address_hash, name);


--
-- Name: user_contacts user_contacts_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_contacts
    ADD CONSTRAINT user_contacts_pkey PRIMARY KEY (id);


--
-- Name: user_operations user_operations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_operations
    ADD CONSTRAINT user_operations_pkey PRIMARY KEY (hash);


--
-- Name: user_ops_indexer_migrations user_ops_indexer_migrations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_ops_indexer_migrations
    ADD CONSTRAINT user_ops_indexer_migrations_pkey PRIMARY KEY (version);


--
-- Name: users users_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_pkey PRIMARY KEY (id);


--
-- Name: validators validators_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.validators
    ADD CONSTRAINT validators_pkey PRIMARY KEY (address_hash);


--
-- Name: withdrawals withdrawals_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.withdrawals
    ADD CONSTRAINT withdrawals_pkey PRIMARY KEY (index);


--
-- Name: address_coin_balances_block_number_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX address_coin_balances_block_number_index ON public.address_coin_balances USING btree (block_number);


--
-- Name: address_contract_code_fetch_attempts_address_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX address_contract_code_fetch_attempts_address_hash_index ON public.address_contract_code_fetch_attempts USING btree (address_hash);


--
-- Name: address_cur_token_balances_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX address_cur_token_balances_index ON public.address_current_token_balances USING btree (block_number);


--
-- Name: address_current_token_balance_token_contract_address_hash_v_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX address_current_token_balance_token_contract_address_hash_v_idx ON public.address_current_token_balances USING btree (token_contract_address_hash, value DESC, address_hash DESC) WHERE ((address_hash <> '\x0000000000000000000000000000000000000000'::bytea) AND (value > (0)::numeric));


--
-- Name: address_current_token_balances_token_contract_address_hash_valu; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX address_current_token_balances_token_contract_address_hash_valu ON public.address_current_token_balances USING btree (token_contract_address_hash, value);


--
-- Name: address_current_token_balances_token_id_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX address_current_token_balances_token_id_index ON public.address_current_token_balances USING btree (token_id);


--
-- Name: address_ids_to_address_hashes_address_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX address_ids_to_address_hashes_address_hash_index ON public.address_ids_to_address_hashes USING btree (address_hash);


--
-- Name: address_names_address_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX address_names_address_hash_index ON public.address_names USING btree (address_hash) WHERE ("primary" = true);


--
-- Name: address_tags_id_index; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX address_tags_id_index ON public.address_tags USING btree (id);


--
-- Name: address_tags_label_index; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX address_tags_label_index ON public.address_tags USING btree (label);


--
-- Name: address_to_tags_address_hash_tag_id_index; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX address_to_tags_address_hash_tag_id_index ON public.address_to_tags USING btree (address_hash, tag_id);


--
-- Name: address_token_balances_address_hash_token_contract_address_hash; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX address_token_balances_address_hash_token_contract_address_hash ON public.address_token_balances USING btree (address_hash, token_contract_address_hash, block_number);


--
-- Name: address_token_balances_block_number_address_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX address_token_balances_block_number_address_hash_index ON public.address_token_balances USING btree (block_number, address_hash);


--
-- Name: address_token_balances_token_contract_address_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX address_token_balances_token_contract_address_hash_index ON public.address_token_balances USING btree (token_contract_address_hash);


--
-- Name: address_token_balances_token_id_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX address_token_balances_token_id_index ON public.address_token_balances USING btree (token_id);


--
-- Name: addresses_fetched_coin_balance_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX addresses_fetched_coin_balance_hash_index ON public.addresses USING btree (fetched_coin_balance DESC, hash) WHERE (fetched_coin_balance > (0)::numeric);


--
-- Name: addresses_fetched_coin_balance_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX addresses_fetched_coin_balance_index ON public.addresses USING btree (fetched_coin_balance);


--
-- Name: addresses_hash_contract_code_not_null; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX addresses_hash_contract_code_not_null ON public.addresses USING btree (hash) WHERE (contract_code IS NOT NULL);


--
-- Name: addresses_inserted_at_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX addresses_inserted_at_index ON public.addresses USING btree (inserted_at);


--
-- Name: addresses_transactions_count_asc_coin_balance_desc_hash_partial; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX addresses_transactions_count_asc_coin_balance_desc_hash_partial ON public.addresses USING btree (transactions_count NULLS FIRST, fetched_coin_balance DESC, hash) WHERE (fetched_coin_balance > (0)::numeric);


--
-- Name: addresses_transactions_count_desc_partial; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX addresses_transactions_count_desc_partial ON public.addresses USING btree (transactions_count DESC NULLS LAST) WHERE (fetched_coin_balance > (0)::numeric);


--
-- Name: addresses_verified_fetched_coin_balance_DESC_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "addresses_verified_fetched_coin_balance_DESC_hash_index" ON public.addresses USING btree (fetched_coin_balance DESC NULLS LAST, hash) WHERE (verified = true);


--
-- Name: addresses_verified_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX addresses_verified_hash_index ON public.addresses USING btree (hash) WHERE (verified = true);


--
-- Name: addresses_verified_transactions_count_DESC_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "addresses_verified_transactions_count_DESC_hash_index" ON public.addresses USING btree (transactions_count DESC NULLS LAST, hash) WHERE (verified = true);


--
-- Name: administrators_user_id_index; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX administrators_user_id_index ON public.administrators USING btree (user_id);


--
-- Name: audit_report_unique_index; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX audit_report_unique_index ON public.smart_contract_audit_reports USING btree (address_hash, audit_report_url, audit_publish_date, audit_company_name);


--
-- Name: beacon_deposits_block_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX beacon_deposits_block_hash_index ON public.beacon_deposits USING btree (block_hash);


--
-- Name: beacon_deposits_from_address_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX beacon_deposits_from_address_hash_index ON public.beacon_deposits USING btree (from_address_hash);


--
-- Name: beacon_deposits_pubkey_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX beacon_deposits_pubkey_index ON public.beacon_deposits USING btree (pubkey) WHERE (status <> 'invalid'::public.beacon_deposits_status);


--
-- Name: beacon_deposits_pubkey_withdrawal_credentials_amount_signature_; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX beacon_deposits_pubkey_withdrawal_credentials_amount_signature_ ON public.beacon_deposits USING btree (pubkey, withdrawal_credentials, amount, signature, block_timestamp) WHERE (status = 'pending'::public.beacon_deposits_status);


--
-- Name: block_rewards_block_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX block_rewards_block_hash_index ON public.block_rewards USING btree (block_hash);


--
-- Name: blocks_consensus_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX blocks_consensus_index ON public.blocks USING btree (consensus);


--
-- Name: blocks_date; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX blocks_date ON public.blocks USING btree (date("timestamp"), number);


--
-- Name: blocks_inserted_at_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX blocks_inserted_at_index ON public.blocks USING btree (inserted_at);


--
-- Name: blocks_is_empty_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX blocks_is_empty_index ON public.blocks USING btree (is_empty);


--
-- Name: blocks_miner_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX blocks_miner_hash_index ON public.blocks USING btree (miner_hash);


--
-- Name: blocks_miner_hash_number_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX blocks_miner_hash_number_index ON public.blocks USING btree (miner_hash, number);


--
-- Name: blocks_number_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX blocks_number_index ON public.blocks USING btree (number);


--
-- Name: blocks_timestamp_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX blocks_timestamp_index ON public.blocks USING btree ("timestamp");


--
-- Name: bridged_tokens_home_token_contract_address_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX bridged_tokens_home_token_contract_address_hash_index ON public.bridged_tokens USING btree (home_token_contract_address_hash);


--
-- Name: consensus_block_hashes_refetch_needed; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX consensus_block_hashes_refetch_needed ON public.blocks USING btree (hash) WHERE (consensus AND refetch_needed);


--
-- Name: contract_methods_identifier_md5_abi_index; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX contract_methods_identifier_md5_abi_index ON public.contract_methods USING btree (identifier, md5((abi)::text));


--
-- Name: contract_methods_inserted_at_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX contract_methods_inserted_at_index ON public.contract_methods USING btree (inserted_at);


--
-- Name: csv_export_requests_pending_per_ip; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX csv_export_requests_pending_per_ip ON public.csv_export_requests USING btree (remote_ip_hash) WHERE ((status)::text = 'pending'::text);


--
-- Name: email_unique_for_user; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX email_unique_for_user ON public.user_contacts USING btree (user_id, email);


--
-- Name: empty_consensus_blocks; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX empty_consensus_blocks ON public.blocks USING btree (consensus) WHERE (is_empty IS NULL);


--
-- Name: fetched_current_token_balances; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX fetched_current_token_balances ON public.address_current_token_balances USING btree (address_hash, token_contract_address_hash, COALESCE(token_id, ('-1'::integer)::numeric));


--
-- Name: fetched_token_balances; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX fetched_token_balances ON public.address_token_balances USING btree (address_hash, token_contract_address_hash, COALESCE(token_id, ('-1'::integer)::numeric), block_number);


--
-- Name: fhe_operations_caller_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX fhe_operations_caller_index ON public.fhe_operations USING btree (caller) WHERE (caller IS NOT NULL);


--
-- Name: fhe_operations_fhe_type_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX fhe_operations_fhe_type_index ON public.fhe_operations USING btree (fhe_type);


--
-- Name: fhe_operations_log_index_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX fhe_operations_log_index_index ON public.fhe_operations USING btree (log_index);


--
-- Name: fhe_operations_operation_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX fhe_operations_operation_index ON public.fhe_operations USING btree (operation);


--
-- Name: fhe_operations_operation_type_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX fhe_operations_operation_type_index ON public.fhe_operations USING btree (operation_type);


--
-- Name: fhe_operations_transaction_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX fhe_operations_transaction_hash_index ON public.fhe_operations USING btree (transaction_hash);


--
-- Name: idx_hot_smart_contracts_date_gas; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_hot_smart_contracts_date_gas ON public.hot_smart_contracts_daily USING btree (date DESC, total_gas_used DESC);


--
-- Name: idx_hot_smart_contracts_date_transactions_count; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_hot_smart_contracts_date_transactions_count ON public.hot_smart_contracts_daily USING btree (date DESC, transactions_count DESC);


--
-- Name: internal_transactions_block_number_DESC_transaction_index_DESC_; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "internal_transactions_block_number_DESC_transaction_index_DESC_" ON public.internal_transactions USING btree (block_number DESC, transaction_index DESC, index DESC);


--
-- Name: internal_transactions_block_number_created_contract_address_id_; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX internal_transactions_block_number_created_contract_address_id_ ON public.internal_transactions USING btree (block_number, created_contract_address_id) WHERE (created_contract_address_id IS NOT NULL);


--
-- Name: internal_transactions_created_contract_address_id_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX internal_transactions_created_contract_address_id_index ON public.internal_transactions USING btree (created_contract_address_id);


--
-- Name: internal_transactions_from_address_id_partial_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX internal_transactions_from_address_id_partial_index ON public.internal_transactions USING btree (from_address_id, block_number DESC, transaction_index DESC, index DESC) WHERE ((((type)::text = 'call'::text) AND (index > 0)) OR ((type)::text <> 'call'::text));


--
-- Name: internal_transactions_to_address_id_partial_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX internal_transactions_to_address_id_partial_index ON public.internal_transactions USING btree (to_address_id, block_number DESC, transaction_index DESC, index DESC) WHERE ((((type)::text = 'call'::text) AND (index > 0)) OR ((type)::text <> 'call'::text));


--
-- Name: logs_address_hash_block_number_DESC_index_DESC_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "logs_address_hash_block_number_DESC_index_DESC_index" ON public.logs USING btree (address_hash, block_number DESC, index DESC);


--
-- Name: logs_address_hash_first_topic_block_number_index_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX logs_address_hash_first_topic_block_number_index_index ON public.logs USING btree (address_hash, first_topic, block_number, index);


--
-- Name: logs_block_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX logs_block_hash_index ON public.logs USING btree (block_hash);


--
-- Name: logs_block_number_DESC__index_DESC_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "logs_block_number_DESC__index_DESC_index" ON public.logs USING btree (block_number DESC, index DESC);


--
-- Name: logs_deposits_withdrawals_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX logs_deposits_withdrawals_index ON public.logs USING btree (transaction_hash, block_hash, index, address_hash) WHERE (first_topic = ANY (ARRAY['\xe1fffcc4923d04b559f4d29a8bfc6cda04eb5b0d3c460751c2402c5c5cc9109c'::bytea, '\x7fcf532c15f0a6db0bd6d0e038bea71d30d808c7d98cb3bf7268a95bf5081b65'::bytea]));


--
-- Name: logs_first_topic_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX logs_first_topic_index ON public.logs USING btree (first_topic);


--
-- Name: logs_fourth_topic_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX logs_fourth_topic_index ON public.logs USING btree (fourth_topic);


--
-- Name: logs_second_topic_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX logs_second_topic_index ON public.logs USING btree (second_topic);


--
-- Name: logs_third_topic_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX logs_third_topic_index ON public.logs USING btree (third_topic);


--
-- Name: logs_transaction_hash_index_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX logs_transaction_hash_index_index ON public.logs USING btree (transaction_hash, index);


--
-- Name: market_history_date_secondary_coin_index; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX market_history_date_secondary_coin_index ON public.market_history USING btree (date, secondary_coin);


--
-- Name: method_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX method_id ON public.transactions USING btree (SUBSTRING(input FROM 1 FOR 4));


--
-- Name: missing_block_ranges_from_number_to_number_index; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX missing_block_ranges_from_number_to_number_index ON public.missing_block_ranges USING btree (from_number, to_number);


--
-- Name: missing_block_ranges_priority_DESC_NULLS_LAST_from_number_DESC_; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "missing_block_ranges_priority_DESC_NULLS_LAST_from_number_DESC_" ON public.missing_block_ranges USING btree (priority DESC NULLS LAST, from_number DESC);


--
-- Name: multichain_search_db_main_export_queue_upper_block_range_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX multichain_search_db_main_export_queue_upper_block_range_index ON public.multichain_search_db_main_export_queue USING btree (upper(block_range) DESC);


--
-- Name: nephew_hash_to_uncle_hash; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX nephew_hash_to_uncle_hash ON public.block_second_degree_relations USING btree (nephew_hash, uncle_hash);


--
-- Name: oban_jobs_args_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX oban_jobs_args_index ON public.oban_jobs USING gin (args);


--
-- Name: oban_jobs_meta_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX oban_jobs_meta_index ON public.oban_jobs USING gin (meta);


--
-- Name: oban_jobs_state_cancelled_at_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX oban_jobs_state_cancelled_at_index ON public.oban_jobs USING btree (state, cancelled_at);


--
-- Name: oban_jobs_state_discarded_at_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX oban_jobs_state_discarded_at_index ON public.oban_jobs USING btree (state, discarded_at);


--
-- Name: oban_jobs_state_queue_priority_scheduled_at_id_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX oban_jobs_state_queue_priority_scheduled_at_id_index ON public.oban_jobs USING btree (state, queue, priority, scheduled_at, id);


--
-- Name: one_consensus_block_at_height; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX one_consensus_block_at_height ON public.blocks USING btree (number) WHERE consensus;


--
-- Name: one_consensus_child_per_parent; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX one_consensus_child_per_parent ON public.blocks USING btree (parent_hash) WHERE consensus;


--
-- Name: one_primary_per_user; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX one_primary_per_user ON public.user_contacts USING btree (user_id) WHERE "primary";


--
-- Name: owner_role_limit; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX owner_role_limit ON public.administrators USING btree (role) WHERE ((role)::text = 'owner'::text);


--
-- Name: pending_block_operations_block_number_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX pending_block_operations_block_number_index ON public.pending_block_operations USING btree (block_number);


--
-- Name: pending_txs_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX pending_txs_index ON public.transactions USING btree (inserted_at, hash) WHERE ((block_hash IS NULL) AND ((error IS NULL) OR ((error)::text <> 'dropped/replaced'::text)));


--
-- Name: proxy_implementations_proxy_type_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX proxy_implementations_proxy_type_index ON public.proxy_implementations USING btree (proxy_type);


--
-- Name: signed_authorizations_authority_nonce_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX signed_authorizations_authority_nonce_index ON public.signed_authorizations USING btree (authority, nonce);


--
-- Name: smart_contract_audit_reports_address_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX smart_contract_audit_reports_address_hash_index ON public.smart_contract_audit_reports USING btree (address_hash);


--
-- Name: smart_contracts_additional_sources_address_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX smart_contracts_additional_sources_address_hash_index ON public.smart_contracts_additional_sources USING btree (address_hash);


--
-- Name: smart_contracts_additional_sources_file_name_address_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX smart_contracts_additional_sources_file_name_address_hash_index ON public.smart_contracts_additional_sources USING btree (address_hash, file_name);


--
-- Name: smart_contracts_address_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX smart_contracts_address_hash_index ON public.smart_contracts USING btree (address_hash);


--
-- Name: smart_contracts_certified_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX smart_contracts_certified_index ON public.smart_contracts USING btree (certified);


--
-- Name: smart_contracts_contract_code_md5_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX smart_contracts_contract_code_md5_index ON public.smart_contracts USING btree (contract_code_md5);


--
-- Name: smart_contracts_language_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX smart_contracts_language_index ON public.smart_contracts USING btree (language);


--
-- Name: smart_contracts_trgm_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX smart_contracts_trgm_idx ON public.smart_contracts USING gin (to_tsvector('english'::regconfig, (name)::text));


--
-- Name: token_instance_metadata_refetch_attempts_token_contract_address; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX token_instance_metadata_refetch_attempts_token_contract_address ON public.token_instance_metadata_refetch_attempts USING btree (token_contract_address_hash, token_id);


--
-- Name: token_instances_error_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX token_instances_error_index ON public.token_instances USING btree (error);


--
-- Name: token_instances_owner_address_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX token_instances_owner_address_hash_index ON public.token_instances USING btree (owner_address_hash);


--
-- Name: token_instances_token_contract_address_hash_token_id_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX token_instances_token_contract_address_hash_token_id_index ON public.token_instances USING btree (token_contract_address_hash, token_id);


--
-- Name: token_transfers_block_consensus_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX token_transfers_block_consensus_index ON public.token_transfers USING btree (block_consensus);


--
-- Name: token_transfers_block_number_DESC_log_index_DESC_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "token_transfers_block_number_DESC_log_index_DESC_index" ON public.token_transfers USING btree (block_number DESC, log_index DESC);


--
-- Name: token_transfers_from_address_hash_block_number_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX token_transfers_from_address_hash_block_number_index ON public.token_transfers USING btree (from_address_hash, block_number);


--
-- Name: token_transfers_to_address_hash_block_number_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX token_transfers_to_address_hash_block_number_index ON public.token_transfers USING btree (to_address_hash, block_number);


--
-- Name: token_transfers_token_contract_address_hash__block_number_DESC_; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "token_transfers_token_contract_address_hash__block_number_DESC_" ON public.token_transfers USING btree (token_contract_address_hash, block_number DESC, log_index DESC);


--
-- Name: token_transfers_token_contract_address_hash_token_ids_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX token_transfers_token_contract_address_hash_token_ids_index ON public.token_transfers USING gin (token_contract_address_hash, token_ids);


--
-- Name: token_transfers_token_type_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX token_transfers_token_type_index ON public.token_transfers USING btree (token_type);


--
-- Name: token_transfers_transaction_hash_log_index_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX token_transfers_transaction_hash_log_index_index ON public.token_transfers USING btree (transaction_hash, log_index);


--
-- Name: tokens_name_partial_fts_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX tokens_name_partial_fts_index ON public.tokens USING gin (to_tsvector('english'::regconfig, name)) WHERE (symbol IS NULL);


--
-- Name: tokens_symbol_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX tokens_symbol_index ON public.tokens USING btree (symbol);


--
-- Name: tokens_trgm_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX tokens_trgm_idx ON public.tokens USING gin (to_tsvector('english'::regconfig, ((symbol || ' '::text) || name)));


--
-- Name: tokens_type_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX tokens_type_index ON public.tokens USING btree (type);


--
-- Name: transaction_errors_message_index; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX transaction_errors_message_index ON public.transaction_errors USING btree (message);


--
-- Name: transaction_stats_date_index; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX transaction_stats_date_index ON public.transaction_stats USING btree (date);


--
-- Name: transactions_block_consensus_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX transactions_block_consensus_index ON public.transactions USING btree (block_consensus);


--
-- Name: transactions_block_hash_error_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX transactions_block_hash_error_index ON public.transactions USING btree (block_hash, error);


--
-- Name: transactions_block_hash_index_index; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX transactions_block_hash_index_index ON public.transactions USING btree (block_hash, index);


--
-- Name: transactions_block_number_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX transactions_block_number_index ON public.transactions USING btree (block_number);


--
-- Name: transactions_block_timestamp_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX transactions_block_timestamp_index ON public.transactions USING btree (block_timestamp);


--
-- Name: transactions_created_contract_address_hash_w_pending_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX transactions_created_contract_address_hash_w_pending_index ON public.transactions USING btree (created_contract_address_hash, block_number, index, inserted_at, hash DESC) WHERE (created_contract_address_hash IS NOT NULL);


--
-- Name: transactions_created_contract_code_indexed_at_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX transactions_created_contract_code_indexed_at_index ON public.transactions USING btree (created_contract_code_indexed_at);


--
-- Name: transactions_from_address_hash_with_pending_index_asc; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX transactions_from_address_hash_with_pending_index_asc ON public.transactions USING btree (from_address_hash, block_number, index, inserted_at, hash DESC);


--
-- Name: transactions_has_token_transfers_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX transactions_has_token_transfers_index ON public.transactions USING btree (has_token_transfers);


--
-- Name: transactions_inserted_at_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX transactions_inserted_at_index ON public.transactions USING btree (inserted_at);


--
-- Name: transactions_nonce_from_address_hash_block_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX transactions_nonce_from_address_hash_block_hash_index ON public.transactions USING btree (nonce, from_address_hash, block_hash);


--
-- Name: transactions_recent_blob_transactions_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX transactions_recent_blob_transactions_index ON public.transactions USING btree (block_number DESC, index DESC) WHERE (type = 3);


--
-- Name: transactions_recent_collated_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX transactions_recent_collated_index ON public.transactions USING btree (block_number DESC, index DESC);


--
-- Name: transactions_status_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX transactions_status_index ON public.transactions USING btree (status);


--
-- Name: transactions_to_address_hash_with_pending_index_asc; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX transactions_to_address_hash_with_pending_index_asc ON public.transactions USING btree (to_address_hash, block_number, index, inserted_at, hash DESC);


--
-- Name: transactions_token_transfer_method_id_ordered_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX transactions_token_transfer_method_id_ordered_index ON public.transactions USING btree (SUBSTRING(input FROM 1 FOR 4), block_number DESC, index DESC) WHERE (has_token_transfers = true);


--
-- Name: transactions_updated_at_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX transactions_updated_at_index ON public.transactions USING btree (updated_at);


--
-- Name: uncataloged_tokens; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX uncataloged_tokens ON public.tokens USING btree (cataloged) WHERE (cataloged = false);


--
-- Name: uncle_hash_to_nephew_hash; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX uncle_hash_to_nephew_hash ON public.block_second_degree_relations USING btree (uncle_hash, nephew_hash);


--
-- Name: unfetched_address_token_balances_v2_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX unfetched_address_token_balances_v2_index ON public.address_token_balances USING btree (id) WHERE ((((address_hash <> '\x0000000000000000000000000000000000000000'::bytea) AND ((token_type)::text = 'ERC-721'::text)) OR ((token_type)::text = 'ERC-20'::text) OR ((token_type)::text = 'ERC-1155'::text) OR ((token_type)::text = 'ERC-404'::text)) AND ((value_fetched_at IS NULL) OR (value IS NULL)));


--
-- Name: unfetched_balances; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX unfetched_balances ON public.address_coin_balances USING btree (address_hash, block_number) WHERE (value_fetched_at IS NULL);


--
-- Name: unfetched_uncles; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX unfetched_uncles ON public.block_second_degree_relations USING btree (nephew_hash, uncle_hash) WHERE (uncle_fetched_at IS NULL);


--
-- Name: unique_multichain_search_db_current_token_balances; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX unique_multichain_search_db_current_token_balances ON public.multichain_search_db_export_balances_queue USING btree (address_hash, token_contract_address_hash_or_native, COALESCE(token_id, ('-1'::integer)::numeric));


--
-- Name: unique_username; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX unique_username ON public.users USING btree (username);


--
-- Name: user_operations_block_number_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX user_operations_block_number_hash_index ON public.user_operations USING btree (block_number DESC, hash DESC);


--
-- Name: user_operations_block_number_transaction_hash_bundle_index_inde; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX user_operations_block_number_transaction_hash_bundle_index_inde ON public.user_operations USING btree (block_number DESC, transaction_hash DESC, bundle_index DESC);


--
-- Name: user_operations_bundler_transaction_hash_bundle_index_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX user_operations_bundler_transaction_hash_bundle_index_index ON public.user_operations USING btree (bundler, transaction_hash, bundle_index);


--
-- Name: user_operations_factory_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX user_operations_factory_index ON public.user_operations USING btree (factory);


--
-- Name: user_operations_paymaster_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX user_operations_paymaster_index ON public.user_operations USING btree (paymaster);


--
-- Name: user_operations_sender_factory_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX user_operations_sender_factory_index ON public.user_operations USING btree (sender, factory);


--
-- Name: user_operations_transaction_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX user_operations_transaction_hash_index ON public.user_operations USING btree (transaction_hash);


--
-- Name: withdrawals_address_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX withdrawals_address_hash_index ON public.withdrawals USING btree (address_hash);


--
-- Name: withdrawals_block_hash_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX withdrawals_block_hash_index ON public.withdrawals USING btree (block_hash);


--
-- Name: token_instances repack_trigger; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER repack_trigger AFTER INSERT OR DELETE OR UPDATE ON public.token_instances FOR EACH ROW EXECUTE FUNCTION repack.repack_trigger('token_id', 'token_contract_address_hash');

ALTER TABLE public.token_instances ENABLE ALWAYS TRIGGER repack_trigger;


--
-- Name: address_to_tags address_to_tags_tag_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.address_to_tags
    ADD CONSTRAINT address_to_tags_tag_id_fkey FOREIGN KEY (tag_id) REFERENCES public.address_tags(id);


--
-- Name: administrators administrators_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.administrators
    ADD CONSTRAINT administrators_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: beacon_blobs_transactions beacon_blobs_transactions_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.beacon_blobs_transactions
    ADD CONSTRAINT beacon_blobs_transactions_hash_fkey FOREIGN KEY (hash) REFERENCES public.transactions(hash) ON DELETE CASCADE;


--
-- Name: block_rewards block_rewards_block_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.block_rewards
    ADD CONSTRAINT block_rewards_block_hash_fkey FOREIGN KEY (block_hash) REFERENCES public.blocks(hash) ON DELETE CASCADE;


--
-- Name: block_second_degree_relations block_second_degree_relations_nephew_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.block_second_degree_relations
    ADD CONSTRAINT block_second_degree_relations_nephew_hash_fkey FOREIGN KEY (nephew_hash) REFERENCES public.blocks(hash);


--
-- Name: bridged_tokens bridged_tokens_home_token_contract_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.bridged_tokens
    ADD CONSTRAINT bridged_tokens_home_token_contract_address_hash_fkey FOREIGN KEY (home_token_contract_address_hash) REFERENCES public.tokens(contract_address_hash);


--
-- Name: deleted_internal_transactions_address_placeholders deleted_internal_transactions_address_placeholders_address_id_f; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.deleted_internal_transactions_address_placeholders
    ADD CONSTRAINT deleted_internal_transactions_address_placeholders_address_id_f FOREIGN KEY (address_id) REFERENCES public.address_ids_to_address_hashes(address_id);


--
-- Name: fhe_operations fhe_operations_block_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.fhe_operations
    ADD CONSTRAINT fhe_operations_block_hash_fkey FOREIGN KEY (block_hash) REFERENCES public.blocks(hash) ON DELETE CASCADE;


--
-- Name: fhe_operations fhe_operations_transaction_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.fhe_operations
    ADD CONSTRAINT fhe_operations_transaction_hash_fkey FOREIGN KEY (transaction_hash) REFERENCES public.transactions(hash) ON DELETE CASCADE;


--
-- Name: internal_transactions internal_transactions_error_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.internal_transactions
    ADD CONSTRAINT internal_transactions_error_id_fkey FOREIGN KEY (error_id) REFERENCES public.transaction_errors(id);


--
-- Name: logs logs_block_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.logs
    ADD CONSTRAINT logs_block_hash_fkey FOREIGN KEY (block_hash) REFERENCES public.blocks(hash);


--
-- Name: logs logs_transaction_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.logs
    ADD CONSTRAINT logs_transaction_hash_fkey FOREIGN KEY (transaction_hash) REFERENCES public.transactions(hash) ON DELETE CASCADE;


--
-- Name: pending_block_operations pending_block_operations_block_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.pending_block_operations
    ADD CONSTRAINT pending_block_operations_block_hash_fkey FOREIGN KEY (block_hash) REFERENCES public.blocks(hash) ON DELETE CASCADE;


--
-- Name: pending_transaction_operations pending_transaction_operations_transaction_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.pending_transaction_operations
    ADD CONSTRAINT pending_transaction_operations_transaction_hash_fkey FOREIGN KEY (transaction_hash) REFERENCES public.transactions(hash) ON DELETE CASCADE;


--
-- Name: proxy_smart_contract_verification_statuses proxy_smart_contract_verification_statuses_contract_address_has; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.proxy_smart_contract_verification_statuses
    ADD CONSTRAINT proxy_smart_contract_verification_statuses_contract_address_has FOREIGN KEY (contract_address_hash) REFERENCES public.smart_contracts(address_hash) ON DELETE CASCADE;


--
-- Name: scam_address_badge_mappings scam_address_badge_mappings_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.scam_address_badge_mappings
    ADD CONSTRAINT scam_address_badge_mappings_address_hash_fkey FOREIGN KEY (address_hash) REFERENCES public.addresses(hash) ON DELETE CASCADE;


--
-- Name: signed_authorizations signed_authorizations_transaction_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.signed_authorizations
    ADD CONSTRAINT signed_authorizations_transaction_hash_fkey FOREIGN KEY (transaction_hash) REFERENCES public.transactions(hash) ON DELETE CASCADE;


--
-- Name: smart_contract_audit_reports smart_contract_audit_reports_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.smart_contract_audit_reports
    ADD CONSTRAINT smart_contract_audit_reports_address_hash_fkey FOREIGN KEY (address_hash) REFERENCES public.smart_contracts(address_hash) ON DELETE CASCADE;


--
-- Name: smart_contracts_additional_sources smart_contracts_additional_sources_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.smart_contracts_additional_sources
    ADD CONSTRAINT smart_contracts_additional_sources_address_hash_fkey FOREIGN KEY (address_hash) REFERENCES public.smart_contracts(address_hash) ON DELETE CASCADE;


--
-- Name: token_instances token_instances_token_contract_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.token_instances
    ADD CONSTRAINT token_instances_token_contract_address_hash_fkey FOREIGN KEY (token_contract_address_hash) REFERENCES public.tokens(contract_address_hash);


--
-- Name: token_transfers token_transfers_block_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.token_transfers
    ADD CONSTRAINT token_transfers_block_hash_fkey FOREIGN KEY (block_hash) REFERENCES public.blocks(hash);


--
-- Name: token_transfers token_transfers_transaction_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.token_transfers
    ADD CONSTRAINT token_transfers_transaction_hash_fkey FOREIGN KEY (transaction_hash) REFERENCES public.transactions(hash) ON DELETE CASCADE;


--
-- Name: transaction_forks transaction_forks_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transaction_forks
    ADD CONSTRAINT transaction_forks_hash_fkey FOREIGN KEY (hash) REFERENCES public.transactions(hash) ON DELETE CASCADE;


--
-- Name: transaction_forks transaction_forks_uncle_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transaction_forks
    ADD CONSTRAINT transaction_forks_uncle_hash_fkey FOREIGN KEY (uncle_hash) REFERENCES public.blocks(hash) ON DELETE CASCADE;


--
-- Name: transactions transactions_block_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transactions
    ADD CONSTRAINT transactions_block_hash_fkey FOREIGN KEY (block_hash) REFERENCES public.blocks(hash) ON DELETE CASCADE;


--
-- Name: user_contacts user_contacts_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_contacts
    ADD CONSTRAINT user_contacts_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: withdrawals withdrawals_block_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.withdrawals
    ADD CONSTRAINT withdrawals_block_hash_fkey FOREIGN KEY (block_hash) REFERENCES public.blocks(hash) ON DELETE CASCADE;


--
-- PostgreSQL database dump complete
--
