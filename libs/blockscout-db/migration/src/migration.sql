--
-- PostgreSQL database dump
--

-- Dumped from database version 15.1 (Debian 15.1-1.pgdg110+1)
-- Dumped by pg_dump version 15.1 (Debian 15.1-1.pgdg110+1)

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: btree_gist; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS btree_gist WITH SCHEMA public;


--
-- Name: EXTENSION btree_gist; Type: COMMENT; Schema: -; Owner: 
--

COMMENT ON EXTENSION btree_gist IS 'support for indexing common datatypes in GiST';


--
-- Name: citext; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS citext WITH SCHEMA public;


--
-- Name: EXTENSION citext; Type: COMMENT; Schema: -; Owner: 
--

COMMENT ON EXTENSION citext IS 'data type for case-insensitive character strings';


--
-- Name: pg_trgm; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public;


--
-- Name: EXTENSION pg_trgm; Type: COMMENT; Schema: -; Owner: 
--

COMMENT ON EXTENSION pg_trgm IS 'text similarity measurement and index searching based on trigrams';


SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: address_coin_balances; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.address_coin_balances (
    address_hash bytea NOT NULL,
    block_number bigint NOT NULL,
    value numeric(100,0) DEFAULT NULL::numeric,
    value_fetched_at timestamp without time zone,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


ALTER TABLE public.address_coin_balances OWNER TO postgres;

--
-- Name: address_coin_balances_daily; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.address_coin_balances_daily (
    address_hash bytea NOT NULL,
    day date NOT NULL,
    value numeric(100,0) DEFAULT NULL::numeric,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


ALTER TABLE public.address_coin_balances_daily OWNER TO postgres;

--
-- Name: address_current_token_balances; Type: TABLE; Schema: public; Owner: postgres
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
    token_type character varying(255)
);


ALTER TABLE public.address_current_token_balances OWNER TO postgres;

--
-- Name: address_current_token_balances_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.address_current_token_balances_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.address_current_token_balances_id_seq OWNER TO postgres;

--
-- Name: address_current_token_balances_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.address_current_token_balances_id_seq OWNED BY public.address_current_token_balances.id;


--
-- Name: address_names; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.address_names (
    address_hash bytea NOT NULL,
    name character varying(255) NOT NULL,
    "primary" boolean DEFAULT false NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    metadata jsonb,
    id integer NOT NULL
);


ALTER TABLE public.address_names OWNER TO postgres;

--
-- Name: address_names_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.address_names_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.address_names_id_seq OWNER TO postgres;

--
-- Name: address_names_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.address_names_id_seq OWNED BY public.address_names.id;


--
-- Name: address_token_balances; Type: TABLE; Schema: public; Owner: postgres
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
    token_type character varying(255)
);


ALTER TABLE public.address_token_balances OWNER TO postgres;

--
-- Name: address_token_balances_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.address_token_balances_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.address_token_balances_id_seq OWNER TO postgres;

--
-- Name: address_token_balances_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.address_token_balances_id_seq OWNED BY public.address_token_balances.id;


--
-- Name: addresses; Type: TABLE; Schema: public; Owner: postgres
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


ALTER TABLE public.addresses OWNER TO postgres;

--
-- Name: administrators; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.administrators (
    id bigint NOT NULL,
    role character varying(255) NOT NULL,
    user_id bigint NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


ALTER TABLE public.administrators OWNER TO postgres;

--
-- Name: administrators_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.administrators_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.administrators_id_seq OWNER TO postgres;

--
-- Name: administrators_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.administrators_id_seq OWNED BY public.administrators.id;


--
-- Name: block_rewards; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.block_rewards (
    address_hash bytea NOT NULL,
    address_type character varying(255) NOT NULL,
    block_hash bytea NOT NULL,
    reward numeric(100,0),
    inserted_at timestamp(0) without time zone NOT NULL,
    updated_at timestamp(0) without time zone NOT NULL
);


ALTER TABLE public.block_rewards OWNER TO postgres;

--
-- Name: block_second_degree_relations; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.block_second_degree_relations (
    nephew_hash bytea NOT NULL,
    uncle_hash bytea NOT NULL,
    uncle_fetched_at timestamp without time zone,
    index integer
);


ALTER TABLE public.block_second_degree_relations OWNER TO postgres;

--
-- Name: blocks; Type: TABLE; Schema: public; Owner: postgres
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
    is_empty boolean
);


ALTER TABLE public.blocks OWNER TO postgres;

--
-- Name: contract_methods; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.contract_methods (
    id bigint NOT NULL,
    identifier integer NOT NULL,
    abi jsonb NOT NULL,
    type character varying(255) NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


ALTER TABLE public.contract_methods OWNER TO postgres;

--
-- Name: contract_methods_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.contract_methods_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.contract_methods_id_seq OWNER TO postgres;

--
-- Name: contract_methods_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.contract_methods_id_seq OWNED BY public.contract_methods.id;


--
-- Name: contract_verification_status; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.contract_verification_status (
    uid character varying(64) NOT NULL,
    status smallint NOT NULL,
    address_hash bytea NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


ALTER TABLE public.contract_verification_status OWNER TO postgres;

--
-- Name: decompiled_smart_contracts; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.decompiled_smart_contracts (
    id bigint NOT NULL,
    decompiler_version character varying(255) NOT NULL,
    decompiled_source_code text NOT NULL,
    address_hash bytea NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


ALTER TABLE public.decompiled_smart_contracts OWNER TO postgres;

--
-- Name: decompiled_smart_contracts_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.decompiled_smart_contracts_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.decompiled_smart_contracts_id_seq OWNER TO postgres;

--
-- Name: decompiled_smart_contracts_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.decompiled_smart_contracts_id_seq OWNED BY public.decompiled_smart_contracts.id;


--
-- Name: emission_rewards; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.emission_rewards (
    block_range int8range NOT NULL,
    reward numeric
);


ALTER TABLE public.emission_rewards OWNER TO postgres;

--
-- Name: event_notifications; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.event_notifications (
    id bigint NOT NULL,
    data text NOT NULL
);


ALTER TABLE public.event_notifications OWNER TO postgres;

--
-- Name: event_notifications_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.event_notifications_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.event_notifications_id_seq OWNER TO postgres;

--
-- Name: event_notifications_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.event_notifications_id_seq OWNED BY public.event_notifications.id;


--
-- Name: internal_transactions; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.internal_transactions (
    call_type character varying(255),
    created_contract_code bytea,
    error character varying(255),
    gas numeric(100,0),
    gas_used numeric(100,0),
    index integer NOT NULL,
    init bytea,
    input bytea,
    output bytea,
    trace_address integer[] NOT NULL,
    type character varying(255) NOT NULL,
    value numeric(100,0) NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    created_contract_address_hash bytea,
    from_address_hash bytea,
    to_address_hash bytea,
    transaction_hash bytea NOT NULL,
    block_number integer,
    transaction_index integer,
    block_hash bytea NOT NULL,
    block_index integer NOT NULL,
    CONSTRAINT call_has_error_or_result CHECK ((((type)::text <> 'call'::text) OR ((gas IS NOT NULL) AND (((error IS NULL) AND (gas_used IS NOT NULL) AND (output IS NOT NULL)) OR ((error IS NOT NULL) AND (gas_used IS NULL) AND (output IS NULL)))))),
    CONSTRAINT create_has_error_or_result CHECK ((((type)::text <> 'create'::text) OR ((gas IS NOT NULL) AND (((error IS NULL) AND (created_contract_address_hash IS NOT NULL) AND (created_contract_code IS NOT NULL) AND (gas_used IS NOT NULL)) OR ((error IS NOT NULL) AND (created_contract_address_hash IS NULL) AND (created_contract_code IS NULL) AND (gas_used IS NULL))))))
);


ALTER TABLE public.internal_transactions OWNER TO postgres;

--
-- Name: last_fetched_counters; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.last_fetched_counters (
    counter_type character varying(255) NOT NULL,
    value numeric(100,0),
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


ALTER TABLE public.last_fetched_counters OWNER TO postgres;

--
-- Name: logs; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.logs (
    data bytea NOT NULL,
    index integer NOT NULL,
    type character varying(255),
    first_topic character varying(255),
    second_topic character varying(255),
    third_topic character varying(255),
    fourth_topic character varying(255),
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    address_hash bytea,
    transaction_hash bytea NOT NULL,
    block_hash bytea NOT NULL,
    block_number integer
);


ALTER TABLE public.logs OWNER TO postgres;

--
-- Name: market_history; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.market_history (
    id bigint NOT NULL,
    date date,
    closing_price numeric,
    opening_price numeric
);


ALTER TABLE public.market_history OWNER TO postgres;

--
-- Name: market_history_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.market_history_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.market_history_id_seq OWNER TO postgres;

--
-- Name: market_history_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.market_history_id_seq OWNED BY public.market_history.id;


--
-- Name: pending_block_operations; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.pending_block_operations (
    block_hash bytea NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    fetch_internal_transactions boolean NOT NULL
);


ALTER TABLE public.pending_block_operations OWNER TO postgres;

--
-- Name: schema_migrations; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.schema_migrations (
    version bigint NOT NULL,
    inserted_at timestamp(0) without time zone
);


ALTER TABLE public.schema_migrations OWNER TO postgres;

--
-- Name: smart_contracts; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.smart_contracts (
    id bigint NOT NULL,
    name character varying(255) NOT NULL,
    compiler_version character varying(255) NOT NULL,
    optimization boolean NOT NULL,
    contract_source_code text NOT NULL,
    abi jsonb NOT NULL,
    address_hash bytea NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    constructor_arguments text,
    optimization_runs bigint,
    evm_version character varying(255),
    external_libraries jsonb[] DEFAULT ARRAY[]::jsonb[],
    verified_via_sourcify boolean,
    is_vyper_contract boolean,
    partially_verified boolean,
    file_path text,
    is_changed_bytecode boolean DEFAULT false,
    bytecode_checked_at timestamp without time zone DEFAULT ((now() AT TIME ZONE 'utc'::text) - '1 day'::interval),
    contract_code_md5 character varying(255) NOT NULL,
    implementation_name character varying(255)
);


ALTER TABLE public.smart_contracts OWNER TO postgres;

--
-- Name: smart_contracts_additional_sources; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.smart_contracts_additional_sources (
    id bigint NOT NULL,
    file_name character varying(255) NOT NULL,
    contract_source_code text NOT NULL,
    address_hash bytea NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


ALTER TABLE public.smart_contracts_additional_sources OWNER TO postgres;

--
-- Name: smart_contracts_additional_sources_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.smart_contracts_additional_sources_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.smart_contracts_additional_sources_id_seq OWNER TO postgres;

--
-- Name: smart_contracts_additional_sources_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.smart_contracts_additional_sources_id_seq OWNED BY public.smart_contracts_additional_sources.id;


--
-- Name: smart_contracts_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.smart_contracts_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.smart_contracts_id_seq OWNER TO postgres;

--
-- Name: smart_contracts_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.smart_contracts_id_seq OWNED BY public.smart_contracts.id;


--
-- Name: token_instances; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.token_instances (
    token_id numeric(78,0) NOT NULL,
    token_contract_address_hash bytea NOT NULL,
    metadata jsonb,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    error character varying(255)
);


ALTER TABLE public.token_instances OWNER TO postgres;

--
-- Name: token_transfers; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.token_transfers (
    transaction_hash bytea NOT NULL,
    log_index integer NOT NULL,
    from_address_hash bytea NOT NULL,
    to_address_hash bytea NOT NULL,
    amount numeric,
    token_id numeric(78,0),
    token_contract_address_hash bytea NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    block_number integer,
    block_hash bytea NOT NULL,
    amounts numeric[],
    token_ids numeric(78,0)[]
);


ALTER TABLE public.token_transfers OWNER TO postgres;

--
-- Name: tokens; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.tokens (
    name text,
    symbol character varying(255),
    total_supply numeric,
    decimals numeric,
    type character varying(255) NOT NULL,
    cataloged boolean DEFAULT false,
    contract_address_hash bytea NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    holder_count integer,
    skip_metadata boolean
);


ALTER TABLE public.tokens OWNER TO postgres;

--
-- Name: transaction_forks; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.transaction_forks (
    hash bytea NOT NULL,
    index integer NOT NULL,
    uncle_hash bytea NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


ALTER TABLE public.transaction_forks OWNER TO postgres;

--
-- Name: transaction_stats; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.transaction_stats (
    id bigint NOT NULL,
    date date,
    number_of_transactions integer,
    gas_used numeric(100,0),
    total_fee numeric(100,0)
);


ALTER TABLE public.transaction_stats OWNER TO postgres;

--
-- Name: transaction_stats_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.transaction_stats_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.transaction_stats_id_seq OWNER TO postgres;

--
-- Name: transaction_stats_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.transaction_stats_id_seq OWNED BY public.transaction_stats.id;


--
-- Name: transactions; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.transactions (
    cumulative_gas_used numeric(100,0),
    error character varying(255),
    gas numeric(100,0) NOT NULL,
    gas_price numeric(100,0) NOT NULL,
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
    has_error_in_internal_txs boolean,
    CONSTRAINT collated_block_number CHECK (((block_hash IS NULL) OR (block_number IS NOT NULL))),
    CONSTRAINT collated_cumalative_gas_used CHECK (((block_hash IS NULL) OR (cumulative_gas_used IS NOT NULL))),
    CONSTRAINT collated_gas_used CHECK (((block_hash IS NULL) OR (gas_used IS NOT NULL))),
    CONSTRAINT collated_index CHECK (((block_hash IS NULL) OR (index IS NOT NULL))),
    CONSTRAINT error CHECK (((status = 0) OR ((status <> 0) AND (error IS NULL)))),
    CONSTRAINT pending_block_number CHECK (((block_hash IS NOT NULL) OR (block_number IS NULL))),
    CONSTRAINT pending_cumalative_gas_used CHECK (((block_hash IS NOT NULL) OR (cumulative_gas_used IS NULL))),
    CONSTRAINT pending_gas_used CHECK (((block_hash IS NOT NULL) OR (gas_used IS NULL))),
    CONSTRAINT pending_index CHECK (((block_hash IS NOT NULL) OR (index IS NULL))),
    CONSTRAINT status CHECK ((((block_hash IS NULL) AND (status IS NULL)) OR (block_hash IS NOT NULL) OR ((status = 0) AND ((error)::text = 'dropped/replaced'::text))))
);


ALTER TABLE public.transactions OWNER TO postgres;

--
-- Name: user_contacts; Type: TABLE; Schema: public; Owner: postgres
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


ALTER TABLE public.user_contacts OWNER TO postgres;

--
-- Name: user_contacts_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.user_contacts_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.user_contacts_id_seq OWNER TO postgres;

--
-- Name: user_contacts_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.user_contacts_id_seq OWNED BY public.user_contacts.id;


--
-- Name: users; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.users (
    id bigint NOT NULL,
    username public.citext NOT NULL,
    password_hash character varying(255) NOT NULL,
    inserted_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL
);


ALTER TABLE public.users OWNER TO postgres;

--
-- Name: users_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.users_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.users_id_seq OWNER TO postgres;

--
-- Name: users_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.users_id_seq OWNED BY public.users.id;


--
-- Name: address_current_token_balances id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.address_current_token_balances ALTER COLUMN id SET DEFAULT nextval('public.address_current_token_balances_id_seq'::regclass);


--
-- Name: address_names id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.address_names ALTER COLUMN id SET DEFAULT nextval('public.address_names_id_seq'::regclass);


--
-- Name: address_token_balances id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.address_token_balances ALTER COLUMN id SET DEFAULT nextval('public.address_token_balances_id_seq'::regclass);


--
-- Name: administrators id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.administrators ALTER COLUMN id SET DEFAULT nextval('public.administrators_id_seq'::regclass);


--
-- Name: contract_methods id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.contract_methods ALTER COLUMN id SET DEFAULT nextval('public.contract_methods_id_seq'::regclass);


--
-- Name: decompiled_smart_contracts id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.decompiled_smart_contracts ALTER COLUMN id SET DEFAULT nextval('public.decompiled_smart_contracts_id_seq'::regclass);


--
-- Name: event_notifications id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.event_notifications ALTER COLUMN id SET DEFAULT nextval('public.event_notifications_id_seq'::regclass);


--
-- Name: market_history id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.market_history ALTER COLUMN id SET DEFAULT nextval('public.market_history_id_seq'::regclass);


--
-- Name: smart_contracts id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.smart_contracts ALTER COLUMN id SET DEFAULT nextval('public.smart_contracts_id_seq'::regclass);


--
-- Name: smart_contracts_additional_sources id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.smart_contracts_additional_sources ALTER COLUMN id SET DEFAULT nextval('public.smart_contracts_additional_sources_id_seq'::regclass);


--
-- Name: transaction_stats id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.transaction_stats ALTER COLUMN id SET DEFAULT nextval('public.transaction_stats_id_seq'::regclass);


--
-- Name: user_contacts id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_contacts ALTER COLUMN id SET DEFAULT nextval('public.user_contacts_id_seq'::regclass);


--
-- Name: users id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.users ALTER COLUMN id SET DEFAULT nextval('public.users_id_seq'::regclass);


--
-- Data for Name: address_coin_balances; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: address_coin_balances_daily; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: address_current_token_balances; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: address_names; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: address_token_balances; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: addresses; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: administrators; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: block_rewards; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: block_second_degree_relations; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: blocks; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: contract_methods; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: contract_verification_status; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: decompiled_smart_contracts; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: emission_rewards; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: event_notifications; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: internal_transactions; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: last_fetched_counters; Type: TABLE DATA; Schema: public; Owner: postgres
--

INSERT INTO public.last_fetched_counters VALUES ('min_missing_block_number', 0, '2022-12-08 13:42:54.023629', '2022-12-08 13:42:54.023629');


--
-- Data for Name: logs; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: market_history; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: pending_block_operations; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: schema_migrations; Type: TABLE DATA; Schema: public; Owner: postgres
--

INSERT INTO public.schema_migrations VALUES (20180117221921, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20180117221922, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20180117221923, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20180212222309, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20180221001948, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20180424203101, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20180508183700, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20180508191045, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20180518221256, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20180522154252, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20180522154253, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20180606135149, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20180606135150, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20180626143840, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20180717204948, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20180817021704, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20180821142139, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20180917182319, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20180918200001, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20180919175123, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20181008195723, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20181011193212, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20181015173318, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20181015173319, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20181016163236, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20181017141409, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20181024141113, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20181024164623, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20181024172010, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20181026180921, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20181029174420, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20181106152300, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20181107164103, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20181108205650, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20181121170616, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20181126203826, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20181206200140, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20181206200312, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20181212115448, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20181213111656, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20181221143000, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20181221145054, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190102141900, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190114204640, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190116082843, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190118040301, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190118152240, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190122125815, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190124082812, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190208113202, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190208143201, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190213180502, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190214135850, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190215080049, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190215093358, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190215105501, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190219082636, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190228102650, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190228152333, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190228220746, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190301095620, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190301120328, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190305095926, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190313085740, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190313103912, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190314084907, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190318151809, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190319081821, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190321185644, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190325081658, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190403080447, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190421143300, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190424170833, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190508152922, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190513134025, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190516140202, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190516160535, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190521104412, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190523112839, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190613065856, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190619154943, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190625085852, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190709043832, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190709103104, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190807111216, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190807113117, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190827120224, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190905083522, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20190910170703, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20191007082500, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20191009121635, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20191010075740, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20191018120546, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20191018140054, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20191121064805, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20191122062035, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20191128124415, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20191203112646, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20191208135613, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20191218120138, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20191220113006, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20200214152058, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20200410115841, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20200410141202, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20200421102450, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20200424070607, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20200518075748, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20200521090250, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20200521170002, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20200525115811, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20200527144742, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20200608075122, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20200806125649, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20200807064700, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20200812143050, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20200904075501, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20200929075625, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20201026093652, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20201214203532, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20210226154732, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20210309104122, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20210331074008, '2022-12-08 13:42:53');
INSERT INTO public.schema_migrations VALUES (20210422115740, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20210423084253, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20210423091652, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20210423094801, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20210423115108, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20210524165427, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20210527093756, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20210616120552, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20210701084814, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20210811140837, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20210823144531, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20210916194004, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20211006121008, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20211013190346, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20211017135545, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20211018072347, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20211018073652, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20211018164843, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20211018170533, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20211018170638, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20211029085117, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20211115164817, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20211203115010, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20211204184037, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20211206071033, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20211217201759, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20220111085751, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20220303083252, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20220306091504, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20220622114402, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20220622140604, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20220706101103, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20220706102257, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20220706102504, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20220706102746, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20220706105925, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20220706111510, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20220804114005, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20220902083436, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20220902103213, '2022-12-08 13:42:54');
INSERT INTO public.schema_migrations VALUES (20220902103527, '2022-12-08 13:42:54');


--
-- Data for Name: smart_contracts; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: smart_contracts_additional_sources; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: token_instances; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: token_transfers; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: tokens; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: transaction_forks; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: transaction_stats; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: transactions; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: user_contacts; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Data for Name: users; Type: TABLE DATA; Schema: public; Owner: postgres
--



--
-- Name: address_current_token_balances_id_seq; Type: SEQUENCE SET; Schema: public; Owner: postgres
--

SELECT pg_catalog.setval('public.address_current_token_balances_id_seq', 1, false);


--
-- Name: address_names_id_seq; Type: SEQUENCE SET; Schema: public; Owner: postgres
--

SELECT pg_catalog.setval('public.address_names_id_seq', 1, false);


--
-- Name: address_token_balances_id_seq; Type: SEQUENCE SET; Schema: public; Owner: postgres
--

SELECT pg_catalog.setval('public.address_token_balances_id_seq', 1, false);


--
-- Name: administrators_id_seq; Type: SEQUENCE SET; Schema: public; Owner: postgres
--

SELECT pg_catalog.setval('public.administrators_id_seq', 1, false);


--
-- Name: contract_methods_id_seq; Type: SEQUENCE SET; Schema: public; Owner: postgres
--

SELECT pg_catalog.setval('public.contract_methods_id_seq', 1, false);


--
-- Name: decompiled_smart_contracts_id_seq; Type: SEQUENCE SET; Schema: public; Owner: postgres
--

SELECT pg_catalog.setval('public.decompiled_smart_contracts_id_seq', 1, false);


--
-- Name: event_notifications_id_seq; Type: SEQUENCE SET; Schema: public; Owner: postgres
--

SELECT pg_catalog.setval('public.event_notifications_id_seq', 1, false);


--
-- Name: market_history_id_seq; Type: SEQUENCE SET; Schema: public; Owner: postgres
--

SELECT pg_catalog.setval('public.market_history_id_seq', 1, false);


--
-- Name: smart_contracts_additional_sources_id_seq; Type: SEQUENCE SET; Schema: public; Owner: postgres
--

SELECT pg_catalog.setval('public.smart_contracts_additional_sources_id_seq', 1, false);


--
-- Name: smart_contracts_id_seq; Type: SEQUENCE SET; Schema: public; Owner: postgres
--

SELECT pg_catalog.setval('public.smart_contracts_id_seq', 1, false);


--
-- Name: transaction_stats_id_seq; Type: SEQUENCE SET; Schema: public; Owner: postgres
--

SELECT pg_catalog.setval('public.transaction_stats_id_seq', 1, false);


--
-- Name: user_contacts_id_seq; Type: SEQUENCE SET; Schema: public; Owner: postgres
--

SELECT pg_catalog.setval('public.user_contacts_id_seq', 1, false);


--
-- Name: users_id_seq; Type: SEQUENCE SET; Schema: public; Owner: postgres
--

SELECT pg_catalog.setval('public.users_id_seq', 1, false);


--
-- Name: address_coin_balances_daily address_coin_balances_daily_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.address_coin_balances_daily
    ADD CONSTRAINT address_coin_balances_daily_pkey PRIMARY KEY (address_hash, day);


--
-- Name: address_coin_balances address_coin_balances_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.address_coin_balances
    ADD CONSTRAINT address_coin_balances_pkey PRIMARY KEY (address_hash, block_number);


--
-- Name: address_current_token_balances address_current_token_balances_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.address_current_token_balances
    ADD CONSTRAINT address_current_token_balances_pkey PRIMARY KEY (id);


--
-- Name: address_names address_names_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.address_names
    ADD CONSTRAINT address_names_pkey PRIMARY KEY (id);


--
-- Name: address_token_balances address_token_balances_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.address_token_balances
    ADD CONSTRAINT address_token_balances_pkey PRIMARY KEY (id);


--
-- Name: addresses addresses_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.addresses
    ADD CONSTRAINT addresses_pkey PRIMARY KEY (hash);


--
-- Name: administrators administrators_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.administrators
    ADD CONSTRAINT administrators_pkey PRIMARY KEY (id);


--
-- Name: block_rewards block_rewards_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.block_rewards
    ADD CONSTRAINT block_rewards_pkey PRIMARY KEY (address_hash, block_hash, address_type);


--
-- Name: block_second_degree_relations block_second_degree_relations_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.block_second_degree_relations
    ADD CONSTRAINT block_second_degree_relations_pkey PRIMARY KEY (nephew_hash, uncle_hash);


--
-- Name: blocks blocks_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.blocks
    ADD CONSTRAINT blocks_pkey PRIMARY KEY (hash);


--
-- Name: internal_transactions call_has_call_type; Type: CHECK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE public.internal_transactions
    ADD CONSTRAINT call_has_call_type CHECK ((((type)::text <> 'call'::text) OR (call_type IS NOT NULL))) NOT VALID;


--
-- Name: internal_transactions call_has_input; Type: CHECK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE public.internal_transactions
    ADD CONSTRAINT call_has_input CHECK ((((type)::text <> 'call'::text) OR (input IS NOT NULL))) NOT VALID;


--
-- Name: contract_methods contract_methods_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.contract_methods
    ADD CONSTRAINT contract_methods_pkey PRIMARY KEY (id);


--
-- Name: contract_verification_status contract_verification_status_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.contract_verification_status
    ADD CONSTRAINT contract_verification_status_pkey PRIMARY KEY (uid);


--
-- Name: internal_transactions create_has_init; Type: CHECK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE public.internal_transactions
    ADD CONSTRAINT create_has_init CHECK ((((type)::text <> 'create'::text) OR (init IS NOT NULL))) NOT VALID;


--
-- Name: decompiled_smart_contracts decompiled_smart_contracts_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.decompiled_smart_contracts
    ADD CONSTRAINT decompiled_smart_contracts_pkey PRIMARY KEY (id);


--
-- Name: emission_rewards emission_rewards_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.emission_rewards
    ADD CONSTRAINT emission_rewards_pkey PRIMARY KEY (block_range);


--
-- Name: event_notifications event_notifications_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.event_notifications
    ADD CONSTRAINT event_notifications_pkey PRIMARY KEY (id);


--
-- Name: internal_transactions internal_transactions_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.internal_transactions
    ADD CONSTRAINT internal_transactions_pkey PRIMARY KEY (block_hash, block_index);


--
-- Name: last_fetched_counters last_fetched_counters_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.last_fetched_counters
    ADD CONSTRAINT last_fetched_counters_pkey PRIMARY KEY (counter_type);


--
-- Name: logs logs_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.logs
    ADD CONSTRAINT logs_pkey PRIMARY KEY (transaction_hash, block_hash, index);


--
-- Name: market_history market_history_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.market_history
    ADD CONSTRAINT market_history_pkey PRIMARY KEY (id);


--
-- Name: emission_rewards no_overlapping_ranges; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.emission_rewards
    ADD CONSTRAINT no_overlapping_ranges EXCLUDE USING gist (block_range WITH &&);


--
-- Name: pending_block_operations pending_block_operations_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.pending_block_operations
    ADD CONSTRAINT pending_block_operations_pkey PRIMARY KEY (block_hash);


--
-- Name: schema_migrations schema_migrations_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.schema_migrations
    ADD CONSTRAINT schema_migrations_pkey PRIMARY KEY (version);


--
-- Name: internal_transactions selfdestruct_has_from_and_to_address; Type: CHECK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE public.internal_transactions
    ADD CONSTRAINT selfdestruct_has_from_and_to_address CHECK ((((type)::text <> 'selfdestruct'::text) OR ((from_address_hash IS NOT NULL) AND (gas IS NULL) AND (to_address_hash IS NOT NULL)))) NOT VALID;


--
-- Name: smart_contracts_additional_sources smart_contracts_additional_sources_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.smart_contracts_additional_sources
    ADD CONSTRAINT smart_contracts_additional_sources_pkey PRIMARY KEY (id);


--
-- Name: smart_contracts smart_contracts_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.smart_contracts
    ADD CONSTRAINT smart_contracts_pkey PRIMARY KEY (id);


--
-- Name: token_instances token_instances_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.token_instances
    ADD CONSTRAINT token_instances_pkey PRIMARY KEY (token_id, token_contract_address_hash);


--
-- Name: token_transfers token_transfers_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.token_transfers
    ADD CONSTRAINT token_transfers_pkey PRIMARY KEY (transaction_hash, block_hash, log_index);


--
-- Name: tokens tokens_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.tokens
    ADD CONSTRAINT tokens_pkey PRIMARY KEY (contract_address_hash);


--
-- Name: transaction_forks transaction_forks_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.transaction_forks
    ADD CONSTRAINT transaction_forks_pkey PRIMARY KEY (uncle_hash, index);


--
-- Name: transaction_stats transaction_stats_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.transaction_stats
    ADD CONSTRAINT transaction_stats_pkey PRIMARY KEY (id);


--
-- Name: transactions transactions_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.transactions
    ADD CONSTRAINT transactions_pkey PRIMARY KEY (hash);


--
-- Name: user_contacts user_contacts_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_contacts
    ADD CONSTRAINT user_contacts_pkey PRIMARY KEY (id);


--
-- Name: users users_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_pkey PRIMARY KEY (id);


--
-- Name: address_coin_balances_value_fetched_at_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX address_coin_balances_value_fetched_at_index ON public.address_coin_balances USING btree (value_fetched_at);


--
-- Name: address_cur_token_balances_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX address_cur_token_balances_index ON public.address_current_token_balances USING btree (block_number);


--
-- Name: address_current_token_balances_address_hash_block_number_token_; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX address_current_token_balances_address_hash_block_number_token_ ON public.address_current_token_balances USING btree (address_hash, block_number, token_contract_address_hash);


--
-- Name: address_current_token_balances_token_contract_address_hash_inde; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX address_current_token_balances_token_contract_address_hash_inde ON public.address_current_token_balances USING btree (token_contract_address_hash) WHERE ((address_hash <> '\x0000000000000000000000000000000000000000'::bytea) AND (value > (0)::numeric));


--
-- Name: address_current_token_balances_token_contract_address_hash_valu; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX address_current_token_balances_token_contract_address_hash_valu ON public.address_current_token_balances USING btree (token_contract_address_hash, value);


--
-- Name: address_current_token_balances_token_id_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX address_current_token_balances_token_id_index ON public.address_current_token_balances USING btree (token_id);


--
-- Name: address_current_token_balances_value; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX address_current_token_balances_value ON public.address_current_token_balances USING btree (value) WHERE (value IS NOT NULL);


--
-- Name: address_decompiler_version; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX address_decompiler_version ON public.decompiled_smart_contracts USING btree (address_hash, decompiler_version);


--
-- Name: address_names_address_hash_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX address_names_address_hash_index ON public.address_names USING btree (address_hash) WHERE ("primary" = true);


--
-- Name: address_token_balances_address_hash_token_contract_address_hash; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX address_token_balances_address_hash_token_contract_address_hash ON public.address_token_balances USING btree (address_hash, token_contract_address_hash, block_number);


--
-- Name: address_token_balances_block_number_address_hash_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX address_token_balances_block_number_address_hash_index ON public.address_token_balances USING btree (block_number, address_hash);


--
-- Name: address_token_balances_token_contract_address_hash_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX address_token_balances_token_contract_address_hash_index ON public.address_token_balances USING btree (token_contract_address_hash);


--
-- Name: address_token_balances_token_id_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX address_token_balances_token_id_index ON public.address_token_balances USING btree (token_id);


--
-- Name: addresses_fetched_coin_balance_hash_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX addresses_fetched_coin_balance_hash_index ON public.addresses USING btree (fetched_coin_balance DESC, hash) WHERE (fetched_coin_balance > (0)::numeric);


--
-- Name: addresses_fetched_coin_balance_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX addresses_fetched_coin_balance_index ON public.addresses USING btree (fetched_coin_balance);


--
-- Name: addresses_inserted_at_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX addresses_inserted_at_index ON public.addresses USING btree (inserted_at);


--
-- Name: administrators_user_id_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX administrators_user_id_index ON public.administrators USING btree (user_id);


--
-- Name: block_rewards_block_hash_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX block_rewards_block_hash_index ON public.block_rewards USING btree (block_hash);


--
-- Name: blocks_consensus_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX blocks_consensus_index ON public.blocks USING btree (consensus);


--
-- Name: blocks_inserted_at_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX blocks_inserted_at_index ON public.blocks USING btree (inserted_at);


--
-- Name: blocks_is_empty_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX blocks_is_empty_index ON public.blocks USING btree (is_empty);


--
-- Name: blocks_miner_hash_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX blocks_miner_hash_index ON public.blocks USING btree (miner_hash);


--
-- Name: blocks_miner_hash_number_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX blocks_miner_hash_number_index ON public.blocks USING btree (miner_hash, number);


--
-- Name: blocks_number_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX blocks_number_index ON public.blocks USING btree (number);


--
-- Name: blocks_timestamp_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX blocks_timestamp_index ON public.blocks USING btree ("timestamp");


--
-- Name: contract_methods_identifier_abi_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX contract_methods_identifier_abi_index ON public.contract_methods USING btree (identifier, abi);


--
-- Name: email_unique_for_user; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX email_unique_for_user ON public.user_contacts USING btree (user_id, email);


--
-- Name: empty_consensus_blocks; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX empty_consensus_blocks ON public.blocks USING btree (consensus) WHERE (is_empty IS NULL);


--
-- Name: fetched_current_token_balances; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX fetched_current_token_balances ON public.address_current_token_balances USING btree (address_hash, token_contract_address_hash) WHERE (token_id IS NULL);


--
-- Name: fetched_current_token_balances_with_token_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX fetched_current_token_balances_with_token_id ON public.address_current_token_balances USING btree (address_hash, token_contract_address_hash, token_id) WHERE (token_id IS NOT NULL);


--
-- Name: fetched_token_balances; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX fetched_token_balances ON public.address_token_balances USING btree (address_hash, token_contract_address_hash, block_number) WHERE (token_id IS NULL);


--
-- Name: fetched_token_balances_with_token_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX fetched_token_balances_with_token_id ON public.address_token_balances USING btree (address_hash, token_contract_address_hash, token_id, block_number) WHERE (token_id IS NOT NULL);


--
-- Name: internal_transactions_block_number_DESC__transaction_index_DESC; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX "internal_transactions_block_number_DESC__transaction_index_DESC" ON public.internal_transactions USING btree (block_number DESC, transaction_index DESC, index DESC);


--
-- Name: internal_transactions_created_contract_address_hash_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX internal_transactions_created_contract_address_hash_index ON public.internal_transactions USING btree (created_contract_address_hash);


--
-- Name: internal_transactions_created_contract_address_hash_partial_ind; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX internal_transactions_created_contract_address_hash_partial_ind ON public.internal_transactions USING btree (created_contract_address_hash, block_number DESC, transaction_index DESC, index DESC) WHERE ((((type)::text = 'call'::text) AND (index > 0)) OR ((type)::text <> 'call'::text));


--
-- Name: internal_transactions_from_address_hash_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX internal_transactions_from_address_hash_index ON public.internal_transactions USING btree (from_address_hash);


--
-- Name: internal_transactions_from_address_hash_partial_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX internal_transactions_from_address_hash_partial_index ON public.internal_transactions USING btree (from_address_hash, block_number DESC, transaction_index DESC, index DESC) WHERE ((((type)::text = 'call'::text) AND (index > 0)) OR ((type)::text <> 'call'::text));


--
-- Name: internal_transactions_to_address_hash_partial_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX internal_transactions_to_address_hash_partial_index ON public.internal_transactions USING btree (to_address_hash, block_number DESC, transaction_index DESC, index DESC) WHERE ((((type)::text = 'call'::text) AND (index > 0)) OR ((type)::text <> 'call'::text));


--
-- Name: internal_transactions_transaction_hash_index_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX internal_transactions_transaction_hash_index_index ON public.internal_transactions USING btree (transaction_hash, index);


--
-- Name: logs_address_hash_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX logs_address_hash_index ON public.logs USING btree (address_hash);


--
-- Name: logs_address_hash_transaction_hash_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX logs_address_hash_transaction_hash_index ON public.logs USING btree (address_hash, transaction_hash);


--
-- Name: logs_block_number_DESC__index_DESC_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX "logs_block_number_DESC__index_DESC_index" ON public.logs USING btree (block_number DESC, index DESC);


--
-- Name: logs_first_topic_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX logs_first_topic_index ON public.logs USING btree (first_topic);


--
-- Name: logs_fourth_topic_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX logs_fourth_topic_index ON public.logs USING btree (fourth_topic);


--
-- Name: logs_index_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX logs_index_index ON public.logs USING btree (index);


--
-- Name: logs_second_topic_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX logs_second_topic_index ON public.logs USING btree (second_topic);


--
-- Name: logs_third_topic_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX logs_third_topic_index ON public.logs USING btree (third_topic);


--
-- Name: logs_transaction_hash_index_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX logs_transaction_hash_index_index ON public.logs USING btree (transaction_hash, index);


--
-- Name: logs_type_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX logs_type_index ON public.logs USING btree (type);


--
-- Name: market_history_date_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX market_history_date_index ON public.market_history USING btree (date);


--
-- Name: nephew_hash_to_uncle_hash; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX nephew_hash_to_uncle_hash ON public.block_second_degree_relations USING btree (nephew_hash, uncle_hash);


--
-- Name: one_consensus_block_at_height; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX one_consensus_block_at_height ON public.blocks USING btree (number) WHERE consensus;


--
-- Name: one_consensus_child_per_parent; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX one_consensus_child_per_parent ON public.blocks USING btree (parent_hash) WHERE consensus;


--
-- Name: one_primary_per_user; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX one_primary_per_user ON public.user_contacts USING btree (user_id) WHERE "primary";


--
-- Name: owner_role_limit; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX owner_role_limit ON public.administrators USING btree (role) WHERE ((role)::text = 'owner'::text);


--
-- Name: pending_block_operations_block_hash_index_partial; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX pending_block_operations_block_hash_index_partial ON public.pending_block_operations USING btree (block_hash) WHERE (fetch_internal_transactions = true);


--
-- Name: pending_txs_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX pending_txs_index ON public.transactions USING btree (inserted_at, hash) WHERE ((block_hash IS NULL) AND ((error IS NULL) OR ((error)::text <> 'dropped/replaced'::text)));


--
-- Name: smart_contracts_additional_sources_address_hash_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX smart_contracts_additional_sources_address_hash_index ON public.smart_contracts_additional_sources USING btree (address_hash);


--
-- Name: smart_contracts_address_hash_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX smart_contracts_address_hash_index ON public.smart_contracts USING btree (address_hash);


--
-- Name: smart_contracts_contract_code_md5_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX smart_contracts_contract_code_md5_index ON public.smart_contracts USING btree (contract_code_md5);


--
-- Name: token_instances_error_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX token_instances_error_index ON public.token_instances USING btree (error);


--
-- Name: token_instances_token_contract_address_hash_token_id_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX token_instances_token_contract_address_hash_token_id_index ON public.token_instances USING btree (token_contract_address_hash, token_id);


--
-- Name: token_instances_token_id_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX token_instances_token_id_index ON public.token_instances USING btree (token_id);


--
-- Name: token_transfers_block_number_DESC_log_index_DESC_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX "token_transfers_block_number_DESC_log_index_DESC_index" ON public.token_transfers USING btree (block_number DESC, log_index DESC);


--
-- Name: token_transfers_block_number_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX token_transfers_block_number_index ON public.token_transfers USING btree (block_number);


--
-- Name: token_transfers_from_address_hash_transaction_hash_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX token_transfers_from_address_hash_transaction_hash_index ON public.token_transfers USING btree (from_address_hash, transaction_hash);


--
-- Name: token_transfers_to_address_hash_transaction_hash_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX token_transfers_to_address_hash_transaction_hash_index ON public.token_transfers USING btree (to_address_hash, transaction_hash);


--
-- Name: token_transfers_token_contract_address_hash_block_number_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX token_transfers_token_contract_address_hash_block_number_index ON public.token_transfers USING btree (token_contract_address_hash, block_number);


--
-- Name: token_transfers_token_contract_address_hash_token_id_DESC_block; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX "token_transfers_token_contract_address_hash_token_id_DESC_block" ON public.token_transfers USING btree (token_contract_address_hash, token_id DESC, block_number DESC);


--
-- Name: token_transfers_token_contract_address_hash_transaction_hash_in; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX token_transfers_token_contract_address_hash_transaction_hash_in ON public.token_transfers USING btree (token_contract_address_hash, transaction_hash);


--
-- Name: token_transfers_token_id_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX token_transfers_token_id_index ON public.token_transfers USING btree (token_id);


--
-- Name: token_transfers_token_ids_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX token_transfers_token_ids_index ON public.token_transfers USING gin (token_ids);


--
-- Name: token_transfers_transaction_hash_log_index_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX token_transfers_transaction_hash_log_index_index ON public.token_transfers USING btree (transaction_hash, log_index);


--
-- Name: tokens_contract_address_hash_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX tokens_contract_address_hash_index ON public.tokens USING btree (contract_address_hash);


--
-- Name: tokens_symbol_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX tokens_symbol_index ON public.tokens USING btree (symbol);


--
-- Name: tokens_trgm_idx; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX tokens_trgm_idx ON public.tokens USING gin (to_tsvector('english'::regconfig, (((symbol)::text || ' '::text) || name)));


--
-- Name: tokens_type_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX tokens_type_index ON public.tokens USING btree (type);


--
-- Name: transaction_stats_date_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX transaction_stats_date_index ON public.transaction_stats USING btree (date);


--
-- Name: transactions_block_hash_error_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX transactions_block_hash_error_index ON public.transactions USING btree (block_hash, error);


--
-- Name: transactions_block_hash_index_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX transactions_block_hash_index_index ON public.transactions USING btree (block_hash, index);


--
-- Name: transactions_block_number_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX transactions_block_number_index ON public.transactions USING btree (block_number);


--
-- Name: transactions_created_contract_address_hash_recent_collated_inde; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX transactions_created_contract_address_hash_recent_collated_inde ON public.transactions USING btree (created_contract_address_hash, block_number DESC, index DESC, hash);


--
-- Name: transactions_created_contract_code_indexed_at_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX transactions_created_contract_code_indexed_at_index ON public.transactions USING btree (created_contract_code_indexed_at);


--
-- Name: transactions_from_address_hash_recent_collated_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX transactions_from_address_hash_recent_collated_index ON public.transactions USING btree (from_address_hash, block_number DESC, index DESC, hash);


--
-- Name: transactions_inserted_at_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX transactions_inserted_at_index ON public.transactions USING btree (inserted_at);


--
-- Name: transactions_nonce_from_address_hash_block_hash_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX transactions_nonce_from_address_hash_block_hash_index ON public.transactions USING btree (nonce, from_address_hash, block_hash);


--
-- Name: transactions_recent_collated_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX transactions_recent_collated_index ON public.transactions USING btree (block_number DESC, index DESC);


--
-- Name: transactions_status_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX transactions_status_index ON public.transactions USING btree (status);


--
-- Name: transactions_to_address_hash_recent_collated_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX transactions_to_address_hash_recent_collated_index ON public.transactions USING btree (to_address_hash, block_number DESC, index DESC, hash);


--
-- Name: transactions_updated_at_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX transactions_updated_at_index ON public.transactions USING btree (updated_at);


--
-- Name: uncle_hash_to_nephew_hash; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX uncle_hash_to_nephew_hash ON public.block_second_degree_relations USING btree (uncle_hash, nephew_hash);


--
-- Name: unfetched_balances; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX unfetched_balances ON public.address_coin_balances USING btree (address_hash, block_number) WHERE (value_fetched_at IS NULL);


--
-- Name: unfetched_token_balances; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX unfetched_token_balances ON public.address_token_balances USING btree (address_hash, token_contract_address_hash, block_number) WHERE ((value_fetched_at IS NULL) AND (token_id IS NULL));


--
-- Name: unfetched_token_balances_with_token_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX unfetched_token_balances_with_token_id ON public.address_token_balances USING btree (address_hash, token_contract_address_hash, token_id, block_number) WHERE ((value_fetched_at IS NULL) AND (token_id IS NOT NULL));


--
-- Name: unfetched_uncles; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX unfetched_uncles ON public.block_second_degree_relations USING btree (nephew_hash, uncle_hash) WHERE (uncle_fetched_at IS NULL);


--
-- Name: unique_address_names; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX unique_address_names ON public.address_names USING btree (address_hash, name);


--
-- Name: unique_username; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX unique_username ON public.users USING btree (username);


--
-- Name: address_coin_balances address_coin_balances_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.address_coin_balances
    ADD CONSTRAINT address_coin_balances_address_hash_fkey FOREIGN KEY (address_hash) REFERENCES public.addresses(hash);


--
-- Name: address_coin_balances_daily address_coin_balances_daily_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.address_coin_balances_daily
    ADD CONSTRAINT address_coin_balances_daily_address_hash_fkey FOREIGN KEY (address_hash) REFERENCES public.addresses(hash);


--
-- Name: address_current_token_balances address_current_token_balances_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.address_current_token_balances
    ADD CONSTRAINT address_current_token_balances_address_hash_fkey FOREIGN KEY (address_hash) REFERENCES public.addresses(hash);


--
-- Name: address_current_token_balances address_current_token_balances_token_contract_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.address_current_token_balances
    ADD CONSTRAINT address_current_token_balances_token_contract_address_hash_fkey FOREIGN KEY (token_contract_address_hash) REFERENCES public.tokens(contract_address_hash);


--
-- Name: address_token_balances address_token_balances_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.address_token_balances
    ADD CONSTRAINT address_token_balances_address_hash_fkey FOREIGN KEY (address_hash) REFERENCES public.addresses(hash);


--
-- Name: address_token_balances address_token_balances_token_contract_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.address_token_balances
    ADD CONSTRAINT address_token_balances_token_contract_address_hash_fkey FOREIGN KEY (token_contract_address_hash) REFERENCES public.tokens(contract_address_hash);


--
-- Name: administrators administrators_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.administrators
    ADD CONSTRAINT administrators_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: block_rewards block_rewards_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.block_rewards
    ADD CONSTRAINT block_rewards_address_hash_fkey FOREIGN KEY (address_hash) REFERENCES public.addresses(hash) ON DELETE CASCADE;


--
-- Name: block_rewards block_rewards_block_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.block_rewards
    ADD CONSTRAINT block_rewards_block_hash_fkey FOREIGN KEY (block_hash) REFERENCES public.blocks(hash) ON DELETE CASCADE;


--
-- Name: block_second_degree_relations block_second_degree_relations_nephew_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.block_second_degree_relations
    ADD CONSTRAINT block_second_degree_relations_nephew_hash_fkey FOREIGN KEY (nephew_hash) REFERENCES public.blocks(hash);


--
-- Name: blocks blocks_miner_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.blocks
    ADD CONSTRAINT blocks_miner_hash_fkey FOREIGN KEY (miner_hash) REFERENCES public.addresses(hash);


--
-- Name: decompiled_smart_contracts decompiled_smart_contracts_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.decompiled_smart_contracts
    ADD CONSTRAINT decompiled_smart_contracts_address_hash_fkey FOREIGN KEY (address_hash) REFERENCES public.addresses(hash) ON DELETE CASCADE;


--
-- Name: internal_transactions internal_transactions_block_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.internal_transactions
    ADD CONSTRAINT internal_transactions_block_hash_fkey FOREIGN KEY (block_hash) REFERENCES public.blocks(hash);


--
-- Name: internal_transactions internal_transactions_created_contract_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.internal_transactions
    ADD CONSTRAINT internal_transactions_created_contract_address_hash_fkey FOREIGN KEY (created_contract_address_hash) REFERENCES public.addresses(hash);


--
-- Name: internal_transactions internal_transactions_from_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.internal_transactions
    ADD CONSTRAINT internal_transactions_from_address_hash_fkey FOREIGN KEY (from_address_hash) REFERENCES public.addresses(hash);


--
-- Name: internal_transactions internal_transactions_to_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.internal_transactions
    ADD CONSTRAINT internal_transactions_to_address_hash_fkey FOREIGN KEY (to_address_hash) REFERENCES public.addresses(hash);


--
-- Name: internal_transactions internal_transactions_transaction_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.internal_transactions
    ADD CONSTRAINT internal_transactions_transaction_hash_fkey FOREIGN KEY (transaction_hash) REFERENCES public.transactions(hash) ON DELETE CASCADE;


--
-- Name: logs logs_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.logs
    ADD CONSTRAINT logs_address_hash_fkey FOREIGN KEY (address_hash) REFERENCES public.addresses(hash) ON DELETE CASCADE;


--
-- Name: logs logs_block_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.logs
    ADD CONSTRAINT logs_block_hash_fkey FOREIGN KEY (block_hash) REFERENCES public.blocks(hash);


--
-- Name: logs logs_transaction_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.logs
    ADD CONSTRAINT logs_transaction_hash_fkey FOREIGN KEY (transaction_hash) REFERENCES public.transactions(hash) ON DELETE CASCADE;


--
-- Name: pending_block_operations pending_block_operations_block_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.pending_block_operations
    ADD CONSTRAINT pending_block_operations_block_hash_fkey FOREIGN KEY (block_hash) REFERENCES public.blocks(hash) ON DELETE CASCADE;


--
-- Name: smart_contracts_additional_sources smart_contracts_additional_sources_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.smart_contracts_additional_sources
    ADD CONSTRAINT smart_contracts_additional_sources_address_hash_fkey FOREIGN KEY (address_hash) REFERENCES public.smart_contracts(address_hash) ON DELETE CASCADE;


--
-- Name: smart_contracts smart_contracts_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.smart_contracts
    ADD CONSTRAINT smart_contracts_address_hash_fkey FOREIGN KEY (address_hash) REFERENCES public.addresses(hash) ON DELETE CASCADE;


--
-- Name: token_instances token_instances_token_contract_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.token_instances
    ADD CONSTRAINT token_instances_token_contract_address_hash_fkey FOREIGN KEY (token_contract_address_hash) REFERENCES public.tokens(contract_address_hash);


--
-- Name: token_transfers token_transfers_block_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.token_transfers
    ADD CONSTRAINT token_transfers_block_hash_fkey FOREIGN KEY (block_hash) REFERENCES public.blocks(hash);


--
-- Name: token_transfers token_transfers_from_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.token_transfers
    ADD CONSTRAINT token_transfers_from_address_hash_fkey FOREIGN KEY (from_address_hash) REFERENCES public.addresses(hash);


--
-- Name: token_transfers token_transfers_to_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.token_transfers
    ADD CONSTRAINT token_transfers_to_address_hash_fkey FOREIGN KEY (to_address_hash) REFERENCES public.addresses(hash);


--
-- Name: token_transfers token_transfers_token_contract_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.token_transfers
    ADD CONSTRAINT token_transfers_token_contract_address_hash_fkey FOREIGN KEY (token_contract_address_hash) REFERENCES public.addresses(hash);


--
-- Name: token_transfers token_transfers_transaction_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.token_transfers
    ADD CONSTRAINT token_transfers_transaction_hash_fkey FOREIGN KEY (transaction_hash) REFERENCES public.transactions(hash) ON DELETE CASCADE;


--
-- Name: tokens tokens_contract_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.tokens
    ADD CONSTRAINT tokens_contract_address_hash_fkey FOREIGN KEY (contract_address_hash) REFERENCES public.addresses(hash) ON DELETE CASCADE;


--
-- Name: transaction_forks transaction_forks_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.transaction_forks
    ADD CONSTRAINT transaction_forks_hash_fkey FOREIGN KEY (hash) REFERENCES public.transactions(hash) ON DELETE CASCADE;


--
-- Name: transaction_forks transaction_forks_uncle_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.transaction_forks
    ADD CONSTRAINT transaction_forks_uncle_hash_fkey FOREIGN KEY (uncle_hash) REFERENCES public.blocks(hash) ON DELETE CASCADE;


--
-- Name: transactions transactions_block_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.transactions
    ADD CONSTRAINT transactions_block_hash_fkey FOREIGN KEY (block_hash) REFERENCES public.blocks(hash) ON DELETE CASCADE;


--
-- Name: transactions transactions_created_contract_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.transactions
    ADD CONSTRAINT transactions_created_contract_address_hash_fkey FOREIGN KEY (created_contract_address_hash) REFERENCES public.addresses(hash);


--
-- Name: transactions transactions_from_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.transactions
    ADD CONSTRAINT transactions_from_address_hash_fkey FOREIGN KEY (from_address_hash) REFERENCES public.addresses(hash) ON DELETE CASCADE;


--
-- Name: transactions transactions_to_address_hash_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.transactions
    ADD CONSTRAINT transactions_to_address_hash_fkey FOREIGN KEY (to_address_hash) REFERENCES public.addresses(hash) ON DELETE CASCADE;


--
-- Name: user_contacts user_contacts_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_contacts
    ADD CONSTRAINT user_contacts_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- PostgreSQL database dump complete
--

