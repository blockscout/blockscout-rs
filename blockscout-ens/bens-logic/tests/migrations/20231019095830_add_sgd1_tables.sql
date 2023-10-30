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

CREATE SCHEMA IF NOT EXISTS sgd1;

--
-- Name: abi_changed; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.abi_changed (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    resolver text NOT NULL,
    block_number integer NOT NULL,
    transaction_id bytea NOT NULL,
    content_type numeric NOT NULL
);


ALTER TABLE sgd1.abi_changed OWNER TO "graph-node";

--
-- Name: abi_changed_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.abi_changed_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.abi_changed_vid_seq OWNER TO "graph-node";

--
-- Name: abi_changed_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.abi_changed_vid_seq OWNED BY sgd1.abi_changed.vid;


--
-- Name: account; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.account (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL
);


ALTER TABLE sgd1.account OWNER TO "graph-node";

--
-- Name: account_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.account_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.account_vid_seq OWNER TO "graph-node";

--
-- Name: account_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.account_vid_seq OWNED BY sgd1.account.vid;


--
-- Name: addr_changed; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.addr_changed (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    resolver text NOT NULL,
    block_number integer NOT NULL,
    transaction_id bytea NOT NULL,
    addr text NOT NULL
);


ALTER TABLE sgd1.addr_changed OWNER TO "graph-node";

--
-- Name: addr_changed_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.addr_changed_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.addr_changed_vid_seq OWNER TO "graph-node";

--
-- Name: addr_changed_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.addr_changed_vid_seq OWNED BY sgd1.addr_changed.vid;


--
-- Name: authorisation_changed; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.authorisation_changed (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    resolver text NOT NULL,
    block_number integer NOT NULL,
    transaction_id bytea NOT NULL,
    owner bytea NOT NULL,
    target bytea NOT NULL,
    is_authorized boolean NOT NULL
);


ALTER TABLE sgd1.authorisation_changed OWNER TO "graph-node";

--
-- Name: authorisation_changed_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.authorisation_changed_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.authorisation_changed_vid_seq OWNER TO "graph-node";

--
-- Name: authorisation_changed_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.authorisation_changed_vid_seq OWNED BY sgd1.authorisation_changed.vid;


--
-- Name: contenthash_changed; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.contenthash_changed (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    resolver text NOT NULL,
    block_number integer NOT NULL,
    transaction_id bytea NOT NULL,
    hash bytea NOT NULL
);


ALTER TABLE sgd1.contenthash_changed OWNER TO "graph-node";

--
-- Name: contenthash_changed_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.contenthash_changed_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.contenthash_changed_vid_seq OWNER TO "graph-node";

--
-- Name: contenthash_changed_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.contenthash_changed_vid_seq OWNED BY sgd1.contenthash_changed.vid;


--
-- Name: data_sources$; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1."data_sources$" (
    vid integer NOT NULL,
    block_range int4range NOT NULL,
    causality_region integer NOT NULL,
    manifest_idx integer NOT NULL,
    parent integer,
    id bytea,
    param bytea,
    context jsonb,
    done_at integer
);


ALTER TABLE sgd1."data_sources$" OWNER TO "graph-node";

--
-- Name: data_sources$_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

ALTER TABLE sgd1."data_sources$" ALTER COLUMN vid ADD GENERATED BY DEFAULT AS IDENTITY (
    SEQUENCE NAME sgd1."data_sources$_vid_seq"
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: domain; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.domain (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    name text,
    label_name text,
    labelhash bytea,
    parent text,
    subdomain_count integer NOT NULL,
    resolved_address text,
    resolver text,
    ttl numeric,
    is_migrated boolean NOT NULL,
    created_at numeric NOT NULL,
    owner text NOT NULL,
    registrant text,
    wrapped_owner text,
    expiry_date numeric
);


ALTER TABLE sgd1.domain OWNER TO "graph-node";

--
-- Name: domain_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.domain_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.domain_vid_seq OWNER TO "graph-node";

--
-- Name: domain_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.domain_vid_seq OWNED BY sgd1.domain.vid;


--
-- Name: expiry_extended; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.expiry_extended (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    domain text NOT NULL,
    block_number integer NOT NULL,
    transaction_id bytea NOT NULL,
    expiry_date numeric NOT NULL
);


ALTER TABLE sgd1.expiry_extended OWNER TO "graph-node";

--
-- Name: expiry_extended_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.expiry_extended_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.expiry_extended_vid_seq OWNER TO "graph-node";

--
-- Name: expiry_extended_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.expiry_extended_vid_seq OWNED BY sgd1.expiry_extended.vid;


--
-- Name: fuses_set; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.fuses_set (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    domain text NOT NULL,
    block_number integer NOT NULL,
    transaction_id bytea NOT NULL,
    fuses integer NOT NULL
);


ALTER TABLE sgd1.fuses_set OWNER TO "graph-node";

--
-- Name: fuses_set_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.fuses_set_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.fuses_set_vid_seq OWNER TO "graph-node";

--
-- Name: fuses_set_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.fuses_set_vid_seq OWNED BY sgd1.fuses_set.vid;


--
-- Name: interface_changed; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.interface_changed (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    resolver text NOT NULL,
    block_number integer NOT NULL,
    transaction_id bytea NOT NULL,
    interface_id bytea NOT NULL,
    implementer bytea NOT NULL
);


ALTER TABLE sgd1.interface_changed OWNER TO "graph-node";

--
-- Name: interface_changed_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.interface_changed_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.interface_changed_vid_seq OWNER TO "graph-node";

--
-- Name: interface_changed_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.interface_changed_vid_seq OWNED BY sgd1.interface_changed.vid;


--
-- Name: multicoin_addr_changed; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.multicoin_addr_changed (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    resolver text NOT NULL,
    block_number integer NOT NULL,
    transaction_id bytea NOT NULL,
    coin_type numeric NOT NULL,
    addr bytea NOT NULL
);


ALTER TABLE sgd1.multicoin_addr_changed OWNER TO "graph-node";

--
-- Name: multicoin_addr_changed_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.multicoin_addr_changed_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.multicoin_addr_changed_vid_seq OWNER TO "graph-node";

--
-- Name: multicoin_addr_changed_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.multicoin_addr_changed_vid_seq OWNED BY sgd1.multicoin_addr_changed.vid;


--
-- Name: name_changed; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.name_changed (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    resolver text NOT NULL,
    block_number integer NOT NULL,
    transaction_id bytea NOT NULL,
    name text NOT NULL
);


ALTER TABLE sgd1.name_changed OWNER TO "graph-node";

--
-- Name: name_changed_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.name_changed_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.name_changed_vid_seq OWNER TO "graph-node";

--
-- Name: name_changed_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.name_changed_vid_seq OWNED BY sgd1.name_changed.vid;


--
-- Name: name_registered; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.name_registered (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    registration text NOT NULL,
    block_number integer NOT NULL,
    transaction_id bytea NOT NULL,
    registrant text NOT NULL,
    expiry_date numeric NOT NULL
);


ALTER TABLE sgd1.name_registered OWNER TO "graph-node";

--
-- Name: name_registered_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.name_registered_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.name_registered_vid_seq OWNER TO "graph-node";

--
-- Name: name_registered_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.name_registered_vid_seq OWNED BY sgd1.name_registered.vid;


--
-- Name: name_renewed; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.name_renewed (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    registration text NOT NULL,
    block_number integer NOT NULL,
    transaction_id bytea NOT NULL,
    expiry_date numeric NOT NULL
);


ALTER TABLE sgd1.name_renewed OWNER TO "graph-node";

--
-- Name: name_renewed_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.name_renewed_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.name_renewed_vid_seq OWNER TO "graph-node";

--
-- Name: name_renewed_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.name_renewed_vid_seq OWNED BY sgd1.name_renewed.vid;


--
-- Name: name_transferred; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.name_transferred (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    registration text NOT NULL,
    block_number integer NOT NULL,
    transaction_id bytea NOT NULL,
    new_owner text NOT NULL
);


ALTER TABLE sgd1.name_transferred OWNER TO "graph-node";

--
-- Name: name_transferred_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.name_transferred_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.name_transferred_vid_seq OWNER TO "graph-node";

--
-- Name: name_transferred_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.name_transferred_vid_seq OWNED BY sgd1.name_transferred.vid;


--
-- Name: name_unwrapped; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.name_unwrapped (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    domain text NOT NULL,
    block_number integer NOT NULL,
    transaction_id bytea NOT NULL,
    owner text NOT NULL
);


ALTER TABLE sgd1.name_unwrapped OWNER TO "graph-node";

--
-- Name: name_unwrapped_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.name_unwrapped_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.name_unwrapped_vid_seq OWNER TO "graph-node";

--
-- Name: name_unwrapped_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.name_unwrapped_vid_seq OWNED BY sgd1.name_unwrapped.vid;


--
-- Name: name_wrapped; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.name_wrapped (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    domain text NOT NULL,
    block_number integer NOT NULL,
    transaction_id bytea NOT NULL,
    name text,
    fuses integer NOT NULL,
    owner text NOT NULL,
    expiry_date numeric NOT NULL
);


ALTER TABLE sgd1.name_wrapped OWNER TO "graph-node";

--
-- Name: name_wrapped_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.name_wrapped_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.name_wrapped_vid_seq OWNER TO "graph-node";

--
-- Name: name_wrapped_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.name_wrapped_vid_seq OWNED BY sgd1.name_wrapped.vid;


--
-- Name: new_owner; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.new_owner (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    parent_domain text NOT NULL,
    domain text NOT NULL,
    block_number integer NOT NULL,
    transaction_id bytea NOT NULL,
    owner text NOT NULL
);


ALTER TABLE sgd1.new_owner OWNER TO "graph-node";

--
-- Name: new_owner_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.new_owner_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.new_owner_vid_seq OWNER TO "graph-node";

--
-- Name: new_owner_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.new_owner_vid_seq OWNED BY sgd1.new_owner.vid;


--
-- Name: new_resolver; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.new_resolver (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    domain text NOT NULL,
    block_number integer NOT NULL,
    transaction_id bytea NOT NULL,
    resolver text NOT NULL
);


ALTER TABLE sgd1.new_resolver OWNER TO "graph-node";

--
-- Name: new_resolver_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.new_resolver_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.new_resolver_vid_seq OWNER TO "graph-node";

--
-- Name: new_resolver_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.new_resolver_vid_seq OWNED BY sgd1.new_resolver.vid;


--
-- Name: new_ttl; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.new_ttl (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    domain text NOT NULL,
    block_number integer NOT NULL,
    transaction_id bytea NOT NULL,
    ttl numeric NOT NULL
);


ALTER TABLE sgd1.new_ttl OWNER TO "graph-node";

--
-- Name: new_ttl_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.new_ttl_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.new_ttl_vid_seq OWNER TO "graph-node";

--
-- Name: new_ttl_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.new_ttl_vid_seq OWNED BY sgd1.new_ttl.vid;


--
-- Name: poi2$; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1."poi2$" (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    digest bytea NOT NULL,
    id text NOT NULL
);


ALTER TABLE sgd1."poi2$" OWNER TO "graph-node";

--
-- Name: poi2$_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1."poi2$_vid_seq"
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1."poi2$_vid_seq" OWNER TO "graph-node";

--
-- Name: poi2$_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1."poi2$_vid_seq" OWNED BY sgd1."poi2$".vid;


--
-- Name: pubkey_changed; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.pubkey_changed (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    resolver text NOT NULL,
    block_number integer NOT NULL,
    transaction_id bytea NOT NULL,
    x bytea NOT NULL,
    y bytea NOT NULL
);


ALTER TABLE sgd1.pubkey_changed OWNER TO "graph-node";

--
-- Name: pubkey_changed_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.pubkey_changed_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.pubkey_changed_vid_seq OWNER TO "graph-node";

--
-- Name: pubkey_changed_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.pubkey_changed_vid_seq OWNED BY sgd1.pubkey_changed.vid;


--
-- Name: registration; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.registration (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    domain text NOT NULL,
    registration_date numeric NOT NULL,
    expiry_date numeric NOT NULL,
    cost numeric,
    registrant text NOT NULL,
    label_name text
);


ALTER TABLE sgd1.registration OWNER TO "graph-node";

--
-- Name: registration_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.registration_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.registration_vid_seq OWNER TO "graph-node";

--
-- Name: registration_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.registration_vid_seq OWNED BY sgd1.registration.vid;


--
-- Name: resolver; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.resolver (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    domain text,
    address bytea NOT NULL,
    addr text,
    content_hash bytea,
    texts text[],
    coin_types numeric[]
);


ALTER TABLE sgd1.resolver OWNER TO "graph-node";

--
-- Name: resolver_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.resolver_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.resolver_vid_seq OWNER TO "graph-node";

--
-- Name: resolver_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.resolver_vid_seq OWNED BY sgd1.resolver.vid;


--
-- Name: text_changed; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.text_changed (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    resolver text NOT NULL,
    block_number integer NOT NULL,
    transaction_id bytea NOT NULL,
    key text NOT NULL,
    value text
);


ALTER TABLE sgd1.text_changed OWNER TO "graph-node";

--
-- Name: text_changed_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.text_changed_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.text_changed_vid_seq OWNER TO "graph-node";

--
-- Name: text_changed_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.text_changed_vid_seq OWNED BY sgd1.text_changed.vid;


--
-- Name: transfer; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.transfer (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    domain text NOT NULL,
    block_number integer NOT NULL,
    transaction_id bytea NOT NULL,
    owner text NOT NULL
);


ALTER TABLE sgd1.transfer OWNER TO "graph-node";

--
-- Name: transfer_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.transfer_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.transfer_vid_seq OWNER TO "graph-node";

--
-- Name: transfer_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.transfer_vid_seq OWNED BY sgd1.transfer.vid;


--
-- Name: version_changed; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.version_changed (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    resolver text NOT NULL,
    block_number integer NOT NULL,
    transaction_id bytea NOT NULL,
    version numeric NOT NULL
);


ALTER TABLE sgd1.version_changed OWNER TO "graph-node";

--
-- Name: version_changed_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.version_changed_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.version_changed_vid_seq OWNER TO "graph-node";

--
-- Name: version_changed_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.version_changed_vid_seq OWNED BY sgd1.version_changed.vid;


--
-- Name: wrapped_domain; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.wrapped_domain (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    domain text NOT NULL,
    expiry_date numeric NOT NULL,
    fuses integer NOT NULL,
    owner text NOT NULL,
    name text
);


ALTER TABLE sgd1.wrapped_domain OWNER TO "graph-node";

--
-- Name: wrapped_domain_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.wrapped_domain_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.wrapped_domain_vid_seq OWNER TO "graph-node";

--
-- Name: wrapped_domain_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.wrapped_domain_vid_seq OWNED BY sgd1.wrapped_domain.vid;


--
-- Name: wrapped_transfer; Type: TABLE; Schema: sgd1; Owner: graph-node
--

CREATE TABLE sgd1.wrapped_transfer (
    vid bigint NOT NULL,
    block_range int4range NOT NULL,
    id text NOT NULL,
    domain text NOT NULL,
    block_number integer NOT NULL,
    transaction_id bytea NOT NULL,
    owner text NOT NULL
);


ALTER TABLE sgd1.wrapped_transfer OWNER TO "graph-node";

--
-- Name: wrapped_transfer_vid_seq; Type: SEQUENCE; Schema: sgd1; Owner: graph-node
--

CREATE SEQUENCE sgd1.wrapped_transfer_vid_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE sgd1.wrapped_transfer_vid_seq OWNER TO "graph-node";

--
-- Name: wrapped_transfer_vid_seq; Type: SEQUENCE OWNED BY; Schema: sgd1; Owner: graph-node
--

ALTER SEQUENCE sgd1.wrapped_transfer_vid_seq OWNED BY sgd1.wrapped_transfer.vid;


--
-- Name: abi_changed vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.abi_changed ALTER COLUMN vid SET DEFAULT nextval('sgd1.abi_changed_vid_seq'::regclass);


--
-- Name: account vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.account ALTER COLUMN vid SET DEFAULT nextval('sgd1.account_vid_seq'::regclass);


--
-- Name: addr_changed vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.addr_changed ALTER COLUMN vid SET DEFAULT nextval('sgd1.addr_changed_vid_seq'::regclass);


--
-- Name: authorisation_changed vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.authorisation_changed ALTER COLUMN vid SET DEFAULT nextval('sgd1.authorisation_changed_vid_seq'::regclass);


--
-- Name: contenthash_changed vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.contenthash_changed ALTER COLUMN vid SET DEFAULT nextval('sgd1.contenthash_changed_vid_seq'::regclass);


--
-- Name: domain vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.domain ALTER COLUMN vid SET DEFAULT nextval('sgd1.domain_vid_seq'::regclass);


--
-- Name: expiry_extended vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.expiry_extended ALTER COLUMN vid SET DEFAULT nextval('sgd1.expiry_extended_vid_seq'::regclass);


--
-- Name: fuses_set vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.fuses_set ALTER COLUMN vid SET DEFAULT nextval('sgd1.fuses_set_vid_seq'::regclass);


--
-- Name: interface_changed vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.interface_changed ALTER COLUMN vid SET DEFAULT nextval('sgd1.interface_changed_vid_seq'::regclass);


--
-- Name: multicoin_addr_changed vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.multicoin_addr_changed ALTER COLUMN vid SET DEFAULT nextval('sgd1.multicoin_addr_changed_vid_seq'::regclass);


--
-- Name: name_changed vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.name_changed ALTER COLUMN vid SET DEFAULT nextval('sgd1.name_changed_vid_seq'::regclass);


--
-- Name: name_registered vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.name_registered ALTER COLUMN vid SET DEFAULT nextval('sgd1.name_registered_vid_seq'::regclass);


--
-- Name: name_renewed vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.name_renewed ALTER COLUMN vid SET DEFAULT nextval('sgd1.name_renewed_vid_seq'::regclass);


--
-- Name: name_transferred vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.name_transferred ALTER COLUMN vid SET DEFAULT nextval('sgd1.name_transferred_vid_seq'::regclass);


--
-- Name: name_unwrapped vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.name_unwrapped ALTER COLUMN vid SET DEFAULT nextval('sgd1.name_unwrapped_vid_seq'::regclass);


--
-- Name: name_wrapped vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.name_wrapped ALTER COLUMN vid SET DEFAULT nextval('sgd1.name_wrapped_vid_seq'::regclass);


--
-- Name: new_owner vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.new_owner ALTER COLUMN vid SET DEFAULT nextval('sgd1.new_owner_vid_seq'::regclass);


--
-- Name: new_resolver vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.new_resolver ALTER COLUMN vid SET DEFAULT nextval('sgd1.new_resolver_vid_seq'::regclass);


--
-- Name: new_ttl vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.new_ttl ALTER COLUMN vid SET DEFAULT nextval('sgd1.new_ttl_vid_seq'::regclass);


--
-- Name: poi2$ vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1."poi2$" ALTER COLUMN vid SET DEFAULT nextval('sgd1."poi2$_vid_seq"'::regclass);


--
-- Name: pubkey_changed vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.pubkey_changed ALTER COLUMN vid SET DEFAULT nextval('sgd1.pubkey_changed_vid_seq'::regclass);


--
-- Name: registration vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.registration ALTER COLUMN vid SET DEFAULT nextval('sgd1.registration_vid_seq'::regclass);


--
-- Name: resolver vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.resolver ALTER COLUMN vid SET DEFAULT nextval('sgd1.resolver_vid_seq'::regclass);


--
-- Name: text_changed vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.text_changed ALTER COLUMN vid SET DEFAULT nextval('sgd1.text_changed_vid_seq'::regclass);


--
-- Name: transfer vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.transfer ALTER COLUMN vid SET DEFAULT nextval('sgd1.transfer_vid_seq'::regclass);


--
-- Name: version_changed vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.version_changed ALTER COLUMN vid SET DEFAULT nextval('sgd1.version_changed_vid_seq'::regclass);


--
-- Name: wrapped_domain vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.wrapped_domain ALTER COLUMN vid SET DEFAULT nextval('sgd1.wrapped_domain_vid_seq'::regclass);


--
-- Name: wrapped_transfer vid; Type: DEFAULT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.wrapped_transfer ALTER COLUMN vid SET DEFAULT nextval('sgd1.wrapped_transfer_vid_seq'::regclass);


--
-- Name: abi_changed abi_changed_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.abi_changed
    ADD CONSTRAINT abi_changed_pkey PRIMARY KEY (vid);


--
-- Name: account account_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.account
    ADD CONSTRAINT account_pkey PRIMARY KEY (vid);


--
-- Name: addr_changed addr_changed_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.addr_changed
    ADD CONSTRAINT addr_changed_pkey PRIMARY KEY (vid);


--
-- Name: authorisation_changed authorisation_changed_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.authorisation_changed
    ADD CONSTRAINT authorisation_changed_pkey PRIMARY KEY (vid);


--
-- Name: contenthash_changed contenthash_changed_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.contenthash_changed
    ADD CONSTRAINT contenthash_changed_pkey PRIMARY KEY (vid);


--
-- Name: data_sources$ data_sources$_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1."data_sources$"
    ADD CONSTRAINT "data_sources$_pkey" PRIMARY KEY (vid);


--
-- Name: domain domain_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.domain
    ADD CONSTRAINT domain_pkey PRIMARY KEY (vid);


--
-- Name: expiry_extended expiry_extended_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.expiry_extended
    ADD CONSTRAINT expiry_extended_pkey PRIMARY KEY (vid);


--
-- Name: fuses_set fuses_set_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.fuses_set
    ADD CONSTRAINT fuses_set_pkey PRIMARY KEY (vid);


--
-- Name: interface_changed interface_changed_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.interface_changed
    ADD CONSTRAINT interface_changed_pkey PRIMARY KEY (vid);


--
-- Name: multicoin_addr_changed multicoin_addr_changed_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.multicoin_addr_changed
    ADD CONSTRAINT multicoin_addr_changed_pkey PRIMARY KEY (vid);


--
-- Name: name_changed name_changed_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.name_changed
    ADD CONSTRAINT name_changed_pkey PRIMARY KEY (vid);


--
-- Name: name_registered name_registered_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.name_registered
    ADD CONSTRAINT name_registered_pkey PRIMARY KEY (vid);


--
-- Name: name_renewed name_renewed_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.name_renewed
    ADD CONSTRAINT name_renewed_pkey PRIMARY KEY (vid);


--
-- Name: name_transferred name_transferred_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.name_transferred
    ADD CONSTRAINT name_transferred_pkey PRIMARY KEY (vid);


--
-- Name: name_unwrapped name_unwrapped_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.name_unwrapped
    ADD CONSTRAINT name_unwrapped_pkey PRIMARY KEY (vid);


--
-- Name: name_wrapped name_wrapped_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.name_wrapped
    ADD CONSTRAINT name_wrapped_pkey PRIMARY KEY (vid);


--
-- Name: new_owner new_owner_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.new_owner
    ADD CONSTRAINT new_owner_pkey PRIMARY KEY (vid);


--
-- Name: new_resolver new_resolver_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.new_resolver
    ADD CONSTRAINT new_resolver_pkey PRIMARY KEY (vid);


--
-- Name: new_ttl new_ttl_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.new_ttl
    ADD CONSTRAINT new_ttl_pkey PRIMARY KEY (vid);


--
-- Name: poi2$ poi2$_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1."poi2$"
    ADD CONSTRAINT "poi2$_pkey" PRIMARY KEY (vid);


--
-- Name: pubkey_changed pubkey_changed_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.pubkey_changed
    ADD CONSTRAINT pubkey_changed_pkey PRIMARY KEY (vid);


--
-- Name: registration registration_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.registration
    ADD CONSTRAINT registration_pkey PRIMARY KEY (vid);


--
-- Name: resolver resolver_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.resolver
    ADD CONSTRAINT resolver_pkey PRIMARY KEY (vid);


--
-- Name: text_changed text_changed_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.text_changed
    ADD CONSTRAINT text_changed_pkey PRIMARY KEY (vid);


--
-- Name: transfer transfer_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.transfer
    ADD CONSTRAINT transfer_pkey PRIMARY KEY (vid);


--
-- Name: version_changed version_changed_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.version_changed
    ADD CONSTRAINT version_changed_pkey PRIMARY KEY (vid);


--
-- Name: wrapped_domain wrapped_domain_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.wrapped_domain
    ADD CONSTRAINT wrapped_domain_pkey PRIMARY KEY (vid);


--
-- Name: wrapped_transfer wrapped_transfer_pkey; Type: CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1.wrapped_transfer
    ADD CONSTRAINT wrapped_transfer_pkey PRIMARY KEY (vid);


--
-- Name: abi_changed_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX abi_changed_block_range_closed ON sgd1.abi_changed USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: abi_changed_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX abi_changed_id_block_range_excl ON sgd1.abi_changed USING gist (id, block_range);


--
-- Name: account_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX account_block_range_closed ON sgd1.account USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: account_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX account_id_block_range_excl ON sgd1.account USING gist (id, block_range);


--
-- Name: addr_changed_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX addr_changed_block_range_closed ON sgd1.addr_changed USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: addr_changed_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX addr_changed_id_block_range_excl ON sgd1.addr_changed USING gist (id, block_range);


--
-- Name: attr_0_0_domain_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_0_0_domain_id ON sgd1.domain USING btree (id);


--
-- Name: attr_0_10_domain_created_at; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_0_10_domain_created_at ON sgd1.domain USING btree (created_at);


--
-- Name: attr_0_11_domain_owner; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_0_11_domain_owner ON sgd1.domain USING gist (owner, block_range);


--
-- Name: attr_0_12_domain_registrant; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_0_12_domain_registrant ON sgd1.domain USING gist (registrant, block_range);


--
-- Name: attr_0_13_domain_wrapped_owner; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_0_13_domain_wrapped_owner ON sgd1.domain USING gist (wrapped_owner, block_range);


--
-- Name: attr_0_14_domain_expiry_date; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_0_14_domain_expiry_date ON sgd1.domain USING btree (expiry_date);


--
-- Name: attr_0_1_domain_name; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_0_1_domain_name ON sgd1.domain USING btree ("left"(name, 256));


--
-- Name: attr_0_2_domain_label_name; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_0_2_domain_label_name ON sgd1.domain USING btree ("left"(label_name, 256));


--
-- Name: attr_0_3_domain_labelhash; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_0_3_domain_labelhash ON sgd1.domain USING btree ("substring"(labelhash, 1, 64));


--
-- Name: attr_0_4_domain_parent; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_0_4_domain_parent ON sgd1.domain USING gist (parent, block_range);


--
-- Name: attr_0_5_domain_subdomain_count; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_0_5_domain_subdomain_count ON sgd1.domain USING btree (subdomain_count);


--
-- Name: attr_0_6_domain_resolved_address; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_0_6_domain_resolved_address ON sgd1.domain USING gist (resolved_address, block_range);


--
-- Name: attr_0_7_domain_resolver; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_0_7_domain_resolver ON sgd1.domain USING gist (resolver, block_range);


--
-- Name: attr_0_8_domain_ttl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_0_8_domain_ttl ON sgd1.domain USING btree (ttl);


--
-- Name: attr_0_9_domain_is_migrated; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_0_9_domain_is_migrated ON sgd1.domain USING btree (is_migrated);


--
-- Name: attr_10_0_registration_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_10_0_registration_id ON sgd1.registration USING btree (id);


--
-- Name: attr_10_1_registration_domain; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_10_1_registration_domain ON sgd1.registration USING gist (domain, block_range);


--
-- Name: attr_10_2_registration_registration_date; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_10_2_registration_registration_date ON sgd1.registration USING btree (registration_date);


--
-- Name: attr_10_3_registration_expiry_date; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_10_3_registration_expiry_date ON sgd1.registration USING btree (expiry_date);


--
-- Name: attr_10_4_registration_cost; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_10_4_registration_cost ON sgd1.registration USING btree (cost);


--
-- Name: attr_10_5_registration_registrant; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_10_5_registration_registrant ON sgd1.registration USING gist (registrant, block_range);


--
-- Name: attr_10_6_registration_label_name; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_10_6_registration_label_name ON sgd1.registration USING btree ("left"(label_name, 256));


--
-- Name: attr_11_0_name_registered_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_11_0_name_registered_id ON sgd1.name_registered USING btree (id);


--
-- Name: attr_11_1_name_registered_registration; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_11_1_name_registered_registration ON sgd1.name_registered USING gist (registration, block_range);


--
-- Name: attr_11_2_name_registered_block_number; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_11_2_name_registered_block_number ON sgd1.name_registered USING btree (block_number);


--
-- Name: attr_11_3_name_registered_transaction_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_11_3_name_registered_transaction_id ON sgd1.name_registered USING btree ("substring"(transaction_id, 1, 64));


--
-- Name: attr_11_4_name_registered_registrant; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_11_4_name_registered_registrant ON sgd1.name_registered USING gist (registrant, block_range);


--
-- Name: attr_11_5_name_registered_expiry_date; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_11_5_name_registered_expiry_date ON sgd1.name_registered USING btree (expiry_date);


--
-- Name: attr_12_0_name_renewed_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_12_0_name_renewed_id ON sgd1.name_renewed USING btree (id);


--
-- Name: attr_12_1_name_renewed_registration; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_12_1_name_renewed_registration ON sgd1.name_renewed USING gist (registration, block_range);


--
-- Name: attr_12_2_name_renewed_block_number; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_12_2_name_renewed_block_number ON sgd1.name_renewed USING btree (block_number);


--
-- Name: attr_12_3_name_renewed_transaction_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_12_3_name_renewed_transaction_id ON sgd1.name_renewed USING btree ("substring"(transaction_id, 1, 64));


--
-- Name: attr_12_4_name_renewed_expiry_date; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_12_4_name_renewed_expiry_date ON sgd1.name_renewed USING btree (expiry_date);


--
-- Name: attr_13_0_name_transferred_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_13_0_name_transferred_id ON sgd1.name_transferred USING btree (id);


--
-- Name: attr_13_1_name_transferred_registration; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_13_1_name_transferred_registration ON sgd1.name_transferred USING gist (registration, block_range);


--
-- Name: attr_13_2_name_transferred_block_number; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_13_2_name_transferred_block_number ON sgd1.name_transferred USING btree (block_number);


--
-- Name: attr_13_3_name_transferred_transaction_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_13_3_name_transferred_transaction_id ON sgd1.name_transferred USING btree ("substring"(transaction_id, 1, 64));


--
-- Name: attr_13_4_name_transferred_new_owner; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_13_4_name_transferred_new_owner ON sgd1.name_transferred USING gist (new_owner, block_range);


--
-- Name: attr_14_0_wrapped_domain_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_14_0_wrapped_domain_id ON sgd1.wrapped_domain USING btree (id);


--
-- Name: attr_14_1_wrapped_domain_domain; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_14_1_wrapped_domain_domain ON sgd1.wrapped_domain USING gist (domain, block_range);


--
-- Name: attr_14_2_wrapped_domain_expiry_date; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_14_2_wrapped_domain_expiry_date ON sgd1.wrapped_domain USING btree (expiry_date);


--
-- Name: attr_14_3_wrapped_domain_fuses; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_14_3_wrapped_domain_fuses ON sgd1.wrapped_domain USING btree (fuses);


--
-- Name: attr_14_4_wrapped_domain_owner; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_14_4_wrapped_domain_owner ON sgd1.wrapped_domain USING gist (owner, block_range);


--
-- Name: attr_14_5_wrapped_domain_name; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_14_5_wrapped_domain_name ON sgd1.wrapped_domain USING btree ("left"(name, 256));


--
-- Name: attr_15_0_account_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_15_0_account_id ON sgd1.account USING btree (id);


--
-- Name: attr_16_0_resolver_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_16_0_resolver_id ON sgd1.resolver USING btree (id);


--
-- Name: attr_16_1_resolver_domain; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_16_1_resolver_domain ON sgd1.resolver USING gist (domain, block_range);


--
-- Name: attr_16_2_resolver_address; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_16_2_resolver_address ON sgd1.resolver USING btree ("substring"(address, 1, 64));


--
-- Name: attr_16_3_resolver_addr; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_16_3_resolver_addr ON sgd1.resolver USING gist (addr, block_range);


--
-- Name: attr_16_4_resolver_content_hash; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_16_4_resolver_content_hash ON sgd1.resolver USING btree ("substring"(content_hash, 1, 64));


--
-- Name: attr_16_5_resolver_texts; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_16_5_resolver_texts ON sgd1.resolver USING gin (texts);


--
-- Name: attr_17_0_addr_changed_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_17_0_addr_changed_id ON sgd1.addr_changed USING btree (id);


--
-- Name: attr_17_1_addr_changed_resolver; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_17_1_addr_changed_resolver ON sgd1.addr_changed USING gist (resolver, block_range);


--
-- Name: attr_17_2_addr_changed_block_number; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_17_2_addr_changed_block_number ON sgd1.addr_changed USING btree (block_number);


--
-- Name: attr_17_3_addr_changed_transaction_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_17_3_addr_changed_transaction_id ON sgd1.addr_changed USING btree ("substring"(transaction_id, 1, 64));


--
-- Name: attr_17_4_addr_changed_addr; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_17_4_addr_changed_addr ON sgd1.addr_changed USING gist (addr, block_range);


--
-- Name: attr_18_0_multicoin_addr_changed_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_18_0_multicoin_addr_changed_id ON sgd1.multicoin_addr_changed USING btree (id);


--
-- Name: attr_18_1_multicoin_addr_changed_resolver; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_18_1_multicoin_addr_changed_resolver ON sgd1.multicoin_addr_changed USING gist (resolver, block_range);


--
-- Name: attr_18_2_multicoin_addr_changed_block_number; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_18_2_multicoin_addr_changed_block_number ON sgd1.multicoin_addr_changed USING btree (block_number);


--
-- Name: attr_18_3_multicoin_addr_changed_transaction_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_18_3_multicoin_addr_changed_transaction_id ON sgd1.multicoin_addr_changed USING btree ("substring"(transaction_id, 1, 64));


--
-- Name: attr_18_4_multicoin_addr_changed_coin_type; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_18_4_multicoin_addr_changed_coin_type ON sgd1.multicoin_addr_changed USING btree (coin_type);


--
-- Name: attr_18_5_multicoin_addr_changed_addr; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_18_5_multicoin_addr_changed_addr ON sgd1.multicoin_addr_changed USING btree ("substring"(addr, 1, 64));


--
-- Name: attr_19_0_name_changed_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_19_0_name_changed_id ON sgd1.name_changed USING btree (id);


--
-- Name: attr_19_1_name_changed_resolver; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_19_1_name_changed_resolver ON sgd1.name_changed USING gist (resolver, block_range);


--
-- Name: attr_19_2_name_changed_block_number; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_19_2_name_changed_block_number ON sgd1.name_changed USING btree (block_number);


--
-- Name: attr_19_3_name_changed_transaction_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_19_3_name_changed_transaction_id ON sgd1.name_changed USING btree ("substring"(transaction_id, 1, 64));


--
-- Name: attr_19_4_name_changed_name; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_19_4_name_changed_name ON sgd1.name_changed USING btree ("left"(name, 256));


--
-- Name: attr_1_0_transfer_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_1_0_transfer_id ON sgd1.transfer USING btree (id);


--
-- Name: attr_1_1_transfer_domain; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_1_1_transfer_domain ON sgd1.transfer USING gist (domain, block_range);


--
-- Name: attr_1_2_transfer_block_number; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_1_2_transfer_block_number ON sgd1.transfer USING btree (block_number);


--
-- Name: attr_1_3_transfer_transaction_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_1_3_transfer_transaction_id ON sgd1.transfer USING btree ("substring"(transaction_id, 1, 64));


--
-- Name: attr_1_4_transfer_owner; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_1_4_transfer_owner ON sgd1.transfer USING gist (owner, block_range);


--
-- Name: attr_20_0_abi_changed_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_20_0_abi_changed_id ON sgd1.abi_changed USING btree (id);


--
-- Name: attr_20_1_abi_changed_resolver; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_20_1_abi_changed_resolver ON sgd1.abi_changed USING gist (resolver, block_range);


--
-- Name: attr_20_2_abi_changed_block_number; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_20_2_abi_changed_block_number ON sgd1.abi_changed USING btree (block_number);


--
-- Name: attr_20_3_abi_changed_transaction_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_20_3_abi_changed_transaction_id ON sgd1.abi_changed USING btree ("substring"(transaction_id, 1, 64));


--
-- Name: attr_20_4_abi_changed_content_type; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_20_4_abi_changed_content_type ON sgd1.abi_changed USING btree (content_type);


--
-- Name: attr_21_0_pubkey_changed_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_21_0_pubkey_changed_id ON sgd1.pubkey_changed USING btree (id);


--
-- Name: attr_21_1_pubkey_changed_resolver; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_21_1_pubkey_changed_resolver ON sgd1.pubkey_changed USING gist (resolver, block_range);


--
-- Name: attr_21_2_pubkey_changed_block_number; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_21_2_pubkey_changed_block_number ON sgd1.pubkey_changed USING btree (block_number);


--
-- Name: attr_21_3_pubkey_changed_transaction_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_21_3_pubkey_changed_transaction_id ON sgd1.pubkey_changed USING btree ("substring"(transaction_id, 1, 64));


--
-- Name: attr_21_4_pubkey_changed_x; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_21_4_pubkey_changed_x ON sgd1.pubkey_changed USING btree ("substring"(x, 1, 64));


--
-- Name: attr_21_5_pubkey_changed_y; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_21_5_pubkey_changed_y ON sgd1.pubkey_changed USING btree ("substring"(y, 1, 64));


--
-- Name: attr_22_0_text_changed_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_22_0_text_changed_id ON sgd1.text_changed USING btree (id);


--
-- Name: attr_22_1_text_changed_resolver; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_22_1_text_changed_resolver ON sgd1.text_changed USING gist (resolver, block_range);


--
-- Name: attr_22_2_text_changed_block_number; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_22_2_text_changed_block_number ON sgd1.text_changed USING btree (block_number);


--
-- Name: attr_22_3_text_changed_transaction_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_22_3_text_changed_transaction_id ON sgd1.text_changed USING btree ("substring"(transaction_id, 1, 64));


--
-- Name: attr_22_4_text_changed_key; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_22_4_text_changed_key ON sgd1.text_changed USING btree ("left"(key, 256));


--
-- Name: attr_22_5_text_changed_value; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_22_5_text_changed_value ON sgd1.text_changed USING btree ("left"(value, 256));


--
-- Name: attr_23_0_contenthash_changed_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_23_0_contenthash_changed_id ON sgd1.contenthash_changed USING btree (id);


--
-- Name: attr_23_1_contenthash_changed_resolver; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_23_1_contenthash_changed_resolver ON sgd1.contenthash_changed USING gist (resolver, block_range);


--
-- Name: attr_23_2_contenthash_changed_block_number; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_23_2_contenthash_changed_block_number ON sgd1.contenthash_changed USING btree (block_number);


--
-- Name: attr_23_3_contenthash_changed_transaction_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_23_3_contenthash_changed_transaction_id ON sgd1.contenthash_changed USING btree ("substring"(transaction_id, 1, 64));


--
-- Name: attr_23_4_contenthash_changed_hash; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_23_4_contenthash_changed_hash ON sgd1.contenthash_changed USING btree ("substring"(hash, 1, 64));


--
-- Name: attr_24_0_interface_changed_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_24_0_interface_changed_id ON sgd1.interface_changed USING btree (id);


--
-- Name: attr_24_1_interface_changed_resolver; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_24_1_interface_changed_resolver ON sgd1.interface_changed USING gist (resolver, block_range);


--
-- Name: attr_24_2_interface_changed_block_number; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_24_2_interface_changed_block_number ON sgd1.interface_changed USING btree (block_number);


--
-- Name: attr_24_3_interface_changed_transaction_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_24_3_interface_changed_transaction_id ON sgd1.interface_changed USING btree ("substring"(transaction_id, 1, 64));


--
-- Name: attr_24_4_interface_changed_interface_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_24_4_interface_changed_interface_id ON sgd1.interface_changed USING btree ("substring"(interface_id, 1, 64));


--
-- Name: attr_24_5_interface_changed_implementer; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_24_5_interface_changed_implementer ON sgd1.interface_changed USING btree ("substring"(implementer, 1, 64));


--
-- Name: attr_25_0_authorisation_changed_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_25_0_authorisation_changed_id ON sgd1.authorisation_changed USING btree (id);


--
-- Name: attr_25_1_authorisation_changed_resolver; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_25_1_authorisation_changed_resolver ON sgd1.authorisation_changed USING gist (resolver, block_range);


--
-- Name: attr_25_2_authorisation_changed_block_number; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_25_2_authorisation_changed_block_number ON sgd1.authorisation_changed USING btree (block_number);


--
-- Name: attr_25_3_authorisation_changed_transaction_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_25_3_authorisation_changed_transaction_id ON sgd1.authorisation_changed USING btree ("substring"(transaction_id, 1, 64));


--
-- Name: attr_25_4_authorisation_changed_owner; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_25_4_authorisation_changed_owner ON sgd1.authorisation_changed USING btree ("substring"(owner, 1, 64));


--
-- Name: attr_25_5_authorisation_changed_target; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_25_5_authorisation_changed_target ON sgd1.authorisation_changed USING btree ("substring"(target, 1, 64));


--
-- Name: attr_25_6_authorisation_changed_is_authorized; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_25_6_authorisation_changed_is_authorized ON sgd1.authorisation_changed USING btree (is_authorized);


--
-- Name: attr_26_0_version_changed_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_26_0_version_changed_id ON sgd1.version_changed USING btree (id);


--
-- Name: attr_26_1_version_changed_resolver; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_26_1_version_changed_resolver ON sgd1.version_changed USING gist (resolver, block_range);


--
-- Name: attr_26_2_version_changed_block_number; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_26_2_version_changed_block_number ON sgd1.version_changed USING btree (block_number);


--
-- Name: attr_26_3_version_changed_transaction_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_26_3_version_changed_transaction_id ON sgd1.version_changed USING btree ("substring"(transaction_id, 1, 64));


--
-- Name: attr_26_4_version_changed_version; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_26_4_version_changed_version ON sgd1.version_changed USING btree (version);


--
-- Name: attr_27_0_poi2$_digest; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX "attr_27_0_poi2$_digest" ON sgd1."poi2$" USING btree (digest);


--
-- Name: attr_27_1_poi2$_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX "attr_27_1_poi2$_id" ON sgd1."poi2$" USING btree (id);


--
-- Name: attr_2_0_new_owner_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_2_0_new_owner_id ON sgd1.new_owner USING btree (id);


--
-- Name: attr_2_1_new_owner_parent_domain; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_2_1_new_owner_parent_domain ON sgd1.new_owner USING gist (parent_domain, block_range);


--
-- Name: attr_2_2_new_owner_domain; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_2_2_new_owner_domain ON sgd1.new_owner USING gist (domain, block_range);


--
-- Name: attr_2_3_new_owner_block_number; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_2_3_new_owner_block_number ON sgd1.new_owner USING btree (block_number);


--
-- Name: attr_2_4_new_owner_transaction_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_2_4_new_owner_transaction_id ON sgd1.new_owner USING btree ("substring"(transaction_id, 1, 64));


--
-- Name: attr_2_5_new_owner_owner; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_2_5_new_owner_owner ON sgd1.new_owner USING gist (owner, block_range);


--
-- Name: attr_3_0_new_resolver_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_3_0_new_resolver_id ON sgd1.new_resolver USING btree (id);


--
-- Name: attr_3_1_new_resolver_domain; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_3_1_new_resolver_domain ON sgd1.new_resolver USING gist (domain, block_range);


--
-- Name: attr_3_2_new_resolver_block_number; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_3_2_new_resolver_block_number ON sgd1.new_resolver USING btree (block_number);


--
-- Name: attr_3_3_new_resolver_transaction_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_3_3_new_resolver_transaction_id ON sgd1.new_resolver USING btree ("substring"(transaction_id, 1, 64));


--
-- Name: attr_3_4_new_resolver_resolver; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_3_4_new_resolver_resolver ON sgd1.new_resolver USING gist (resolver, block_range);


--
-- Name: attr_4_0_new_ttl_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_4_0_new_ttl_id ON sgd1.new_ttl USING btree (id);


--
-- Name: attr_4_1_new_ttl_domain; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_4_1_new_ttl_domain ON sgd1.new_ttl USING gist (domain, block_range);


--
-- Name: attr_4_2_new_ttl_block_number; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_4_2_new_ttl_block_number ON sgd1.new_ttl USING btree (block_number);


--
-- Name: attr_4_3_new_ttl_transaction_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_4_3_new_ttl_transaction_id ON sgd1.new_ttl USING btree ("substring"(transaction_id, 1, 64));


--
-- Name: attr_4_4_new_ttl_ttl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_4_4_new_ttl_ttl ON sgd1.new_ttl USING btree (ttl);


--
-- Name: attr_5_0_wrapped_transfer_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_5_0_wrapped_transfer_id ON sgd1.wrapped_transfer USING btree (id);


--
-- Name: attr_5_1_wrapped_transfer_domain; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_5_1_wrapped_transfer_domain ON sgd1.wrapped_transfer USING gist (domain, block_range);


--
-- Name: attr_5_2_wrapped_transfer_block_number; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_5_2_wrapped_transfer_block_number ON sgd1.wrapped_transfer USING btree (block_number);


--
-- Name: attr_5_3_wrapped_transfer_transaction_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_5_3_wrapped_transfer_transaction_id ON sgd1.wrapped_transfer USING btree ("substring"(transaction_id, 1, 64));


--
-- Name: attr_5_4_wrapped_transfer_owner; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_5_4_wrapped_transfer_owner ON sgd1.wrapped_transfer USING gist (owner, block_range);


--
-- Name: attr_6_0_name_wrapped_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_6_0_name_wrapped_id ON sgd1.name_wrapped USING btree (id);


--
-- Name: attr_6_1_name_wrapped_domain; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_6_1_name_wrapped_domain ON sgd1.name_wrapped USING gist (domain, block_range);


--
-- Name: attr_6_2_name_wrapped_block_number; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_6_2_name_wrapped_block_number ON sgd1.name_wrapped USING btree (block_number);


--
-- Name: attr_6_3_name_wrapped_transaction_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_6_3_name_wrapped_transaction_id ON sgd1.name_wrapped USING btree ("substring"(transaction_id, 1, 64));


--
-- Name: attr_6_4_name_wrapped_name; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_6_4_name_wrapped_name ON sgd1.name_wrapped USING btree ("left"(name, 256));


--
-- Name: attr_6_5_name_wrapped_fuses; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_6_5_name_wrapped_fuses ON sgd1.name_wrapped USING btree (fuses);


--
-- Name: attr_6_6_name_wrapped_owner; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_6_6_name_wrapped_owner ON sgd1.name_wrapped USING gist (owner, block_range);


--
-- Name: attr_6_7_name_wrapped_expiry_date; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_6_7_name_wrapped_expiry_date ON sgd1.name_wrapped USING btree (expiry_date);


--
-- Name: attr_7_0_name_unwrapped_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_7_0_name_unwrapped_id ON sgd1.name_unwrapped USING btree (id);


--
-- Name: attr_7_1_name_unwrapped_domain; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_7_1_name_unwrapped_domain ON sgd1.name_unwrapped USING gist (domain, block_range);


--
-- Name: attr_7_2_name_unwrapped_block_number; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_7_2_name_unwrapped_block_number ON sgd1.name_unwrapped USING btree (block_number);


--
-- Name: attr_7_3_name_unwrapped_transaction_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_7_3_name_unwrapped_transaction_id ON sgd1.name_unwrapped USING btree ("substring"(transaction_id, 1, 64));


--
-- Name: attr_7_4_name_unwrapped_owner; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_7_4_name_unwrapped_owner ON sgd1.name_unwrapped USING gist (owner, block_range);


--
-- Name: attr_8_0_fuses_set_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_8_0_fuses_set_id ON sgd1.fuses_set USING btree (id);


--
-- Name: attr_8_1_fuses_set_domain; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_8_1_fuses_set_domain ON sgd1.fuses_set USING gist (domain, block_range);


--
-- Name: attr_8_2_fuses_set_block_number; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_8_2_fuses_set_block_number ON sgd1.fuses_set USING btree (block_number);


--
-- Name: attr_8_3_fuses_set_transaction_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_8_3_fuses_set_transaction_id ON sgd1.fuses_set USING btree ("substring"(transaction_id, 1, 64));


--
-- Name: attr_8_4_fuses_set_fuses; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_8_4_fuses_set_fuses ON sgd1.fuses_set USING btree (fuses);


--
-- Name: attr_9_0_expiry_extended_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_9_0_expiry_extended_id ON sgd1.expiry_extended USING btree (id);


--
-- Name: attr_9_1_expiry_extended_domain; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_9_1_expiry_extended_domain ON sgd1.expiry_extended USING gist (domain, block_range);


--
-- Name: attr_9_2_expiry_extended_block_number; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_9_2_expiry_extended_block_number ON sgd1.expiry_extended USING btree (block_number);


--
-- Name: attr_9_3_expiry_extended_transaction_id; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_9_3_expiry_extended_transaction_id ON sgd1.expiry_extended USING btree ("substring"(transaction_id, 1, 64));


--
-- Name: attr_9_4_expiry_extended_expiry_date; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX attr_9_4_expiry_extended_expiry_date ON sgd1.expiry_extended USING btree (expiry_date);


--
-- Name: authorisation_changed_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX authorisation_changed_block_range_closed ON sgd1.authorisation_changed USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: authorisation_changed_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX authorisation_changed_id_block_range_excl ON sgd1.authorisation_changed USING gist (id, block_range);


--
-- Name: brin_abi_changed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_abi_changed ON sgd1.abi_changed USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_account; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_account ON sgd1.account USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_addr_changed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_addr_changed ON sgd1.addr_changed USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_authorisation_changed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_authorisation_changed ON sgd1.authorisation_changed USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_contenthash_changed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_contenthash_changed ON sgd1.contenthash_changed USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_domain; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_domain ON sgd1.domain USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_expiry_extended; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_expiry_extended ON sgd1.expiry_extended USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_fuses_set; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_fuses_set ON sgd1.fuses_set USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_interface_changed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_interface_changed ON sgd1.interface_changed USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_multicoin_addr_changed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_multicoin_addr_changed ON sgd1.multicoin_addr_changed USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_name_changed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_name_changed ON sgd1.name_changed USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_name_registered; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_name_registered ON sgd1.name_registered USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_name_renewed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_name_renewed ON sgd1.name_renewed USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_name_transferred; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_name_transferred ON sgd1.name_transferred USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_name_unwrapped; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_name_unwrapped ON sgd1.name_unwrapped USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_name_wrapped; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_name_wrapped ON sgd1.name_wrapped USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_new_owner; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_new_owner ON sgd1.new_owner USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_new_resolver; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_new_resolver ON sgd1.new_resolver USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_new_ttl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_new_ttl ON sgd1.new_ttl USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_poi2$; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX "brin_poi2$" ON sgd1."poi2$" USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_pubkey_changed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_pubkey_changed ON sgd1.pubkey_changed USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_registration; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_registration ON sgd1.registration USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_resolver; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_resolver ON sgd1.resolver USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_text_changed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_text_changed ON sgd1.text_changed USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_transfer; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_transfer ON sgd1.transfer USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_version_changed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_version_changed ON sgd1.version_changed USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_wrapped_domain; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_wrapped_domain ON sgd1.wrapped_domain USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: brin_wrapped_transfer; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX brin_wrapped_transfer ON sgd1.wrapped_transfer USING brin (lower(block_range) int4_minmax_multi_ops, COALESCE(upper(block_range), 2147483647) int4_minmax_multi_ops, vid int8_minmax_multi_ops);


--
-- Name: btree_causality_region_data_sources$; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX "btree_causality_region_data_sources$" ON sgd1."data_sources$" USING btree (causality_region);


--
-- Name: contenthash_changed_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX contenthash_changed_block_range_closed ON sgd1.contenthash_changed USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: contenthash_changed_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX contenthash_changed_id_block_range_excl ON sgd1.contenthash_changed USING gist (id, block_range);


--
-- Name: domain_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX domain_block_range_closed ON sgd1.domain USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: domain_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX domain_id_block_range_excl ON sgd1.domain USING gist (id, block_range);


--
-- Name: expiry_extended_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX expiry_extended_block_range_closed ON sgd1.expiry_extended USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: expiry_extended_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX expiry_extended_id_block_range_excl ON sgd1.expiry_extended USING gist (id, block_range);


--
-- Name: fuses_set_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX fuses_set_block_range_closed ON sgd1.fuses_set USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: fuses_set_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX fuses_set_id_block_range_excl ON sgd1.fuses_set USING gist (id, block_range);


--
-- Name: gist_block_range_data_sources$; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX "gist_block_range_data_sources$" ON sgd1."data_sources$" USING gist (block_range);


--
-- Name: interface_changed_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX interface_changed_block_range_closed ON sgd1.interface_changed USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: interface_changed_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX interface_changed_id_block_range_excl ON sgd1.interface_changed USING gist (id, block_range);


--
-- Name: multicoin_addr_changed_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX multicoin_addr_changed_block_range_closed ON sgd1.multicoin_addr_changed USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: multicoin_addr_changed_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX multicoin_addr_changed_id_block_range_excl ON sgd1.multicoin_addr_changed USING gist (id, block_range);


--
-- Name: name_changed_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX name_changed_block_range_closed ON sgd1.name_changed USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: name_changed_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX name_changed_id_block_range_excl ON sgd1.name_changed USING gist (id, block_range);


--
-- Name: name_registered_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX name_registered_block_range_closed ON sgd1.name_registered USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: name_registered_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX name_registered_id_block_range_excl ON sgd1.name_registered USING gist (id, block_range);


--
-- Name: name_renewed_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX name_renewed_block_range_closed ON sgd1.name_renewed USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: name_renewed_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX name_renewed_id_block_range_excl ON sgd1.name_renewed USING gist (id, block_range);


--
-- Name: name_transferred_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX name_transferred_block_range_closed ON sgd1.name_transferred USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: name_transferred_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX name_transferred_id_block_range_excl ON sgd1.name_transferred USING gist (id, block_range);


--
-- Name: name_unwrapped_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX name_unwrapped_block_range_closed ON sgd1.name_unwrapped USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: name_unwrapped_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX name_unwrapped_id_block_range_excl ON sgd1.name_unwrapped USING gist (id, block_range);


--
-- Name: name_wrapped_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX name_wrapped_block_range_closed ON sgd1.name_wrapped USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: name_wrapped_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX name_wrapped_id_block_range_excl ON sgd1.name_wrapped USING gist (id, block_range);


--
-- Name: new_owner_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX new_owner_block_range_closed ON sgd1.new_owner USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: new_owner_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX new_owner_id_block_range_excl ON sgd1.new_owner USING gist (id, block_range);


--
-- Name: new_resolver_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX new_resolver_block_range_closed ON sgd1.new_resolver USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: new_resolver_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX new_resolver_id_block_range_excl ON sgd1.new_resolver USING gist (id, block_range);


--
-- Name: new_ttl_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX new_ttl_block_range_closed ON sgd1.new_ttl USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: new_ttl_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX new_ttl_id_block_range_excl ON sgd1.new_ttl USING gist (id, block_range);


--
-- Name: poi2$_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX "poi2$_block_range_closed" ON sgd1."poi2$" USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: poi2$_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX "poi2$_id_block_range_excl" ON sgd1."poi2$" USING gist (id, block_range);


--
-- Name: pubkey_changed_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX pubkey_changed_block_range_closed ON sgd1.pubkey_changed USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: pubkey_changed_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX pubkey_changed_id_block_range_excl ON sgd1.pubkey_changed USING gist (id, block_range);


--
-- Name: registration_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX registration_block_range_closed ON sgd1.registration USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: registration_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX registration_id_block_range_excl ON sgd1.registration USING gist (id, block_range);


--
-- Name: resolver_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX resolver_block_range_closed ON sgd1.resolver USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: resolver_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX resolver_id_block_range_excl ON sgd1.resolver USING gist (id, block_range);


--
-- Name: text_changed_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX text_changed_block_range_closed ON sgd1.text_changed USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: text_changed_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX text_changed_id_block_range_excl ON sgd1.text_changed USING gist (id, block_range);


--
-- Name: transfer_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX transfer_block_range_closed ON sgd1.transfer USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: transfer_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX transfer_id_block_range_excl ON sgd1.transfer USING gist (id, block_range);


--
-- Name: version_changed_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX version_changed_block_range_closed ON sgd1.version_changed USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: version_changed_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX version_changed_id_block_range_excl ON sgd1.version_changed USING gist (id, block_range);


--
-- Name: wrapped_domain_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX wrapped_domain_block_range_closed ON sgd1.wrapped_domain USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: wrapped_domain_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX wrapped_domain_id_block_range_excl ON sgd1.wrapped_domain USING gist (id, block_range);


--
-- Name: wrapped_transfer_block_range_closed; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX wrapped_transfer_block_range_closed ON sgd1.wrapped_transfer USING btree (COALESCE(upper(block_range), 2147483647)) WHERE (COALESCE(upper(block_range), 2147483647) < 2147483647);


--
-- Name: wrapped_transfer_id_block_range_excl; Type: INDEX; Schema: sgd1; Owner: graph-node
--

CREATE INDEX wrapped_transfer_id_block_range_excl ON sgd1.wrapped_transfer USING gist (id, block_range);


--
-- Name: data_sources$ data_sources$_parent_fkey; Type: FK CONSTRAINT; Schema: sgd1; Owner: graph-node
--

ALTER TABLE ONLY sgd1."data_sources$"
    ADD CONSTRAINT "data_sources$_parent_fkey" FOREIGN KEY (parent) REFERENCES sgd1."data_sources$"(vid);


--
-- PostgreSQL database dump complete
--

