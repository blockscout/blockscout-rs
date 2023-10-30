--
-- PostgreSQL database dump
--

-- Dumped from database version 14.9 (Debian 14.9-1.pgdg120+1)
-- Dumped by pg_dump version 14.9 (Debian 14.9-1.pgdg120+1)

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: deployment_schemas; Type: TABLE; Schema: public; Owner: graph-node
--
CREATE EXTENSION btree_gist;

CREATE TABLE public.deployment_schemas (
    id integer NOT NULL,
    subgraph character varying NOT NULL,
    name character varying NOT NULL,
    version integer NOT NULL,
    shard text NOT NULL,
    network text NOT NULL,
    active boolean NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.deployment_schemas OWNER TO "graph-node";

--
-- Name: active_copies; Type: TABLE; Schema: public; Owner: graph-node
--

CREATE TABLE public.active_copies (
    src integer NOT NULL,
    dst integer NOT NULL,
    queued_at timestamp with time zone NOT NULL,
    cancelled_at timestamp with time zone
);


ALTER TABLE public.active_copies OWNER TO "graph-node";

--
-- Name: chains; Type: TABLE; Schema: public; Owner: graph-node
--

CREATE TABLE public.chains (
    id integer NOT NULL,
    name text NOT NULL,
    net_version text NOT NULL,
    genesis_block_hash text NOT NULL,
    shard text NOT NULL,
    namespace text NOT NULL,
    CONSTRAINT chains_genesis_version_check CHECK (((net_version IS NULL) = (genesis_block_hash IS NULL)))
);


ALTER TABLE public.chains OWNER TO "graph-node";

--
-- Name: __diesel_schema_migrations; Type: TABLE; Schema: public; Owner: graph-node
--

CREATE TABLE public.__diesel_schema_migrations (
    version character varying(50) NOT NULL,
    run_on timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL
);


ALTER TABLE public.__diesel_schema_migrations OWNER TO "graph-node";

--
-- Name: chains_id_seq; Type: SEQUENCE; Schema: public; Owner: graph-node
--

CREATE SEQUENCE public.chains_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.chains_id_seq OWNER TO "graph-node";

--
-- Name: chains_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: graph-node
--

ALTER SEQUENCE public.chains_id_seq OWNED BY public.chains.id;


--
-- Name: db_version; Type: TABLE; Schema: public; Owner: graph-node
--

CREATE TABLE public.db_version (
    db_version bigint NOT NULL
);


ALTER TABLE public.db_version OWNER TO "graph-node";

--
-- Name: deployment_schemas_id_seq; Type: SEQUENCE; Schema: public; Owner: graph-node
--

CREATE SEQUENCE public.deployment_schemas_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.deployment_schemas_id_seq OWNER TO "graph-node";

--
-- Name: deployment_schemas_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: graph-node
--

ALTER SEQUENCE public.deployment_schemas_id_seq OWNED BY public.deployment_schemas.id;


--
-- Name: ens_names; Type: TABLE; Schema: public; Owner: graph-node
--

CREATE TABLE public.ens_names (
    hash character varying NOT NULL,
    name character varying NOT NULL
);


ALTER TABLE public.ens_names OWNER TO "graph-node";

--
-- Name: eth_call_cache; Type: TABLE; Schema: public; Owner: graph-node
--

CREATE TABLE public.eth_call_cache (
    id bytea NOT NULL,
    return_value bytea NOT NULL,
    contract_address bytea NOT NULL,
    block_number integer NOT NULL
);


ALTER TABLE public.eth_call_cache OWNER TO "graph-node";

--
-- Name: eth_call_meta; Type: TABLE; Schema: public; Owner: graph-node
--

CREATE TABLE public.eth_call_meta (
    contract_address bytea NOT NULL,
    accessed_at date NOT NULL
);


ALTER TABLE public.eth_call_meta OWNER TO "graph-node";

--
-- Name: ethereum_blocks; Type: TABLE; Schema: public; Owner: graph-node
--

CREATE TABLE public.ethereum_blocks (
    hash character varying NOT NULL,
    number bigint NOT NULL,
    parent_hash character varying NOT NULL,
    network_name character varying NOT NULL,
    data jsonb NOT NULL
);


ALTER TABLE public.ethereum_blocks OWNER TO "graph-node";

--
-- Name: ethereum_networks; Type: TABLE; Schema: public; Owner: graph-node
--

CREATE TABLE public.ethereum_networks (
    name character varying NOT NULL,
    head_block_hash character varying,
    head_block_number bigint,
    net_version character varying NOT NULL,
    genesis_block_hash character varying NOT NULL,
    namespace text NOT NULL,
    head_block_cursor text,
    CONSTRAINT ethereum_networks_check CHECK (((head_block_hash IS NULL) = (head_block_number IS NULL))),
    CONSTRAINT ethereum_networks_check1 CHECK (((net_version IS NULL) = (genesis_block_hash IS NULL)))
);


ALTER TABLE public.ethereum_networks OWNER TO "graph-node";

--
-- Name: event_meta_data; Type: TABLE; Schema: public; Owner: graph-node
--

CREATE TABLE public.event_meta_data (
    id integer NOT NULL,
    db_transaction_id bigint NOT NULL,
    db_transaction_time timestamp without time zone NOT NULL,
    source character varying
);


ALTER TABLE public.event_meta_data OWNER TO "graph-node";

--
-- Name: event_meta_data_id_seq; Type: SEQUENCE; Schema: public; Owner: graph-node
--

CREATE SEQUENCE public.event_meta_data_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.event_meta_data_id_seq OWNER TO "graph-node";

--
-- Name: event_meta_data_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: graph-node
--

ALTER SEQUENCE public.event_meta_data_id_seq OWNED BY public.event_meta_data.id;


--
-- Name: large_notifications; Type: TABLE; Schema: public; Owner: graph-node
--

CREATE UNLOGGED TABLE public.large_notifications (
    id integer NOT NULL,
    payload character varying NOT NULL,
    created_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL
);


ALTER TABLE public.large_notifications OWNER TO "graph-node";

--
-- Name: TABLE large_notifications; Type: COMMENT; Schema: public; Owner: graph-node
--

COMMENT ON TABLE public.large_notifications IS 'Table for notifications whose payload is too big to send directly';


--
-- Name: large_notifications_id_seq; Type: SEQUENCE; Schema: public; Owner: graph-node
--

CREATE SEQUENCE public.large_notifications_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.large_notifications_id_seq OWNER TO "graph-node";

--
-- Name: large_notifications_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: graph-node
--

ALTER SEQUENCE public.large_notifications_id_seq OWNED BY public.large_notifications.id;


--
-- Name: unneeded_event_ids; Type: TABLE; Schema: public; Owner: graph-node
--

CREATE TABLE public.unneeded_event_ids (
    event_id bigint NOT NULL
);


ALTER TABLE public.unneeded_event_ids OWNER TO "graph-node";

--
-- Name: unused_deployments; Type: TABLE; Schema: public; Owner: graph-node
--

CREATE TABLE public.unused_deployments (
    deployment text NOT NULL,
    unused_at timestamp with time zone DEFAULT now() NOT NULL,
    removed_at timestamp with time zone,
    subgraphs text[],
    namespace text NOT NULL,
    shard text NOT NULL,
    entity_count integer DEFAULT 0 NOT NULL,
    latest_ethereum_block_hash bytea,
    latest_ethereum_block_number integer,
    failed boolean DEFAULT false NOT NULL,
    synced boolean DEFAULT false NOT NULL,
    id integer NOT NULL,
    created_at timestamp with time zone NOT NULL
);


ALTER TABLE public.unused_deployments OWNER TO "graph-node";

--
-- Name: chains id; Type: DEFAULT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.chains ALTER COLUMN id SET DEFAULT nextval('public.chains_id_seq'::regclass);


--
-- Name: chains namespace; Type: DEFAULT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.chains ALTER COLUMN namespace SET DEFAULT ('chain'::text || currval('public.chains_id_seq'::regclass));


--
-- Name: deployment_schemas id; Type: DEFAULT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.deployment_schemas ALTER COLUMN id SET DEFAULT nextval('public.deployment_schemas_id_seq'::regclass);


--
-- Name: deployment_schemas name; Type: DEFAULT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.deployment_schemas ALTER COLUMN name SET DEFAULT ('sgd'::text || currval('public.deployment_schemas_id_seq'::regclass));


--
-- Name: event_meta_data id; Type: DEFAULT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.event_meta_data ALTER COLUMN id SET DEFAULT nextval('public.event_meta_data_id_seq'::regclass);


--
-- Name: large_notifications id; Type: DEFAULT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.large_notifications ALTER COLUMN id SET DEFAULT nextval('public.large_notifications_id_seq'::regclass);


--
-- Name: __diesel_schema_migrations __diesel_schema_migrations_pkey; Type: CONSTRAINT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.__diesel_schema_migrations
    ADD CONSTRAINT __diesel_schema_migrations_pkey PRIMARY KEY (version);


--
-- Name: active_copies active_copies_pkey; Type: CONSTRAINT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.active_copies
    ADD CONSTRAINT active_copies_pkey PRIMARY KEY (dst);


--
-- Name: active_copies active_copies_src_dst_key; Type: CONSTRAINT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.active_copies
    ADD CONSTRAINT active_copies_src_dst_key UNIQUE (src, dst);


--
-- Name: chains chains_name_key; Type: CONSTRAINT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.chains
    ADD CONSTRAINT chains_name_key UNIQUE (name);


--
-- Name: chains chains_pkey; Type: CONSTRAINT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.chains
    ADD CONSTRAINT chains_pkey PRIMARY KEY (id);


--
-- Name: db_version db_version_pkey; Type: CONSTRAINT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.db_version
    ADD CONSTRAINT db_version_pkey PRIMARY KEY (db_version);


--
-- Name: deployment_schemas deployment_schemas_pkey; Type: CONSTRAINT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.deployment_schemas
    ADD CONSTRAINT deployment_schemas_pkey PRIMARY KEY (id);


--
-- Name: ens_names ens_names_pkey; Type: CONSTRAINT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.ens_names
    ADD CONSTRAINT ens_names_pkey PRIMARY KEY (hash);


--
-- Name: eth_call_cache eth_call_cache_pkey; Type: CONSTRAINT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.eth_call_cache
    ADD CONSTRAINT eth_call_cache_pkey PRIMARY KEY (id);


--
-- Name: eth_call_meta eth_call_meta_pkey; Type: CONSTRAINT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.eth_call_meta
    ADD CONSTRAINT eth_call_meta_pkey PRIMARY KEY (contract_address);


--
-- Name: ethereum_blocks ethereum_blocks_pkey; Type: CONSTRAINT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.ethereum_blocks
    ADD CONSTRAINT ethereum_blocks_pkey PRIMARY KEY (hash);


--
-- Name: ethereum_networks ethereum_networks_pkey; Type: CONSTRAINT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.ethereum_networks
    ADD CONSTRAINT ethereum_networks_pkey PRIMARY KEY (name);


--
-- Name: event_meta_data event_meta_data_db_transaction_id_key; Type: CONSTRAINT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.event_meta_data
    ADD CONSTRAINT event_meta_data_db_transaction_id_key UNIQUE (db_transaction_id);


--
-- Name: event_meta_data event_meta_data_pkey; Type: CONSTRAINT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.event_meta_data
    ADD CONSTRAINT event_meta_data_pkey PRIMARY KEY (id);


--
-- Name: large_notifications large_notifications_pkey; Type: CONSTRAINT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.large_notifications
    ADD CONSTRAINT large_notifications_pkey PRIMARY KEY (id);


--
-- Name: unneeded_event_ids unneeded_event_ids_pkey; Type: CONSTRAINT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.unneeded_event_ids
    ADD CONSTRAINT unneeded_event_ids_pkey PRIMARY KEY (event_id);


--
-- Name: unused_deployments unused_deployments_pkey; Type: CONSTRAINT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.unused_deployments
    ADD CONSTRAINT unused_deployments_pkey PRIMARY KEY (id);


--
-- Name: deployment_schemas_deployment_active; Type: INDEX; Schema: public; Owner: graph-node
--

CREATE UNIQUE INDEX deployment_schemas_deployment_active ON public.deployment_schemas USING btree (subgraph) WHERE active;


--
-- Name: deployment_schemas_subgraph_shard_uq; Type: INDEX; Schema: public; Owner: graph-node
--

CREATE UNIQUE INDEX deployment_schemas_subgraph_shard_uq ON public.deployment_schemas USING btree (subgraph, shard);


--
-- Name: eth_call_cache_block_number_idx; Type: INDEX; Schema: public; Owner: graph-node
--

CREATE INDEX eth_call_cache_block_number_idx ON public.eth_call_cache USING btree (block_number);


--
-- Name: ethereum_blocks_name_number; Type: INDEX; Schema: public; Owner: graph-node
--

CREATE INDEX ethereum_blocks_name_number ON public.ethereum_blocks USING btree (network_name, number);


--
-- Name: event_meta_data_source; Type: INDEX; Schema: public; Owner: graph-node
--

CREATE INDEX event_meta_data_source ON public.event_meta_data USING btree (source);


--
-- Name: active_copies active_copies_dst_fkey; Type: FK CONSTRAINT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.active_copies
    ADD CONSTRAINT active_copies_dst_fkey FOREIGN KEY (dst) REFERENCES public.deployment_schemas(id) ON DELETE CASCADE;


--
-- Name: active_copies active_copies_src_fkey; Type: FK CONSTRAINT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.active_copies
    ADD CONSTRAINT active_copies_src_fkey FOREIGN KEY (src) REFERENCES public.deployment_schemas(id);


--
-- Name: deployment_schemas deployment_schemas_network_fkey; Type: FK CONSTRAINT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.deployment_schemas
    ADD CONSTRAINT deployment_schemas_network_fkey FOREIGN KEY (network) REFERENCES public.chains(name);


--
-- Name: ethereum_blocks ethereum_blocks_network_name_fkey; Type: FK CONSTRAINT; Schema: public; Owner: graph-node
--

ALTER TABLE ONLY public.ethereum_blocks
    ADD CONSTRAINT ethereum_blocks_network_name_fkey FOREIGN KEY (network_name) REFERENCES public.ethereum_networks(name);


--
-- PostgreSQL database dump complete
--

