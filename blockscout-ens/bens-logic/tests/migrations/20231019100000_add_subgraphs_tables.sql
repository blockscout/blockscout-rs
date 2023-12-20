--
-- PostgreSQL database dump
--

-- Dumped from database version 14.6 (Ubuntu 14.6-1.pgdg22.04+1)
-- Dumped by pg_dump version 14.10 (Homebrew)

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: subgraphs; Type: SCHEMA; Schema: -; Owner: graph
--

CREATE SCHEMA subgraphs;


ALTER SCHEMA subgraphs OWNER TO "graph-node";

--
-- Name: health; Type: TYPE; Schema: subgraphs; Owner: graph
--

CREATE TYPE subgraphs.health AS ENUM (
    'failed',
    'healthy',
    'unhealthy'
);


ALTER TYPE subgraphs.health OWNER TO "graph-node";

SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: subgraph; Type: TABLE; Schema: subgraphs; Owner: graph
--

CREATE TABLE subgraphs.subgraph (
    id text NOT NULL,
    name text NOT NULL,
    current_version text,
    pending_version text,
    created_at numeric NOT NULL,
    vid bigint NOT NULL,
    block_range int4range NOT NULL
);


ALTER TABLE subgraphs.subgraph OWNER TO "graph-node";

--
-- Name: subgraph_deployment; Type: TABLE; Schema: subgraphs; Owner: graph
--

CREATE TABLE subgraphs.subgraph_deployment (
    deployment text NOT NULL,
    failed boolean NOT NULL,
    synced boolean NOT NULL,
    latest_ethereum_block_hash bytea,
    latest_ethereum_block_number numeric,
    entity_count numeric NOT NULL,
    graft_base text,
    graft_block_hash bytea,
    graft_block_number numeric,
    fatal_error text,
    non_fatal_errors text[] DEFAULT '{}'::text[],
    health subgraphs.health NOT NULL,
    reorg_count integer DEFAULT 0 NOT NULL,
    current_reorg_depth integer DEFAULT 0 NOT NULL,
    max_reorg_depth integer DEFAULT 0 NOT NULL,
    last_healthy_ethereum_block_hash bytea,
    last_healthy_ethereum_block_number numeric,
    id integer NOT NULL,
    firehose_cursor text,
    debug_fork text,
    earliest_block_number integer DEFAULT 0 NOT NULL
);


ALTER TABLE subgraphs.subgraph_deployment OWNER TO "graph-node";

--
-- Name: subgraph_version; Type: TABLE; Schema: subgraphs; Owner: graph
--

CREATE TABLE subgraphs.subgraph_version (
    id text NOT NULL,
    subgraph text NOT NULL,
    deployment text NOT NULL,
    created_at numeric NOT NULL,
    vid bigint NOT NULL,
    block_range int4range NOT NULL
);


ALTER TABLE subgraphs.subgraph_version OWNER TO "graph-node";

--
-- Name: copy_state; Type: TABLE; Schema: subgraphs; Owner: graph
--

CREATE TABLE subgraphs.copy_state (
    src integer NOT NULL,
    dst integer NOT NULL,
    target_block_hash bytea NOT NULL,
    target_block_number integer NOT NULL,
    started_at timestamp with time zone DEFAULT now() NOT NULL,
    finished_at timestamp with time zone,
    cancelled_at timestamp with time zone
);


ALTER TABLE subgraphs.copy_state OWNER TO "graph-node";

--
-- Name: copy_table_state; Type: TABLE; Schema: subgraphs; Owner: graph
--

CREATE TABLE subgraphs.copy_table_state (
    id integer NOT NULL,
    entity_type text NOT NULL,
    dst integer NOT NULL,
    next_vid bigint NOT NULL,
    target_vid bigint NOT NULL,
    batch_size bigint NOT NULL,
    started_at timestamp with time zone DEFAULT now() NOT NULL,
    finished_at timestamp with time zone,
    duration_ms bigint DEFAULT 0 NOT NULL
);


ALTER TABLE subgraphs.copy_table_state OWNER TO "graph-node";

--
-- Name: copy_table_state_id_seq; Type: SEQUENCE; Schema: subgraphs; Owner: graph
--

CREATE SEQUENCE subgraphs.copy_table_state_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE subgraphs.copy_table_state_id_seq OWNER TO "graph-node";

--
-- Name: copy_table_state_id_seq; Type: SEQUENCE OWNED BY; Schema: subgraphs; Owner: graph
--

ALTER SEQUENCE subgraphs.copy_table_state_id_seq OWNED BY subgraphs.copy_table_state.id;


--
-- Name: dynamic_ethereum_contract_data_source; Type: TABLE; Schema: subgraphs; Owner: graph
--

CREATE TABLE subgraphs.dynamic_ethereum_contract_data_source (
    name text NOT NULL,
    ethereum_block_hash bytea NOT NULL,
    ethereum_block_number numeric NOT NULL,
    deployment text NOT NULL,
    vid bigint NOT NULL,
    context text,
    address bytea NOT NULL,
    abi text NOT NULL,
    start_block integer NOT NULL
);


ALTER TABLE subgraphs.dynamic_ethereum_contract_data_source OWNER TO "graph-node";

--
-- Name: dynamic_ethereum_contract_data_source_vid_seq; Type: SEQUENCE; Schema: subgraphs; Owner: graph
--

CREATE SEQUENCE subgraphs.dynamic_ethereum_contract_data_source_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE subgraphs.dynamic_ethereum_contract_data_source_vid_seq OWNER TO "graph-node";

--
-- Name: dynamic_ethereum_contract_data_source_vid_seq; Type: SEQUENCE OWNED BY; Schema: subgraphs; Owner: graph
--

ALTER SEQUENCE subgraphs.dynamic_ethereum_contract_data_source_vid_seq OWNED BY subgraphs.dynamic_ethereum_contract_data_source.vid;


--
-- Name: graph_node_versions; Type: TABLE; Schema: subgraphs; Owner: graph
--

CREATE TABLE subgraphs.graph_node_versions (
    id integer NOT NULL,
    git_commit_hash text NOT NULL,
    git_repository_dirty boolean NOT NULL,
    crate_version text NOT NULL,
    major integer NOT NULL,
    minor integer NOT NULL,
    patch integer NOT NULL
);


ALTER TABLE subgraphs.graph_node_versions OWNER TO "graph-node";

--
-- Name: graph_node_versions_id_seq; Type: SEQUENCE; Schema: subgraphs; Owner: graph
--

CREATE SEQUENCE subgraphs.graph_node_versions_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE subgraphs.graph_node_versions_id_seq OWNER TO "graph-node";

--
-- Name: graph_node_versions_id_seq; Type: SEQUENCE OWNED BY; Schema: subgraphs; Owner: graph
--

ALTER SEQUENCE subgraphs.graph_node_versions_id_seq OWNED BY subgraphs.graph_node_versions.id;


--
-- Name: subgraph_deployment_assignment; Type: TABLE; Schema: subgraphs; Owner: graph
--

CREATE TABLE subgraphs.subgraph_deployment_assignment (
    node_id text NOT NULL,
    id integer NOT NULL,
    paused_at timestamp with time zone,
    assigned_at timestamp with time zone
);


ALTER TABLE subgraphs.subgraph_deployment_assignment OWNER TO "graph-node";

--
-- Name: subgraph_error; Type: TABLE; Schema: subgraphs; Owner: graph
--

CREATE TABLE subgraphs.subgraph_error (
    id text NOT NULL,
    subgraph_id text NOT NULL,
    message text NOT NULL,
    block_hash bytea,
    handler text,
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    deterministic boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE subgraphs.subgraph_error OWNER TO "graph-node";

--
-- Name: subgraph_error_vid_seq; Type: SEQUENCE; Schema: subgraphs; Owner: graph
--

CREATE SEQUENCE subgraphs.subgraph_error_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE subgraphs.subgraph_error_vid_seq OWNER TO "graph-node";

--
-- Name: subgraph_error_vid_seq; Type: SEQUENCE OWNED BY; Schema: subgraphs; Owner: graph
--

ALTER SEQUENCE subgraphs.subgraph_error_vid_seq OWNED BY subgraphs.subgraph_error.vid;


--
-- Name: subgraph_features; Type: TABLE; Schema: subgraphs; Owner: graph
--

CREATE TABLE subgraphs.subgraph_features (
    id text NOT NULL,
    spec_version text NOT NULL,
    api_version text,
    features text[] DEFAULT '{}'::text[] NOT NULL,
    data_sources text[] DEFAULT '{}'::text[] NOT NULL,
    network text NOT NULL,
    handlers text[] DEFAULT '{}'::text[] NOT NULL
);


ALTER TABLE subgraphs.subgraph_features OWNER TO "graph-node";

--
-- Name: subgraph_manifest; Type: TABLE; Schema: subgraphs; Owner: graph
--

CREATE TABLE subgraphs.subgraph_manifest (
    spec_version text NOT NULL,
    description text,
    repository text,
    schema text NOT NULL,
    features text[] DEFAULT '{}'::text[] NOT NULL,
    id integer NOT NULL,
    graph_node_version_id integer,
    use_bytea_prefix boolean DEFAULT false NOT NULL,
    start_block_number integer,
    start_block_hash bytea,
    raw_yaml text,
    entities_with_causality_region text[] DEFAULT ARRAY[]::text[] NOT NULL,
    on_sync text,
    history_blocks integer DEFAULT 2147483647 NOT NULL,
    CONSTRAINT subgraph_manifest_history_blocks_check CHECK ((history_blocks > 0)),
    CONSTRAINT subgraph_manifest_on_sync_ck CHECK ((on_sync = ANY (ARRAY['activate'::text, 'replace'::text])))
);


ALTER TABLE subgraphs.subgraph_manifest OWNER TO "graph-node";

--
-- Name: subgraph_version_vid_seq; Type: SEQUENCE; Schema: subgraphs; Owner: graph
--

CREATE SEQUENCE subgraphs.subgraph_version_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE subgraphs.subgraph_version_vid_seq OWNER TO "graph-node";

--
-- Name: subgraph_version_vid_seq; Type: SEQUENCE OWNED BY; Schema: subgraphs; Owner: graph
--

ALTER SEQUENCE subgraphs.subgraph_version_vid_seq OWNED BY subgraphs.subgraph_version.vid;


--
-- Name: subgraph_vid_seq; Type: SEQUENCE; Schema: subgraphs; Owner: graph
--

CREATE SEQUENCE subgraphs.subgraph_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE subgraphs.subgraph_vid_seq OWNER TO "graph-node";

--
-- Name: subgraph_vid_seq; Type: SEQUENCE OWNED BY; Schema: subgraphs; Owner: graph
--

ALTER SEQUENCE subgraphs.subgraph_vid_seq OWNED BY subgraphs.subgraph.vid;


--
-- Name: table_stats; Type: TABLE; Schema: subgraphs; Owner: graph
--

CREATE TABLE subgraphs.table_stats (
    id integer NOT NULL,
    deployment integer NOT NULL,
    table_name text NOT NULL,
    is_account_like boolean,
    last_pruned_block integer
);


ALTER TABLE subgraphs.table_stats OWNER TO "graph-node";

--
-- Name: table_stats_id_seq; Type: SEQUENCE; Schema: subgraphs; Owner: graph
--

CREATE SEQUENCE subgraphs.table_stats_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE subgraphs.table_stats_id_seq OWNER TO "graph-node";

--
-- Name: table_stats_id_seq; Type: SEQUENCE OWNED BY; Schema: subgraphs; Owner: graph
--

ALTER SEQUENCE subgraphs.table_stats_id_seq OWNED BY subgraphs.table_stats.id;


--
-- Name: copy_table_state id; Type: DEFAULT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.copy_table_state ALTER COLUMN id SET DEFAULT nextval('subgraphs.copy_table_state_id_seq'::regclass);


--
-- Name: dynamic_ethereum_contract_data_source vid; Type: DEFAULT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.dynamic_ethereum_contract_data_source ALTER COLUMN vid SET DEFAULT nextval('subgraphs.dynamic_ethereum_contract_data_source_vid_seq'::regclass);


--
-- Name: graph_node_versions id; Type: DEFAULT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.graph_node_versions ALTER COLUMN id SET DEFAULT nextval('subgraphs.graph_node_versions_id_seq'::regclass);


--
-- Name: subgraph vid; Type: DEFAULT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.subgraph ALTER COLUMN vid SET DEFAULT nextval('subgraphs.subgraph_vid_seq'::regclass);


--
-- Name: subgraph_error vid; Type: DEFAULT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.subgraph_error ALTER COLUMN vid SET DEFAULT nextval('subgraphs.subgraph_error_vid_seq'::regclass);


--
-- Name: subgraph_version vid; Type: DEFAULT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.subgraph_version ALTER COLUMN vid SET DEFAULT nextval('subgraphs.subgraph_version_vid_seq'::regclass);


--
-- Name: table_stats id; Type: DEFAULT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.table_stats ALTER COLUMN id SET DEFAULT nextval('subgraphs.table_stats_id_seq'::regclass);


--
-- Name: copy_state copy_state_pkey; Type: CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.copy_state
    ADD CONSTRAINT copy_state_pkey PRIMARY KEY (dst);


--
-- Name: copy_table_state copy_table_state_dst_entity_type_key; Type: CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.copy_table_state
    ADD CONSTRAINT copy_table_state_dst_entity_type_key UNIQUE (dst, entity_type);


--
-- Name: copy_table_state copy_table_state_pkey; Type: CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.copy_table_state
    ADD CONSTRAINT copy_table_state_pkey PRIMARY KEY (id);


--
-- Name: dynamic_ethereum_contract_data_source dynamic_ethereum_contract_data_source_pkey; Type: CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.dynamic_ethereum_contract_data_source
    ADD CONSTRAINT dynamic_ethereum_contract_data_source_pkey PRIMARY KEY (vid);


--
-- Name: graph_node_versions graph_node_versions_pkey; Type: CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.graph_node_versions
    ADD CONSTRAINT graph_node_versions_pkey PRIMARY KEY (id);


--
-- Name: subgraph_deployment_assignment subgraph_deployment_assignment_pkey; Type: CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.subgraph_deployment_assignment
    ADD CONSTRAINT subgraph_deployment_assignment_pkey PRIMARY KEY (id);


--
-- Name: subgraph_deployment subgraph_deployment_id_key; Type: CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.subgraph_deployment
    ADD CONSTRAINT subgraph_deployment_id_key UNIQUE (deployment);


--
-- Name: subgraph_deployment subgraph_deployment_pkey; Type: CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.subgraph_deployment
    ADD CONSTRAINT subgraph_deployment_pkey PRIMARY KEY (id);


--
-- Name: subgraph_error subgraph_error_id_block_range_excl; Type: CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.subgraph_error
    ADD CONSTRAINT subgraph_error_id_block_range_excl EXCLUDE USING gist (id WITH =, block_range WITH &&);


--
-- Name: subgraph_error subgraph_error_pkey; Type: CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.subgraph_error
    ADD CONSTRAINT subgraph_error_pkey PRIMARY KEY (vid);


--
-- Name: subgraph_features subgraph_features_pkey; Type: CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.subgraph_features
    ADD CONSTRAINT subgraph_features_pkey PRIMARY KEY (id);


--
-- Name: subgraph subgraph_id_block_range_excl; Type: CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.subgraph
    ADD CONSTRAINT subgraph_id_block_range_excl EXCLUDE USING gist (id WITH =, block_range WITH &&);


--
-- Name: subgraph_manifest subgraph_manifest_pkey; Type: CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.subgraph_manifest
    ADD CONSTRAINT subgraph_manifest_pkey PRIMARY KEY (id);


--
-- Name: subgraph subgraph_name_uq; Type: CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.subgraph
    ADD CONSTRAINT subgraph_name_uq UNIQUE (name);


--
-- Name: subgraph subgraph_pkey; Type: CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.subgraph
    ADD CONSTRAINT subgraph_pkey PRIMARY KEY (vid);


--
-- Name: subgraph_version subgraph_version_id_block_range_excl; Type: CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.subgraph_version
    ADD CONSTRAINT subgraph_version_id_block_range_excl EXCLUDE USING gist (id WITH =, block_range WITH &&);


--
-- Name: subgraph_version subgraph_version_pkey; Type: CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.subgraph_version
    ADD CONSTRAINT subgraph_version_pkey PRIMARY KEY (vid);


--
-- Name: table_stats table_stats_deployment_table_name_key; Type: CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.table_stats
    ADD CONSTRAINT table_stats_deployment_table_name_key UNIQUE (deployment, table_name);


--
-- Name: table_stats table_stats_pkey; Type: CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.table_stats
    ADD CONSTRAINT table_stats_pkey PRIMARY KEY (id);


--
-- Name: graph_node_versions unique_graph_node_versions; Type: CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.graph_node_versions
    ADD CONSTRAINT unique_graph_node_versions UNIQUE (git_commit_hash, git_repository_dirty, crate_version, major, minor, patch);


--
-- Name: attr_0_0_subgraph_id; Type: INDEX; Schema: subgraphs; Owner: graph
--

CREATE INDEX attr_0_0_subgraph_id ON subgraphs.subgraph USING btree (id);


--
-- Name: attr_0_1_subgraph_name; Type: INDEX; Schema: subgraphs; Owner: graph
--

CREATE INDEX attr_0_1_subgraph_name ON subgraphs.subgraph USING btree ("left"(name, 256));


--
-- Name: attr_0_2_subgraph_current_version; Type: INDEX; Schema: subgraphs; Owner: graph
--

CREATE INDEX attr_0_2_subgraph_current_version ON subgraphs.subgraph USING btree (current_version);


--
-- Name: attr_0_3_subgraph_pending_version; Type: INDEX; Schema: subgraphs; Owner: graph
--

CREATE INDEX attr_0_3_subgraph_pending_version ON subgraphs.subgraph USING btree (pending_version);


--
-- Name: attr_0_4_subgraph_created_at; Type: INDEX; Schema: subgraphs; Owner: graph
--

CREATE INDEX attr_0_4_subgraph_created_at ON subgraphs.subgraph USING btree (created_at);


--
-- Name: attr_16_0_subgraph_error_id; Type: INDEX; Schema: subgraphs; Owner: graph
--

CREATE INDEX attr_16_0_subgraph_error_id ON subgraphs.subgraph_error USING btree (id);


--
-- Name: attr_1_0_subgraph_version_id; Type: INDEX; Schema: subgraphs; Owner: graph
--

CREATE INDEX attr_1_0_subgraph_version_id ON subgraphs.subgraph_version USING btree (id);


--
-- Name: attr_1_1_subgraph_version_subgraph; Type: INDEX; Schema: subgraphs; Owner: graph
--

CREATE INDEX attr_1_1_subgraph_version_subgraph ON subgraphs.subgraph_version USING btree (subgraph);


--
-- Name: attr_1_2_subgraph_version_deployment; Type: INDEX; Schema: subgraphs; Owner: graph
--

CREATE INDEX attr_1_2_subgraph_version_deployment ON subgraphs.subgraph_version USING btree (deployment);


--
-- Name: attr_1_3_subgraph_version_created_at; Type: INDEX; Schema: subgraphs; Owner: graph
--

CREATE INDEX attr_1_3_subgraph_version_created_at ON subgraphs.subgraph_version USING btree (created_at);


--
-- Name: attr_6_9_dynamic_ethereum_contract_data_source_deployment; Type: INDEX; Schema: subgraphs; Owner: graph
--

CREATE INDEX attr_6_9_dynamic_ethereum_contract_data_source_deployment ON subgraphs.dynamic_ethereum_contract_data_source USING btree (deployment);


--
-- Name: copy_state copy_state_dst_fkey; Type: FK CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.copy_state
    ADD CONSTRAINT copy_state_dst_fkey FOREIGN KEY (dst) REFERENCES subgraphs.subgraph_deployment(id) ON DELETE CASCADE;


--
-- Name: copy_table_state copy_table_state_dst_fkey; Type: FK CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.copy_table_state
    ADD CONSTRAINT copy_table_state_dst_fkey FOREIGN KEY (dst) REFERENCES subgraphs.copy_state(dst) ON DELETE CASCADE;


--
-- Name: subgraph_manifest graph_node_versions_fk; Type: FK CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.subgraph_manifest
    ADD CONSTRAINT graph_node_versions_fk FOREIGN KEY (graph_node_version_id) REFERENCES subgraphs.graph_node_versions(id);


--
-- Name: subgraph_error subgraph_error_subgraph_id_fkey; Type: FK CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.subgraph_error
    ADD CONSTRAINT subgraph_error_subgraph_id_fkey FOREIGN KEY (subgraph_id) REFERENCES subgraphs.subgraph_deployment(deployment) ON DELETE CASCADE;


--
-- Name: subgraph_manifest subgraph_manifest_new_id_fkey; Type: FK CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.subgraph_manifest
    ADD CONSTRAINT subgraph_manifest_new_id_fkey FOREIGN KEY (id) REFERENCES subgraphs.subgraph_deployment(id) ON DELETE CASCADE;


--
-- Name: table_stats table_stats_deployment_fkey; Type: FK CONSTRAINT; Schema: subgraphs; Owner: graph
--

ALTER TABLE ONLY subgraphs.table_stats
    ADD CONSTRAINT table_stats_deployment_fkey FOREIGN KEY (deployment) REFERENCES subgraphs.subgraph_deployment(id) ON DELETE CASCADE;


--
-- PostgreSQL database dump complete
--

