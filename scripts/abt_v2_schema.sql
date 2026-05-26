--
-- PostgreSQL database dump
--

\restrict 9s9pLQpMwGWyp7iXO27d9I5rbfJOg5lRumDqytaEOkcGCgbgzPh6CRCDiS6xe8o

-- Dumped from database version 17.9
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

-- *not* creating schema, since initdb creates it


--
-- Name: SCHEMA public; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON SCHEMA public IS '';


--
-- Name: pg_trgm; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public;


--
-- Name: EXTENSION pg_trgm; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON EXTENSION pg_trgm IS 'text similarity measurement and index searching based on trigrams';


SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: arrival_notice_items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.arrival_notice_items (
    id bigint NOT NULL,
    notice_id bigint NOT NULL,
    order_item_id bigint,
    product_id bigint NOT NULL,
    declared_qty numeric(10,6) NOT NULL,
    received_qty numeric(10,6) DEFAULT 0 NOT NULL,
    accepted_qty numeric(10,6) DEFAULT 0 NOT NULL,
    batch_no character varying(50)
);


--
-- Name: arrival_notice_items_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.arrival_notice_items_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: arrival_notice_items_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.arrival_notice_items_id_seq OWNED BY public.arrival_notice_items.id;


--
-- Name: arrival_notices; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.arrival_notices (
    id bigint NOT NULL,
    doc_number character varying(50) NOT NULL,
    purchase_order_id bigint,
    supplier_id bigint NOT NULL,
    arrival_date date NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    warehouse_id bigint NOT NULL,
    zone_id bigint,
    delivery_note text,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: arrival_notices_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.arrival_notices_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: arrival_notices_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.arrival_notices_id_seq OWNED BY public.arrival_notices.id;


--
-- Name: audit_logs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.audit_logs (
    id bigint NOT NULL,
    entity_type character varying(50) NOT NULL,
    entity_id bigint NOT NULL,
    action smallint NOT NULL,
    changes jsonb,
    operator_id bigint NOT NULL,
    context jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL
)
PARTITION BY RANGE (created_at);


--
-- Name: audit_logs_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.audit_logs_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: audit_logs_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.audit_logs_id_seq OWNED BY public.audit_logs.id;


--
-- Name: audit_logs_2026_01; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.audit_logs_2026_01 (
    id bigint DEFAULT nextval('public.audit_logs_id_seq'::regclass) NOT NULL,
    entity_type character varying(50) NOT NULL,
    entity_id bigint NOT NULL,
    action smallint NOT NULL,
    changes jsonb,
    operator_id bigint NOT NULL,
    context jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: audit_logs_2026_02; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.audit_logs_2026_02 (
    id bigint DEFAULT nextval('public.audit_logs_id_seq'::regclass) NOT NULL,
    entity_type character varying(50) NOT NULL,
    entity_id bigint NOT NULL,
    action smallint NOT NULL,
    changes jsonb,
    operator_id bigint NOT NULL,
    context jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: audit_logs_2026_03; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.audit_logs_2026_03 (
    id bigint DEFAULT nextval('public.audit_logs_id_seq'::regclass) NOT NULL,
    entity_type character varying(50) NOT NULL,
    entity_id bigint NOT NULL,
    action smallint NOT NULL,
    changes jsonb,
    operator_id bigint NOT NULL,
    context jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: audit_logs_2026_04; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.audit_logs_2026_04 (
    id bigint DEFAULT nextval('public.audit_logs_id_seq'::regclass) NOT NULL,
    entity_type character varying(50) NOT NULL,
    entity_id bigint NOT NULL,
    action smallint NOT NULL,
    changes jsonb,
    operator_id bigint NOT NULL,
    context jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: audit_logs_2026_05; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.audit_logs_2026_05 (
    id bigint DEFAULT nextval('public.audit_logs_id_seq'::regclass) NOT NULL,
    entity_type character varying(50) NOT NULL,
    entity_id bigint NOT NULL,
    action smallint NOT NULL,
    changes jsonb,
    operator_id bigint NOT NULL,
    context jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: audit_logs_2026_06; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.audit_logs_2026_06 (
    id bigint DEFAULT nextval('public.audit_logs_id_seq'::regclass) NOT NULL,
    entity_type character varying(50) NOT NULL,
    entity_id bigint NOT NULL,
    action smallint NOT NULL,
    changes jsonb,
    operator_id bigint NOT NULL,
    context jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: audit_logs_2026_07; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.audit_logs_2026_07 (
    id bigint DEFAULT nextval('public.audit_logs_id_seq'::regclass) NOT NULL,
    entity_type character varying(50) NOT NULL,
    entity_id bigint NOT NULL,
    action smallint NOT NULL,
    changes jsonb,
    operator_id bigint NOT NULL,
    context jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: audit_logs_2026_08; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.audit_logs_2026_08 (
    id bigint DEFAULT nextval('public.audit_logs_id_seq'::regclass) NOT NULL,
    entity_type character varying(50) NOT NULL,
    entity_id bigint NOT NULL,
    action smallint NOT NULL,
    changes jsonb,
    operator_id bigint NOT NULL,
    context jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: audit_logs_2026_09; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.audit_logs_2026_09 (
    id bigint DEFAULT nextval('public.audit_logs_id_seq'::regclass) NOT NULL,
    entity_type character varying(50) NOT NULL,
    entity_id bigint NOT NULL,
    action smallint NOT NULL,
    changes jsonb,
    operator_id bigint NOT NULL,
    context jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: audit_logs_2026_10; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.audit_logs_2026_10 (
    id bigint DEFAULT nextval('public.audit_logs_id_seq'::regclass) NOT NULL,
    entity_type character varying(50) NOT NULL,
    entity_id bigint NOT NULL,
    action smallint NOT NULL,
    changes jsonb,
    operator_id bigint NOT NULL,
    context jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: audit_logs_2026_11; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.audit_logs_2026_11 (
    id bigint DEFAULT nextval('public.audit_logs_id_seq'::regclass) NOT NULL,
    entity_type character varying(50) NOT NULL,
    entity_id bigint NOT NULL,
    action smallint NOT NULL,
    changes jsonb,
    operator_id bigint NOT NULL,
    context jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: audit_logs_2026_12; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.audit_logs_2026_12 (
    id bigint DEFAULT nextval('public.audit_logs_id_seq'::regclass) NOT NULL,
    entity_type character varying(50) NOT NULL,
    entity_id bigint NOT NULL,
    action smallint NOT NULL,
    changes jsonb,
    operator_id bigint NOT NULL,
    context jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: backflush_items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.backflush_items (
    id bigint NOT NULL,
    record_id bigint NOT NULL,
    component_id bigint NOT NULL,
    theoretical_qty numeric(10,6) NOT NULL,
    actual_qty numeric(10,6) NOT NULL,
    variance_qty numeric(10,6) NOT NULL,
    variance_rate numeric(10,6) NOT NULL,
    is_over_threshold boolean DEFAULT false NOT NULL
);


--
-- Name: backflush_items_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.backflush_items_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: backflush_items_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.backflush_items_id_seq OWNED BY public.backflush_items.id;


--
-- Name: backflush_records; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.backflush_records (
    id bigint NOT NULL,
    doc_number character varying(50) NOT NULL,
    work_order_id bigint NOT NULL,
    product_id bigint NOT NULL,
    completed_qty numeric(10,6) NOT NULL,
    backflush_date date NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    variance_threshold numeric(10,6) DEFAULT 0.05 NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: backflush_records_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.backflush_records_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: backflush_records_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.backflush_records_id_seq OWNED BY public.backflush_records.id;


--
-- Name: bins; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.bins (
    id bigint NOT NULL,
    zone_id bigint NOT NULL,
    code character varying(50) NOT NULL,
    name character varying(200) NOT NULL,
    row_no character varying(20),
    column_no character varying(20),
    layer_no character varying(20),
    capacity_limit numeric(10,6),
    allowed_product_types text[],
    temperature_req character varying(50),
    status smallint DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: bins_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.bins_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: bins_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.bins_id_seq OWNED BY public.bins.id;


--
-- Name: bom_categories; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.bom_categories (
    bom_category_id bigint NOT NULL,
    bom_category_name character varying(255) NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: bom_categories_bom_category_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.bom_categories_bom_category_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: bom_categories_bom_category_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.bom_categories_bom_category_id_seq OWNED BY public.bom_categories.bom_category_id;


--
-- Name: bom_labor_processes; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.bom_labor_processes (
    id bigint NOT NULL,
    product_code character varying(100) NOT NULL,
    labor_process_dict_id bigint NOT NULL,
    process_code character varying(100),
    name character varying(255) NOT NULL,
    unit_price numeric(20,4) DEFAULT 0 NOT NULL,
    quantity numeric(18,6) DEFAULT 0 NOT NULL,
    sort_order integer DEFAULT 0 NOT NULL,
    remark text,
    operator_id bigint,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone,
    deleted_at timestamp with time zone
);


--
-- Name: bom_labor_processes_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.bom_labor_processes_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: bom_labor_processes_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.bom_labor_processes_id_seq OWNED BY public.bom_labor_processes.id;


--
-- Name: bom_nodes; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.bom_nodes (
    node_id bigint NOT NULL,
    bom_id bigint NOT NULL,
    product_id bigint NOT NULL,
    product_code character varying(100),
    quantity numeric(18,6) DEFAULT 0 NOT NULL,
    parent_id bigint DEFAULT 0 NOT NULL,
    loss_rate numeric(10,4) DEFAULT 0 NOT NULL,
    order_num integer DEFAULT 0 NOT NULL,
    unit character varying(50),
    remark text,
    "position" character varying(100),
    work_center character varying(100),
    properties text
);


--
-- Name: bom_nodes_node_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.bom_nodes_node_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: bom_nodes_node_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.bom_nodes_node_id_seq OWNED BY public.bom_nodes.node_id;


--
-- Name: bom_routings; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.bom_routings (
    id bigint NOT NULL,
    product_code character varying(100) NOT NULL,
    routing_id bigint NOT NULL,
    operator_id bigint,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone
);


--
-- Name: bom_routings_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.bom_routings_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: bom_routings_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.bom_routings_id_seq OWNED BY public.bom_routings.id;


--
-- Name: bom_snapshots; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.bom_snapshots (
    snapshot_id bigint NOT NULL,
    bom_id bigint NOT NULL,
    version integer NOT NULL,
    bom_name character varying(255) NOT NULL,
    bom_detail jsonb NOT NULL,
    published_at timestamp with time zone NOT NULL,
    published_by bigint NOT NULL
);


--
-- Name: bom_snapshots_snapshot_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.bom_snapshots_snapshot_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: bom_snapshots_snapshot_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.bom_snapshots_snapshot_id_seq OWNED BY public.bom_snapshots.snapshot_id;


--
-- Name: boms; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.boms (
    bom_id bigint NOT NULL,
    bom_name character varying(255) NOT NULL,
    create_at timestamp with time zone DEFAULT now() NOT NULL,
    update_at timestamp with time zone,
    bom_detail jsonb DEFAULT '{"nodes": []}'::jsonb NOT NULL,
    bom_category_id bigint,
    status smallint DEFAULT 1 NOT NULL,
    version integer DEFAULT 1 NOT NULL,
    published_at timestamp with time zone,
    created_by bigint,
    deleted_at timestamp with time zone
);


--
-- Name: boms_bom_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.boms_bom_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: boms_bom_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.boms_bom_id_seq OWNED BY public.boms.bom_id;


--
-- Name: cash_journal_lines; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.cash_journal_lines (
    id bigint NOT NULL,
    journal_id bigint NOT NULL,
    account_code character varying(32) NOT NULL,
    debit_amount numeric(20,4) DEFAULT 0 NOT NULL,
    credit_amount numeric(20,4) DEFAULT 0 NOT NULL,
    cost_center bigint,
    profit_center bigint,
    remark text DEFAULT ''::text NOT NULL
);


--
-- Name: cash_journal_lines_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.cash_journal_lines_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: cash_journal_lines_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.cash_journal_lines_id_seq OWNED BY public.cash_journal_lines.id;


--
-- Name: cash_journals; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.cash_journals (
    id bigint NOT NULL,
    doc_number character varying(32) NOT NULL,
    journal_type smallint NOT NULL,
    direction smallint NOT NULL,
    amount numeric(20,4) NOT NULL,
    counterparty_type smallint NOT NULL,
    counterparty_id bigint NOT NULL,
    source_type smallint NOT NULL,
    source_id bigint NOT NULL,
    bank_account character varying(64) DEFAULT ''::character varying NOT NULL,
    transaction_date date NOT NULL,
    period character varying(7) NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    version integer DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: cash_journals_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.cash_journals_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: cash_journals_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.cash_journals_id_seq OWNED BY public.cash_journals.id;


--
-- Name: categories; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.categories (
    category_id bigint NOT NULL,
    category_name character varying(200) NOT NULL,
    parent_id bigint DEFAULT 0 NOT NULL,
    path character varying(1000) DEFAULT '/'::character varying NOT NULL,
    meta jsonb DEFAULT '{"count": 0}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone
);


--
-- Name: categories_category_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.categories_category_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: categories_category_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.categories_category_id_seq OWNED BY public.categories.category_id;


--
-- Name: conversion_items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.conversion_items (
    id bigint NOT NULL,
    conversion_id bigint NOT NULL,
    direction smallint NOT NULL,
    product_id bigint NOT NULL,
    quantity numeric(10,6) NOT NULL,
    unit_cost numeric(10,6) NOT NULL,
    batch_no character varying(50)
);


--
-- Name: conversion_items_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.conversion_items_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: conversion_items_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.conversion_items_id_seq OWNED BY public.conversion_items.id;


--
-- Name: cost_entries; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.cost_entries (
    id bigint NOT NULL,
    entity_type smallint NOT NULL,
    entity_id bigint NOT NULL,
    cost_type smallint NOT NULL,
    debit_amount numeric(20,4) DEFAULT 0 NOT NULL,
    credit_amount numeric(20,4) DEFAULT 0 NOT NULL,
    cost_center bigint,
    profit_center bigint,
    period character varying(7) NOT NULL,
    source_type smallint NOT NULL,
    source_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: cost_entries_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.cost_entries_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: cost_entries_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.cost_entries_id_seq OWNED BY public.cost_entries.id;


--
-- Name: customer_addresses; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.customer_addresses (
    address_id bigint NOT NULL,
    customer_id bigint NOT NULL,
    address_type character varying(50) NOT NULL,
    province character varying(100) NOT NULL,
    city character varying(100) NOT NULL,
    district character varying(100),
    detail text NOT NULL,
    contact_name character varying(100),
    contact_phone character varying(50),
    is_default boolean DEFAULT false NOT NULL
);


--
-- Name: customer_addresses_address_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.customer_addresses_address_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: customer_addresses_address_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.customer_addresses_address_id_seq OWNED BY public.customer_addresses.address_id;


--
-- Name: customer_contacts; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.customer_contacts (
    contact_id bigint NOT NULL,
    customer_id bigint NOT NULL,
    contact_name character varying(100) NOT NULL,
    "position" character varying(100),
    phone character varying(50),
    email character varying(100),
    is_primary boolean DEFAULT false NOT NULL
);


--
-- Name: customer_contacts_contact_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.customer_contacts_contact_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: customer_contacts_contact_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.customer_contacts_contact_id_seq OWNED BY public.customer_contacts.contact_id;


--
-- Name: customers; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.customers (
    customer_id bigint NOT NULL,
    customer_code character varying(100) NOT NULL,
    customer_name character varying(255) NOT NULL,
    short_name character varying(100),
    category smallint NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    tax_number character varying(50),
    invoice_title character varying(255),
    credit_limit numeric(20,4),
    payment_terms text,
    receivable_account character varying(100),
    owner_id bigint,
    department_id bigint,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: customers_customer_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.customers_customer_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: customers_customer_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.customers_customer_id_seq OWNED BY public.customers.customer_id;


--
-- Name: cycle_count_items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.cycle_count_items (
    id bigint NOT NULL,
    count_id bigint NOT NULL,
    bin_id bigint NOT NULL,
    product_id bigint NOT NULL,
    batch_no character varying(50),
    system_qty numeric(10,6) NOT NULL,
    counted_qty numeric(10,6) DEFAULT 0 NOT NULL,
    variance_qty numeric(10,6) DEFAULT 0 NOT NULL,
    variance_reason text,
    is_adjusted boolean DEFAULT false NOT NULL
);


--
-- Name: cycle_count_items_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.cycle_count_items_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: cycle_count_items_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.cycle_count_items_id_seq OWNED BY public.cycle_count_items.id;


--
-- Name: cycle_counts; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.cycle_counts (
    id bigint NOT NULL,
    doc_number character varying(50) NOT NULL,
    warehouse_id bigint NOT NULL,
    zone_id bigint,
    count_date date NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    is_blind boolean DEFAULT false NOT NULL,
    remark text,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: cycle_counts_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.cycle_counts_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: cycle_counts_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.cycle_counts_id_seq OWNED BY public.cycle_counts.id;


--
-- Name: departments; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.departments (
    department_id bigint NOT NULL,
    department_name character varying(100) NOT NULL,
    department_code character varying(50) NOT NULL,
    description character varying(255),
    is_active boolean DEFAULT true NOT NULL,
    is_default boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone
);


--
-- Name: departments_department_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.departments_department_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: departments_department_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.departments_department_id_seq OWNED BY public.departments.department_id;


--
-- Name: document_links; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.document_links (
    id bigint NOT NULL,
    source_type smallint NOT NULL,
    source_id bigint NOT NULL,
    target_type smallint NOT NULL,
    target_id bigint NOT NULL,
    link_type smallint NOT NULL,
    path character varying(255) DEFAULT ''::character varying NOT NULL,
    depth integer DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by bigint
);


--
-- Name: document_links_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.document_links_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: document_links_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.document_links_id_seq OWNED BY public.document_links.id;


--
-- Name: document_sequences; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.document_sequences (
    id bigint NOT NULL,
    prefix character varying(20) NOT NULL,
    current_value integer DEFAULT 0 NOT NULL,
    seq_date date NOT NULL,
    padding_len integer DEFAULT 4 NOT NULL,
    strategy smallint DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: document_sequences_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.document_sequences_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: document_sequences_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.document_sequences_id_seq OWNED BY public.document_sequences.id;


--
-- Name: domain_events; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.domain_events (
    id bigint NOT NULL,
    event_type smallint NOT NULL,
    event_version integer DEFAULT 1 NOT NULL,
    aggregate_type character varying(50) NOT NULL,
    aggregate_id bigint NOT NULL,
    payload jsonb DEFAULT '{}'::jsonb NOT NULL,
    operator_id bigint NOT NULL,
    idempotency_key character varying(255) NOT NULL,
    trace_id character varying(255),
    request_id character varying(255),
    status smallint DEFAULT 1 NOT NULL,
    retry_count integer DEFAULT 0 NOT NULL,
    failure_reason text,
    processed_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: domain_events_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.domain_events_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: domain_events_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.domain_events_id_seq OWNED BY public.domain_events.id;


--
-- Name: entity_state_logs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.entity_state_logs (
    id bigint NOT NULL,
    entity_type character varying(50) NOT NULL,
    entity_id bigint NOT NULL,
    from_state character varying(50),
    to_state character varying(50) NOT NULL,
    transition_id bigint NOT NULL,
    operator_id bigint NOT NULL,
    remark character varying(500),
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: entity_state_logs_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.entity_state_logs_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: entity_state_logs_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.entity_state_logs_id_seq OWNED BY public.entity_state_logs.id;


--
-- Name: expense_reimbursement_items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.expense_reimbursement_items (
    id bigint NOT NULL,
    reimbursement_id bigint NOT NULL,
    expense_type smallint NOT NULL,
    amount numeric(20,4) NOT NULL,
    description text DEFAULT ''::text NOT NULL,
    receipt_no character varying(64),
    cost_center bigint,
    profit_center bigint
);


--
-- Name: expense_reimbursement_items_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.expense_reimbursement_items_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: expense_reimbursement_items_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.expense_reimbursement_items_id_seq OWNED BY public.expense_reimbursement_items.id;


--
-- Name: expense_reimbursements; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.expense_reimbursements (
    id bigint NOT NULL,
    doc_number character varying(32) NOT NULL,
    applicant_id bigint NOT NULL,
    department_id bigint,
    expense_date date NOT NULL,
    total_amount numeric(20,4) NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    version integer DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: expense_reimbursements_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.expense_reimbursements_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: expense_reimbursements_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.expense_reimbursements_id_seq OWNED BY public.expense_reimbursements.id;


--
-- Name: form_conversions; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.form_conversions (
    id bigint NOT NULL,
    doc_number character varying(50) NOT NULL,
    warehouse_id bigint NOT NULL,
    conversion_date date NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: form_conversions_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.form_conversions_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: form_conversions_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.form_conversions_id_seq OWNED BY public.form_conversions.id;


--
-- Name: idempotency_records; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.idempotency_records (
    id bigint NOT NULL,
    idempotency_key character varying(255) NOT NULL,
    event_id bigint NOT NULL,
    handler_name character varying(100) NOT NULL,
    status character varying(20) DEFAULT 'Processing'::character varying NOT NULL,
    result jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    expires_at timestamp with time zone
);


--
-- Name: idempotency_records_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.idempotency_records_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: idempotency_records_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.idempotency_records_id_seq OWNED BY public.idempotency_records.id;


--
-- Name: inspection_results; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.inspection_results (
    id bigint NOT NULL,
    doc_number character varying(30) NOT NULL,
    spec_id bigint NOT NULL,
    source_type smallint NOT NULL,
    source_id bigint NOT NULL,
    inspection_type smallint NOT NULL,
    batch_no character varying(80) DEFAULT ''::character varying NOT NULL,
    sample_qty numeric(18,6) DEFAULT 0 NOT NULL,
    qualified_qty numeric(18,6) DEFAULT 0 NOT NULL,
    unqualified_qty numeric(18,6) DEFAULT 0 NOT NULL,
    result smallint DEFAULT 1 NOT NULL,
    check_results jsonb DEFAULT '[]'::jsonb NOT NULL,
    inspector_id bigint DEFAULT 0 NOT NULL,
    inspection_date date,
    status smallint DEFAULT 1 NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: inspection_results_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.inspection_results_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: inspection_results_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.inspection_results_id_seq OWNED BY public.inspection_results.id;


--
-- Name: inspection_specifications; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.inspection_specifications (
    id bigint NOT NULL,
    doc_number character varying(30) NOT NULL,
    product_id bigint NOT NULL,
    inspection_type smallint NOT NULL,
    check_items jsonb DEFAULT '[]'::jsonb NOT NULL,
    sample_plan jsonb DEFAULT '{}'::jsonb NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    version integer DEFAULT 1 NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: inspection_specifications_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.inspection_specifications_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: inspection_specifications_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.inspection_specifications_id_seq OWNED BY public.inspection_specifications.id;


--
-- Name: inventory_locks; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.inventory_locks (
    id bigint NOT NULL,
    doc_number character varying(50) NOT NULL,
    product_id bigint NOT NULL,
    warehouse_id bigint NOT NULL,
    locked_qty numeric(10,6) NOT NULL,
    lock_reason text NOT NULL,
    customer_id bigint,
    status smallint DEFAULT 1 NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: inventory_locks_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.inventory_locks_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: inventory_locks_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.inventory_locks_id_seq OWNED BY public.inventory_locks.id;


--
-- Name: inventory_reservations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.inventory_reservations (
    id bigint NOT NULL,
    product_id bigint NOT NULL,
    warehouse_id bigint NOT NULL,
    reserved_qty numeric(18,6) NOT NULL,
    reservation_type smallint NOT NULL,
    source_type smallint NOT NULL,
    source_id bigint NOT NULL,
    source_line_id bigint,
    status smallint DEFAULT 1 NOT NULL,
    priority integer DEFAULT 5 NOT NULL,
    expires_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: inventory_reservations_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.inventory_reservations_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: inventory_reservations_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.inventory_reservations_id_seq OWNED BY public.inventory_reservations.id;


--
-- Name: inventory_transactions; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.inventory_transactions (
    id bigint NOT NULL,
    doc_number character varying(50),
    transaction_type smallint NOT NULL,
    product_id bigint NOT NULL,
    warehouse_id bigint NOT NULL,
    zone_id bigint,
    bin_id bigint,
    batch_no character varying(50),
    quantity numeric(10,6) NOT NULL,
    unit_cost numeric(10,6),
    source_type character varying(50) NOT NULL,
    source_id bigint NOT NULL,
    remark text,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: inventory_transactions_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.inventory_transactions_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: inventory_transactions_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.inventory_transactions_id_seq OWNED BY public.inventory_transactions.id;


--
-- Name: inventory_transfers; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.inventory_transfers (
    id bigint NOT NULL,
    doc_number character varying(50) NOT NULL,
    from_warehouse_id bigint NOT NULL,
    from_zone_id bigint,
    from_bin_id bigint,
    to_warehouse_id bigint NOT NULL,
    to_zone_id bigint,
    to_bin_id bigint,
    transfer_date date NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: inventory_transfers_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.inventory_transfers_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: inventory_transfers_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.inventory_transfers_id_seq OWNED BY public.inventory_transfers.id;


--
-- Name: labor_process_dicts; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.labor_process_dicts (
    id bigint NOT NULL,
    code character varying(100) NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    sort_order integer DEFAULT 0 NOT NULL,
    operator_id bigint,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone,
    deleted_at timestamp with time zone
);


--
-- Name: labor_process_dicts_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.labor_process_dicts_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: labor_process_dicts_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.labor_process_dicts_id_seq OWNED BY public.labor_process_dicts.id;


--
-- Name: material_requisition_items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.material_requisition_items (
    id bigint NOT NULL,
    requisition_id bigint NOT NULL,
    product_id bigint NOT NULL,
    requested_qty numeric(10,6) NOT NULL,
    issued_qty numeric(10,6) DEFAULT 0 NOT NULL,
    variance_qty numeric(10,6) DEFAULT 0 NOT NULL,
    bin_id bigint
);


--
-- Name: material_requisition_items_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.material_requisition_items_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: material_requisition_items_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.material_requisition_items_id_seq OWNED BY public.material_requisition_items.id;


--
-- Name: material_requisitions; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.material_requisitions (
    id bigint NOT NULL,
    doc_number character varying(50) NOT NULL,
    work_order_id bigint NOT NULL,
    requisition_date date NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    warehouse_id bigint NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: material_requisitions_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.material_requisitions_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: material_requisitions_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.material_requisitions_id_seq OWNED BY public.material_requisitions.id;


--
-- Name: misc_request_items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.misc_request_items (
    id bigint NOT NULL,
    request_id bigint NOT NULL,
    line_no integer NOT NULL,
    item_name text NOT NULL,
    specification text,
    quantity numeric(18,6) NOT NULL,
    unit character varying(16) NOT NULL,
    estimated_price numeric(18,6),
    remark text
);


--
-- Name: misc_request_items_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.misc_request_items_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: misc_request_items_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.misc_request_items_id_seq OWNED BY public.misc_request_items.id;


--
-- Name: miscellaneous_requests; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.miscellaneous_requests (
    id bigint NOT NULL,
    doc_number character varying(32) NOT NULL,
    department_id bigint NOT NULL,
    request_date date NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    total_amount numeric(20,4) DEFAULT 0 NOT NULL,
    purpose text NOT NULL,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: miscellaneous_requests_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.miscellaneous_requests_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: miscellaneous_requests_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.miscellaneous_requests_id_seq OWNED BY public.miscellaneous_requests.id;


--
-- Name: mrbs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.mrbs (
    id bigint NOT NULL,
    doc_number character varying(30) NOT NULL,
    inspection_result_id bigint NOT NULL,
    product_id bigint NOT NULL,
    defect_description text DEFAULT ''::text NOT NULL,
    disposition smallint DEFAULT 1 NOT NULL,
    responsible_party smallint DEFAULT 1 NOT NULL,
    cost_impact numeric(20,4) DEFAULT 0 NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: mrbs_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.mrbs_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: mrbs_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.mrbs_id_seq OWNED BY public.mrbs.id;


--
-- Name: notifications; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.notifications (
    notification_id bigint NOT NULL,
    user_id bigint NOT NULL,
    notification_type smallint NOT NULL,
    title character varying(255) NOT NULL,
    content text,
    related_type character varying(100),
    related_id bigint,
    is_read boolean DEFAULT false NOT NULL,
    read_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: notifications_notification_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.notifications_notification_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: notifications_notification_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.notifications_notification_id_seq OWNED BY public.notifications.notification_id;


--
-- Name: outsourcing_materials; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.outsourcing_materials (
    id bigint NOT NULL,
    outsourcing_id bigint NOT NULL,
    product_id bigint NOT NULL,
    planned_qty numeric(18,6) DEFAULT 0 NOT NULL,
    sent_qty numeric(18,6) DEFAULT 0 NOT NULL,
    returned_qty numeric(18,6) DEFAULT 0 NOT NULL,
    unit_cost numeric(18,6) DEFAULT 0 NOT NULL
);


--
-- Name: outsourcing_materials_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.outsourcing_materials_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: outsourcing_materials_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.outsourcing_materials_id_seq OWNED BY public.outsourcing_materials.id;


--
-- Name: outsourcing_orders; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.outsourcing_orders (
    id bigint NOT NULL,
    doc_number text NOT NULL,
    work_order_id bigint,
    routing_id bigint,
    supplier_id bigint NOT NULL,
    product_id bigint NOT NULL,
    outsourcing_type smallint DEFAULT 1 NOT NULL,
    planned_qty numeric(18,6) DEFAULT 0 NOT NULL,
    completed_qty numeric(18,6) DEFAULT 0 NOT NULL,
    unit_price numeric(18,6) DEFAULT 0 NOT NULL,
    scheduled_date date,
    status smallint DEFAULT 1 NOT NULL,
    virtual_warehouse_id bigint NOT NULL,
    version integer DEFAULT 1 NOT NULL,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: outsourcing_orders_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.outsourcing_orders_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: outsourcing_orders_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.outsourcing_orders_id_seq OWNED BY public.outsourcing_orders.id;


--
-- Name: outsourcing_trackings; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.outsourcing_trackings (
    id bigint NOT NULL,
    outsourcing_id bigint NOT NULL,
    node_type smallint NOT NULL,
    tracked_at timestamp with time zone,
    planned_at timestamp with time zone,
    remark text,
    operator_id bigint DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: outsourcing_trackings_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.outsourcing_trackings_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: outsourcing_trackings_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.outsourcing_trackings_id_seq OWNED BY public.outsourcing_trackings.id;


--
-- Name: payment_requests; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.payment_requests (
    id bigint NOT NULL,
    doc_number character varying(32) NOT NULL,
    supplier_id bigint NOT NULL,
    reconciliation_id bigint,
    payment_date date NOT NULL,
    amount numeric(20,4) NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    payment_method smallint NOT NULL,
    bank_account_id bigint,
    invoice_number character varying(64),
    invoice_amount numeric(20,4),
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: payment_requests_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.payment_requests_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: payment_requests_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.payment_requests_id_seq OWNED BY public.payment_requests.id;


--
-- Name: pick_strategies; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.pick_strategies (
    id bigint NOT NULL,
    name character varying(200) NOT NULL,
    strategy_type smallint NOT NULL,
    warehouse_id bigint,
    priority integer DEFAULT 0 NOT NULL,
    is_active boolean DEFAULT true NOT NULL
);


--
-- Name: pick_strategies_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.pick_strategies_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: pick_strategies_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.pick_strategies_id_seq OWNED BY public.pick_strategies.id;


--
-- Name: price_log; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.price_log (
    log_id bigint NOT NULL,
    product_id bigint NOT NULL,
    price_type smallint NOT NULL,
    old_price numeric(20,4),
    new_price numeric(20,4) NOT NULL,
    operator_id bigint,
    remark text DEFAULT ''::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: price_log_log_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.price_log_log_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: price_log_log_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.price_log_log_id_seq OWNED BY public.price_log.log_id;


--
-- Name: product_categories; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.product_categories (
    product_id bigint NOT NULL,
    category_id bigint NOT NULL
);


--
-- Name: product_watchers; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.product_watchers (
    user_id bigint NOT NULL,
    product_id bigint NOT NULL,
    safety_stock_override numeric(18,6),
    alert_active boolean DEFAULT false NOT NULL,
    last_notified_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: TABLE product_watchers; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.product_watchers IS '用户关注的产品列表';


--
-- Name: COLUMN product_watchers.safety_stock_override; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.product_watchers.safety_stock_override IS '用户自定义告警阈值，NULL 则使用 stock_ledger.safety_stock';


--
-- Name: COLUMN product_watchers.alert_active; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.product_watchers.alert_active IS '当前是否处于活跃告警状态（库存低于阈值且已发送通知）';


--
-- Name: COLUMN product_watchers.last_notified_at; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.product_watchers.last_notified_at IS '上次发送库存告警通知的时间';


--
-- Name: production_batches; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.production_batches (
    id bigint NOT NULL,
    batch_no character varying(80) NOT NULL,
    card_sn character varying(80) NOT NULL,
    work_order_id bigint NOT NULL,
    product_id bigint NOT NULL,
    batch_qty numeric(18,6) NOT NULL,
    completed_qty numeric(18,6) DEFAULT 0 NOT NULL,
    scrap_qty numeric(18,6) DEFAULT 0 NOT NULL,
    team_id bigint,
    current_step integer DEFAULT 0 NOT NULL,
    actual_start timestamp with time zone,
    actual_end timestamp with time zone,
    status smallint DEFAULT 1 NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: production_batches_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.production_batches_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: production_batches_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.production_batches_id_seq OWNED BY public.production_batches.id;


--
-- Name: production_inspections; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.production_inspections (
    id bigint NOT NULL,
    doc_number character varying(50) NOT NULL,
    work_order_id bigint NOT NULL,
    routing_id bigint,
    product_id bigint NOT NULL,
    inspection_type smallint NOT NULL,
    sample_qty numeric(18,6) DEFAULT 0 NOT NULL,
    qualified_qty numeric(18,6) DEFAULT 0 NOT NULL,
    unqualified_qty numeric(18,6) DEFAULT 0 NOT NULL,
    result smallint DEFAULT 1 NOT NULL,
    inspector_id bigint DEFAULT 0 NOT NULL,
    inspection_date date NOT NULL,
    disposition text,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: production_inspections_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.production_inspections_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: production_inspections_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.production_inspections_id_seq OWNED BY public.production_inspections.id;


--
-- Name: production_plan_items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.production_plan_items (
    id bigint NOT NULL,
    plan_id bigint NOT NULL,
    product_id bigint NOT NULL,
    planned_qty numeric(18,6) NOT NULL,
    scheduled_start date NOT NULL,
    scheduled_end date NOT NULL,
    sales_order_id bigint,
    sales_order_item_id bigint,
    bom_snapshot_id bigint,
    routing_id bigint,
    work_center_id bigint,
    priority integer DEFAULT 0 NOT NULL,
    status smallint DEFAULT 1 NOT NULL
);


--
-- Name: production_plan_items_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.production_plan_items_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: production_plan_items_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.production_plan_items_id_seq OWNED BY public.production_plan_items.id;


--
-- Name: production_plans; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.production_plans (
    id bigint NOT NULL,
    doc_number character varying(50) NOT NULL,
    plan_date date NOT NULL,
    plan_type smallint NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: production_plans_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.production_plans_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: production_plans_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.production_plans_id_seq OWNED BY public.production_plans.id;


--
-- Name: production_receipts; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.production_receipts (
    id bigint NOT NULL,
    doc_number character varying(50) NOT NULL,
    work_order_id bigint NOT NULL,
    batch_id bigint,
    product_id bigint NOT NULL,
    received_qty numeric(18,6) NOT NULL,
    warehouse_id bigint NOT NULL,
    zone_id bigint,
    bin_id bigint,
    receipt_date date NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    backflush_triggered boolean DEFAULT false NOT NULL,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: production_receipts_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.production_receipts_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: production_receipts_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.production_receipts_id_seq OWNED BY public.production_receipts.id;


--
-- Name: products; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.products (
    product_id bigint NOT NULL,
    pdt_name character varying(255) NOT NULL,
    product_code character varying(100) NOT NULL,
    unit character varying(50) NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    external_code character varying(100),
    owner_department_id bigint,
    meta jsonb DEFAULT '{"specification": "", "acquire_channel": ""}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone,
    deleted_at timestamp with time zone
);


--
-- Name: products_product_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.products_product_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: products_product_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.products_product_id_seq OWNED BY public.products.product_id;


--
-- Name: purchase_order_items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.purchase_order_items (
    id bigint NOT NULL,
    order_id bigint NOT NULL,
    line_no integer NOT NULL,
    product_id bigint NOT NULL,
    description text DEFAULT ''::text NOT NULL,
    quantity numeric(18,6) NOT NULL,
    unit_price numeric(18,6) NOT NULL,
    amount numeric(20,4) NOT NULL,
    received_qty numeric(18,6) DEFAULT 0 NOT NULL,
    inspected_qty numeric(18,6) DEFAULT 0 NOT NULL,
    returned_qty numeric(18,6) DEFAULT 0 NOT NULL,
    quotation_item_id bigint,
    expected_delivery_date date
);


--
-- Name: purchase_order_items_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.purchase_order_items_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: purchase_order_items_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.purchase_order_items_id_seq OWNED BY public.purchase_order_items.id;


--
-- Name: purchase_orders; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.purchase_orders (
    id bigint NOT NULL,
    doc_number character varying(32) NOT NULL,
    supplier_id bigint NOT NULL,
    order_date date NOT NULL,
    expected_delivery_date date,
    status smallint DEFAULT 1 NOT NULL,
    total_amount numeric(20,4) DEFAULT 0 NOT NULL,
    payment_terms text,
    delivery_address text,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: purchase_orders_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.purchase_orders_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: purchase_orders_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.purchase_orders_id_seq OWNED BY public.purchase_orders.id;


--
-- Name: purchase_quotation_items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.purchase_quotation_items (
    id bigint NOT NULL,
    quotation_id bigint NOT NULL,
    product_id bigint NOT NULL,
    line_no integer NOT NULL,
    unit_price numeric(18,6) NOT NULL,
    min_order_qty numeric(18,6),
    lead_time_days integer,
    currency character varying(3) DEFAULT 'CNY'::character varying NOT NULL,
    is_preferred boolean DEFAULT false NOT NULL
);


--
-- Name: purchase_quotation_items_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.purchase_quotation_items_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: purchase_quotation_items_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.purchase_quotation_items_id_seq OWNED BY public.purchase_quotation_items.id;


--
-- Name: purchase_quotations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.purchase_quotations (
    id bigint NOT NULL,
    doc_number character varying(32) NOT NULL,
    supplier_id bigint NOT NULL,
    quotation_date date NOT NULL,
    valid_from date NOT NULL,
    valid_until date NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: purchase_quotations_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.purchase_quotations_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: purchase_quotations_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.purchase_quotations_id_seq OWNED BY public.purchase_quotations.id;


--
-- Name: purchase_recon_items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.purchase_recon_items (
    id bigint NOT NULL,
    reconciliation_id bigint NOT NULL,
    order_id bigint NOT NULL,
    order_item_id bigint NOT NULL,
    received_qty numeric(18,6) NOT NULL,
    returned_qty numeric(18,6) DEFAULT 0 NOT NULL,
    returned_amount numeric(20,4) DEFAULT 0 NOT NULL,
    unit_price numeric(18,6) NOT NULL,
    amount numeric(20,4) NOT NULL,
    confirmed boolean DEFAULT false NOT NULL
);


--
-- Name: purchase_recon_items_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.purchase_recon_items_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: purchase_recon_items_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.purchase_recon_items_id_seq OWNED BY public.purchase_recon_items.id;


--
-- Name: purchase_reconciliations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.purchase_reconciliations (
    id bigint NOT NULL,
    doc_number character varying(32) NOT NULL,
    supplier_id bigint NOT NULL,
    period character varying(7) NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    total_amount numeric(20,4) DEFAULT 0 NOT NULL,
    confirmed_amount numeric(20,4) DEFAULT 0 NOT NULL,
    difference numeric(20,4) DEFAULT 0 NOT NULL,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: purchase_reconciliations_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.purchase_reconciliations_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: purchase_reconciliations_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.purchase_reconciliations_id_seq OWNED BY public.purchase_reconciliations.id;


--
-- Name: purchase_return_items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.purchase_return_items (
    id bigint NOT NULL,
    return_id bigint NOT NULL,
    order_item_id bigint NOT NULL,
    product_id bigint NOT NULL,
    returned_qty numeric(18,6) NOT NULL,
    unit_price numeric(18,6) NOT NULL,
    amount numeric(20,4) NOT NULL
);


--
-- Name: purchase_return_items_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.purchase_return_items_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: purchase_return_items_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.purchase_return_items_id_seq OWNED BY public.purchase_return_items.id;


--
-- Name: purchase_returns; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.purchase_returns (
    id bigint NOT NULL,
    doc_number character varying(32) NOT NULL,
    order_id bigint NOT NULL,
    supplier_id bigint NOT NULL,
    return_date date NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    return_reason text NOT NULL,
    total_amount numeric(20,4) DEFAULT 0 NOT NULL,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: purchase_returns_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.purchase_returns_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: purchase_returns_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.purchase_returns_id_seq OWNED BY public.purchase_returns.id;


--
-- Name: putaway_strategies; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.putaway_strategies (
    id bigint NOT NULL,
    name character varying(200) NOT NULL,
    strategy_type smallint NOT NULL,
    warehouse_id bigint,
    product_category_id bigint,
    priority integer DEFAULT 0 NOT NULL,
    is_active boolean DEFAULT true NOT NULL
);


--
-- Name: putaway_strategies_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.putaway_strategies_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: putaway_strategies_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.putaway_strategies_id_seq OWNED BY public.putaway_strategies.id;


--
-- Name: quotation_items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.quotation_items (
    id bigint NOT NULL,
    quotation_id bigint NOT NULL,
    line_no integer NOT NULL,
    product_id bigint NOT NULL,
    description text DEFAULT ''::text NOT NULL,
    quantity numeric(18,6) NOT NULL,
    unit character varying(20) DEFAULT ''::character varying NOT NULL,
    unit_price numeric(18,6) NOT NULL,
    unit_cost numeric(18,6) DEFAULT 0 NOT NULL,
    discount_rate numeric(5,2) DEFAULT 0 NOT NULL,
    amount numeric(20,4) NOT NULL,
    delivery_date date
);


--
-- Name: quotation_items_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.quotation_items_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: quotation_items_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.quotation_items_id_seq OWNED BY public.quotation_items.id;


--
-- Name: quotations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.quotations (
    id bigint NOT NULL,
    doc_number character varying(30) NOT NULL,
    customer_id bigint NOT NULL,
    contact_id bigint NOT NULL,
    sales_rep_id bigint NOT NULL,
    quotation_date date DEFAULT CURRENT_DATE NOT NULL,
    valid_until date NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    total_amount numeric(20,4) DEFAULT 0 NOT NULL,
    total_cost numeric(20,4) DEFAULT 0 NOT NULL,
    estimated_margin numeric(5,2) DEFAULT 0 NOT NULL,
    payment_terms character varying(100) DEFAULT ''::character varying NOT NULL,
    delivery_terms character varying(100) DEFAULT ''::character varying NOT NULL,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: quotations_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.quotations_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: quotations_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.quotations_id_seq OWNED BY public.quotations.id;


--
-- Name: reconciliation_items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.reconciliation_items (
    id bigint NOT NULL,
    reconciliation_id bigint NOT NULL,
    shipping_request_id bigint NOT NULL,
    sales_order_id bigint NOT NULL,
    product_id bigint NOT NULL,
    quantity numeric(18,6) NOT NULL,
    unit_price numeric(18,6) NOT NULL,
    amount numeric(20,4) NOT NULL,
    confirmed boolean DEFAULT false NOT NULL,
    remark text
);


--
-- Name: reconciliation_items_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.reconciliation_items_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: reconciliation_items_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.reconciliation_items_id_seq OWNED BY public.reconciliation_items.id;


--
-- Name: reconciliations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.reconciliations (
    id bigint NOT NULL,
    doc_number character varying(30) NOT NULL,
    customer_id bigint NOT NULL,
    period character varying(7) NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    total_amount numeric(20,4) DEFAULT 0 NOT NULL,
    confirmed_amount numeric(20,4) DEFAULT 0 NOT NULL,
    difference numeric(20,4) DEFAULT 0 NOT NULL,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: reconciliations_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.reconciliations_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: reconciliations_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.reconciliations_id_seq OWNED BY public.reconciliations.id;


--
-- Name: rmas; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.rmas (
    id bigint NOT NULL,
    doc_number character varying(30) NOT NULL,
    customer_id bigint NOT NULL,
    sales_order_id bigint,
    shipping_request_id bigint,
    product_id bigint NOT NULL,
    linked_inspection_result_id bigint,
    defect_description text DEFAULT ''::text NOT NULL,
    severity smallint DEFAULT 1 NOT NULL,
    root_cause text,
    corrective_action text,
    status smallint DEFAULT 1 NOT NULL,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: rmas_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.rmas_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: rmas_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.rmas_id_seq OWNED BY public.rmas.id;


--
-- Name: role_permissions; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.role_permissions (
    role_id bigint NOT NULL,
    resource_code character varying(50) NOT NULL,
    action character varying(20) NOT NULL
);


--
-- Name: roles; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.roles (
    role_id bigint NOT NULL,
    role_name character varying(100) NOT NULL,
    role_code character varying(50) NOT NULL,
    is_system_role boolean DEFAULT false NOT NULL,
    parent_role_id bigint,
    description character varying(255),
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone
);


--
-- Name: roles_role_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.roles_role_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: roles_role_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.roles_role_id_seq OWNED BY public.roles.role_id;


--
-- Name: routing_steps; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.routing_steps (
    id bigint NOT NULL,
    routing_id bigint NOT NULL,
    process_code character varying(100) NOT NULL,
    step_order integer NOT NULL,
    is_required boolean DEFAULT true NOT NULL,
    remark text,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: routing_steps_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.routing_steps_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: routing_steps_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.routing_steps_id_seq OWNED BY public.routing_steps.id;


--
-- Name: routings; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.routings (
    id bigint NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    operator_id bigint,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone,
    deleted_at timestamp with time zone
);


--
-- Name: routings_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.routings_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: routings_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.routings_id_seq OWNED BY public.routings.id;


--
-- Name: sales_order_items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.sales_order_items (
    id bigint NOT NULL,
    order_id bigint NOT NULL,
    line_no integer NOT NULL,
    product_id bigint NOT NULL,
    description text DEFAULT ''::text NOT NULL,
    quantity numeric(18,6) NOT NULL,
    unit character varying(20) DEFAULT ''::character varying NOT NULL,
    unit_price numeric(18,6) NOT NULL,
    unit_cost numeric(18,6) DEFAULT 0 NOT NULL,
    discount_rate numeric(5,2) DEFAULT 0 NOT NULL,
    amount numeric(20,4) NOT NULL,
    shipped_qty numeric(18,6) DEFAULT 0 NOT NULL,
    returned_qty numeric(18,6) DEFAULT 0 NOT NULL,
    delivery_date date
);


--
-- Name: sales_order_items_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.sales_order_items_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: sales_order_items_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.sales_order_items_id_seq OWNED BY public.sales_order_items.id;


--
-- Name: sales_orders; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.sales_orders (
    id bigint NOT NULL,
    doc_number character varying(30) NOT NULL,
    customer_id bigint NOT NULL,
    contact_id bigint NOT NULL,
    sales_rep_id bigint NOT NULL,
    order_date date DEFAULT CURRENT_DATE NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    total_amount numeric(20,4) DEFAULT 0 NOT NULL,
    total_cost numeric(20,4) DEFAULT 0 NOT NULL,
    payment_terms character varying(100) DEFAULT ''::character varying NOT NULL,
    delivery_terms character varying(100) DEFAULT ''::character varying NOT NULL,
    delivery_address text DEFAULT ''::text NOT NULL,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: sales_orders_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.sales_orders_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: sales_orders_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.sales_orders_id_seq OWNED BY public.sales_orders.id;


--
-- Name: sales_return_items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.sales_return_items (
    id bigint NOT NULL,
    return_id bigint NOT NULL,
    order_item_id bigint NOT NULL,
    product_id bigint NOT NULL,
    returned_qty numeric(18,6) NOT NULL,
    unit_price numeric(18,6) NOT NULL,
    amount numeric(20,4) NOT NULL,
    disposition smallint DEFAULT 1 NOT NULL
);


--
-- Name: sales_return_items_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.sales_return_items_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: sales_return_items_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.sales_return_items_id_seq OWNED BY public.sales_return_items.id;


--
-- Name: sales_returns; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.sales_returns (
    id bigint NOT NULL,
    doc_number character varying(30) NOT NULL,
    order_id bigint NOT NULL,
    shipping_request_id bigint NOT NULL,
    customer_id bigint NOT NULL,
    return_date date DEFAULT CURRENT_DATE NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    return_reason text DEFAULT ''::text NOT NULL,
    total_amount numeric(20,4) DEFAULT 0 NOT NULL,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: sales_returns_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.sales_returns_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: sales_returns_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.sales_returns_id_seq OWNED BY public.sales_returns.id;


--
-- Name: scheduled_task_defs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.scheduled_task_defs (
    task_id bigint NOT NULL,
    name character varying(255) NOT NULL,
    interval_secs bigint NOT NULL,
    timeout_secs bigint NOT NULL,
    is_enabled boolean DEFAULT true NOT NULL,
    last_run_at timestamp with time zone,
    last_elapsed_ms bigint,
    last_result text,
    last_error text,
    total_runs bigint DEFAULT 0 NOT NULL
);


--
-- Name: scheduled_task_defs_task_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.scheduled_task_defs_task_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: scheduled_task_defs_task_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.scheduled_task_defs_task_id_seq OWNED BY public.scheduled_task_defs.task_id;


--
-- Name: shipping_request_items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.shipping_request_items (
    id bigint NOT NULL,
    shipping_request_id bigint NOT NULL,
    line_no integer NOT NULL,
    order_item_id bigint NOT NULL,
    product_id bigint NOT NULL,
    warehouse_id bigint NOT NULL,
    requested_qty numeric(18,6) NOT NULL,
    shipped_qty numeric(18,6) DEFAULT 0 NOT NULL,
    description text DEFAULT ''::text NOT NULL
);


--
-- Name: shipping_request_items_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.shipping_request_items_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: shipping_request_items_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.shipping_request_items_id_seq OWNED BY public.shipping_request_items.id;


--
-- Name: shipping_requests; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.shipping_requests (
    id bigint NOT NULL,
    doc_number character varying(30) NOT NULL,
    order_id bigint NOT NULL,
    customer_id bigint NOT NULL,
    request_date date DEFAULT CURRENT_DATE NOT NULL,
    expected_ship_date date,
    status smallint DEFAULT 1 NOT NULL,
    shipping_address text DEFAULT ''::text NOT NULL,
    carrier character varying(100) DEFAULT ''::character varying NOT NULL,
    tracking_number character varying(100) DEFAULT ''::character varying NOT NULL,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: shipping_requests_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.shipping_requests_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: shipping_requests_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.shipping_requests_id_seq OWNED BY public.shipping_requests.id;


--
-- Name: state_definitions; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.state_definitions (
    id bigint NOT NULL,
    entity_type character varying(50) NOT NULL,
    state_name character varying(50) NOT NULL,
    label character varying(100) NOT NULL,
    is_initial boolean DEFAULT false NOT NULL,
    is_final boolean DEFAULT false NOT NULL
);


--
-- Name: state_definitions_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.state_definitions_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: state_definitions_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.state_definitions_id_seq OWNED BY public.state_definitions.id;


--
-- Name: state_transition_defs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.state_transition_defs (
    id bigint NOT NULL,
    entity_type character varying(50) NOT NULL,
    from_state character varying(50) NOT NULL,
    to_state character varying(50) NOT NULL,
    trigger_event smallint,
    guard_condition jsonb,
    side_effects jsonb DEFAULT '[]'::jsonb NOT NULL,
    sort_order integer DEFAULT 0 NOT NULL
);


--
-- Name: state_transition_defs_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.state_transition_defs_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: state_transition_defs_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.state_transition_defs_id_seq OWNED BY public.state_transition_defs.id;


--
-- Name: stock_ledger; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.stock_ledger (
    id bigint NOT NULL,
    product_id bigint NOT NULL,
    warehouse_id bigint NOT NULL,
    zone_id bigint NOT NULL,
    bin_id bigint NOT NULL,
    batch_no character varying(50),
    quantity numeric(18,6) DEFAULT 0 NOT NULL,
    reserved_qty numeric(18,6) DEFAULT 0 NOT NULL,
    available_qty numeric(18,6) DEFAULT 0 NOT NULL,
    unit_cost numeric(10,6),
    received_date date,
    expiry_date date,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    safety_stock numeric(18,6) DEFAULT 0 NOT NULL
);


--
-- Name: stock_ledger_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.stock_ledger_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: stock_ledger_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.stock_ledger_id_seq OWNED BY public.stock_ledger.id;


--
-- Name: supplier_bank_accounts; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.supplier_bank_accounts (
    account_id bigint NOT NULL,
    supplier_id bigint NOT NULL,
    bank_name character varying(100) NOT NULL,
    account_name character varying(100) NOT NULL,
    account_number character varying(50) NOT NULL,
    is_default boolean DEFAULT false NOT NULL
);


--
-- Name: supplier_bank_accounts_account_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.supplier_bank_accounts_account_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: supplier_bank_accounts_account_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.supplier_bank_accounts_account_id_seq OWNED BY public.supplier_bank_accounts.account_id;


--
-- Name: supplier_contacts; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.supplier_contacts (
    contact_id bigint NOT NULL,
    supplier_id bigint NOT NULL,
    contact_name character varying(100) NOT NULL,
    "position" character varying(100),
    phone character varying(50),
    email character varying(100),
    is_primary boolean DEFAULT false NOT NULL
);


--
-- Name: supplier_contacts_contact_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.supplier_contacts_contact_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: supplier_contacts_contact_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.supplier_contacts_contact_id_seq OWNED BY public.supplier_contacts.contact_id;


--
-- Name: suppliers; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.suppliers (
    supplier_id bigint NOT NULL,
    supplier_code character varying(100) NOT NULL,
    supplier_name character varying(255) NOT NULL,
    short_name character varying(100),
    category smallint NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    tax_number character varying(50),
    lead_time_days integer DEFAULT 0 NOT NULL,
    payment_terms text,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: suppliers_supplier_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.suppliers_supplier_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: suppliers_supplier_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.suppliers_supplier_id_seq OWNED BY public.suppliers.supplier_id;


--
-- Name: task_run_logs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.task_run_logs (
    run_id bigint NOT NULL,
    task_id bigint NOT NULL,
    status smallint NOT NULL,
    started_at timestamp with time zone NOT NULL,
    finished_at timestamp with time zone,
    elapsed_ms bigint,
    result text,
    error text
);


--
-- Name: task_run_logs_run_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.task_run_logs_run_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: task_run_logs_run_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.task_run_logs_run_id_seq OWNED BY public.task_run_logs.run_id;


--
-- Name: transfer_items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.transfer_items (
    id bigint NOT NULL,
    transfer_id bigint NOT NULL,
    product_id bigint NOT NULL,
    quantity numeric(10,6) NOT NULL,
    batch_no character varying(50)
);


--
-- Name: transfer_items_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.transfer_items_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: transfer_items_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.transfer_items_id_seq OWNED BY public.transfer_items.id;


--
-- Name: user_departments; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.user_departments (
    user_id bigint NOT NULL,
    department_id bigint NOT NULL
);


--
-- Name: user_roles; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.user_roles (
    user_id bigint NOT NULL,
    role_id bigint NOT NULL
);


--
-- Name: users; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.users (
    user_id bigint NOT NULL,
    username character varying(50) NOT NULL,
    password_hash character varying(255) NOT NULL,
    display_name character varying(100),
    is_active boolean DEFAULT true NOT NULL,
    is_super_admin boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone
);


--
-- Name: users_user_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.users_user_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: users_user_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.users_user_id_seq OWNED BY public.users.user_id;


--
-- Name: warehouses; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.warehouses (
    id bigint NOT NULL,
    code character varying(50) NOT NULL,
    name character varying(200) NOT NULL,
    warehouse_type smallint NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    address text,
    manager_id bigint,
    is_virtual boolean DEFAULT false NOT NULL,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: warehouses_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.warehouses_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: warehouses_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.warehouses_id_seq OWNED BY public.warehouses.id;


--
-- Name: work_order_routings; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.work_order_routings (
    id bigint NOT NULL,
    work_order_id bigint NOT NULL,
    step_no integer NOT NULL,
    process_name character varying(200) NOT NULL,
    work_center_id bigint,
    standard_time numeric(18,6),
    standard_cost numeric(18,6),
    unit_price numeric(18,6),
    allowed_loss_rate numeric(18,6),
    planned_qty numeric(18,6) DEFAULT 0 NOT NULL,
    completed_qty numeric(18,6) DEFAULT 0 NOT NULL,
    defect_qty numeric(18,6) DEFAULT 0 NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    is_outsourced boolean DEFAULT false NOT NULL,
    is_inspection_point boolean DEFAULT false NOT NULL
);


--
-- Name: work_order_routings_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.work_order_routings_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: work_order_routings_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.work_order_routings_id_seq OWNED BY public.work_order_routings.id;


--
-- Name: work_orders; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.work_orders (
    id bigint NOT NULL,
    doc_number character varying(50) NOT NULL,
    plan_item_id bigint,
    product_id bigint NOT NULL,
    bom_snapshot_id bigint,
    routing_id bigint,
    planned_qty numeric(18,6) NOT NULL,
    scheduled_start date NOT NULL,
    scheduled_end date NOT NULL,
    status smallint DEFAULT 1 NOT NULL,
    work_center_id bigint,
    sales_order_id bigint,
    version integer DEFAULT 1 NOT NULL,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: work_orders_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.work_orders_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: work_orders_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.work_orders_id_seq OWNED BY public.work_orders.id;


--
-- Name: work_reports; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.work_reports (
    id bigint NOT NULL,
    doc_number character varying(50) NOT NULL,
    work_order_id bigint NOT NULL,
    batch_id bigint NOT NULL,
    routing_id bigint NOT NULL,
    report_date date NOT NULL,
    shift smallint NOT NULL,
    worker_id bigint NOT NULL,
    completed_qty numeric(18,6) NOT NULL,
    defect_qty numeric(18,6) DEFAULT 0 NOT NULL,
    defect_reason smallint,
    work_hours numeric(18,6) DEFAULT 0 NOT NULL,
    remark text DEFAULT ''::text NOT NULL,
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: work_reports_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.work_reports_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: work_reports_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.work_reports_id_seq OWNED BY public.work_reports.id;


--
-- Name: workflow_history; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.workflow_history (
    id bigint NOT NULL,
    instance_id bigint NOT NULL,
    task_id bigint,
    node_id character varying(100),
    event_type character varying(50) NOT NULL,
    actor_id bigint,
    payload jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: TABLE workflow_history; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.workflow_history IS '工作流审计历史';


--
-- Name: workflow_history_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.workflow_history_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: workflow_history_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.workflow_history_id_seq OWNED BY public.workflow_history.id;


--
-- Name: workflow_instances; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.workflow_instances (
    id bigint NOT NULL,
    template_id bigint NOT NULL,
    template_version integer,
    entity_type character varying(100) NOT NULL,
    entity_id bigint NOT NULL,
    status character varying(20) DEFAULT 'running'::character varying NOT NULL,
    frozen_graph jsonb,
    context jsonb,
    suspended_reason jsonb,
    initiator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone,
    last_advanced_at timestamp with time zone,
    completed_at timestamp with time zone,
    CONSTRAINT wf_instance_status_check CHECK (((status)::text = ANY ((ARRAY['running'::character varying, 'completed'::character varying, 'rejected'::character varying, 'suspended'::character varying, 'cancelled'::character varying, 'terminated'::character varying])::text[])))
);


--
-- Name: TABLE workflow_instances; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.workflow_instances IS '工作流实例';


--
-- Name: workflow_instances_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.workflow_instances_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: workflow_instances_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.workflow_instances_id_seq OWNED BY public.workflow_instances.id;


--
-- Name: workflow_tasks; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.workflow_tasks (
    id bigint NOT NULL,
    instance_id bigint NOT NULL,
    node_id character varying(100) NOT NULL,
    prev_task_id bigint,
    assignee_id bigint,
    status character varying(20) DEFAULT 'pending'::character varying NOT NULL,
    action character varying(20),
    timeout_action character varying(20),
    due_at timestamp with time zone,
    remind_at timestamp with time zone,
    result jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    completed_at timestamp with time zone,
    CONSTRAINT wf_task_status_check CHECK (((status)::text = ANY ((ARRAY['pending'::character varying, 'completed'::character varying, 'rejected'::character varying, 'delegated'::character varying, 'timed_out'::character varying, 'cancelled'::character varying])::text[])))
);


--
-- Name: TABLE workflow_tasks; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.workflow_tasks IS '工作流任务';


--
-- Name: workflow_tasks_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.workflow_tasks_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: workflow_tasks_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.workflow_tasks_id_seq OWNED BY public.workflow_tasks.id;


--
-- Name: workflow_templates; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.workflow_templates (
    id bigint NOT NULL,
    entity_type character varying(100) NOT NULL,
    name character varying(255) NOT NULL,
    version integer DEFAULT 1 NOT NULL,
    status character varying(20) DEFAULT 'draft'::character varying NOT NULL,
    graph jsonb,
    graph_checksum character varying(64),
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone,
    deleted_at timestamp with time zone,
    trigger_event character varying(100),
    CONSTRAINT wf_template_status_check CHECK (((status)::text = ANY ((ARRAY['draft'::character varying, 'active'::character varying, 'archived'::character varying])::text[])))
);


--
-- Name: TABLE workflow_templates; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.workflow_templates IS '工作流模板';


--
-- Name: workflow_templates_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.workflow_templates_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: workflow_templates_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.workflow_templates_id_seq OWNED BY public.workflow_templates.id;


--
-- Name: write_offs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.write_offs (
    id bigint NOT NULL,
    write_off_type smallint NOT NULL,
    cash_journal_id bigint NOT NULL,
    source_type smallint NOT NULL,
    source_id bigint NOT NULL,
    amount numeric(20,4) NOT NULL,
    write_off_date date NOT NULL,
    idempotency_key character varying(128),
    operator_id bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT write_offs_amount_check CHECK ((amount > (0)::numeric))
);


--
-- Name: write_offs_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.write_offs_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: write_offs_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.write_offs_id_seq OWNED BY public.write_offs.id;


--
-- Name: zones; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.zones (
    id bigint NOT NULL,
    warehouse_id bigint NOT NULL,
    code character varying(50) NOT NULL,
    name character varying(200) NOT NULL,
    zone_type smallint NOT NULL,
    sort_order integer DEFAULT 0 NOT NULL,
    remark text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: zones_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.zones_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: zones_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.zones_id_seq OWNED BY public.zones.id;


--
-- Name: audit_logs_2026_01; Type: TABLE ATTACH; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs ATTACH PARTITION public.audit_logs_2026_01 FOR VALUES FROM ('2026-01-01 00:00:00+00') TO ('2026-02-01 00:00:00+00');


--
-- Name: audit_logs_2026_02; Type: TABLE ATTACH; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs ATTACH PARTITION public.audit_logs_2026_02 FOR VALUES FROM ('2026-02-01 00:00:00+00') TO ('2026-03-01 00:00:00+00');


--
-- Name: audit_logs_2026_03; Type: TABLE ATTACH; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs ATTACH PARTITION public.audit_logs_2026_03 FOR VALUES FROM ('2026-03-01 00:00:00+00') TO ('2026-04-01 00:00:00+00');


--
-- Name: audit_logs_2026_04; Type: TABLE ATTACH; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs ATTACH PARTITION public.audit_logs_2026_04 FOR VALUES FROM ('2026-04-01 00:00:00+00') TO ('2026-05-01 00:00:00+00');


--
-- Name: audit_logs_2026_05; Type: TABLE ATTACH; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs ATTACH PARTITION public.audit_logs_2026_05 FOR VALUES FROM ('2026-05-01 00:00:00+00') TO ('2026-06-01 00:00:00+00');


--
-- Name: audit_logs_2026_06; Type: TABLE ATTACH; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs ATTACH PARTITION public.audit_logs_2026_06 FOR VALUES FROM ('2026-06-01 00:00:00+00') TO ('2026-07-01 00:00:00+00');


--
-- Name: audit_logs_2026_07; Type: TABLE ATTACH; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs ATTACH PARTITION public.audit_logs_2026_07 FOR VALUES FROM ('2026-07-01 00:00:00+00') TO ('2026-08-01 00:00:00+00');


--
-- Name: audit_logs_2026_08; Type: TABLE ATTACH; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs ATTACH PARTITION public.audit_logs_2026_08 FOR VALUES FROM ('2026-08-01 00:00:00+00') TO ('2026-09-01 00:00:00+00');


--
-- Name: audit_logs_2026_09; Type: TABLE ATTACH; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs ATTACH PARTITION public.audit_logs_2026_09 FOR VALUES FROM ('2026-09-01 00:00:00+00') TO ('2026-10-01 00:00:00+00');


--
-- Name: audit_logs_2026_10; Type: TABLE ATTACH; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs ATTACH PARTITION public.audit_logs_2026_10 FOR VALUES FROM ('2026-10-01 00:00:00+00') TO ('2026-11-01 00:00:00+00');


--
-- Name: audit_logs_2026_11; Type: TABLE ATTACH; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs ATTACH PARTITION public.audit_logs_2026_11 FOR VALUES FROM ('2026-11-01 00:00:00+00') TO ('2026-12-01 00:00:00+00');


--
-- Name: audit_logs_2026_12; Type: TABLE ATTACH; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs ATTACH PARTITION public.audit_logs_2026_12 FOR VALUES FROM ('2026-12-01 00:00:00+00') TO ('2027-01-01 00:00:00+00');


--
-- Name: arrival_notice_items id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.arrival_notice_items ALTER COLUMN id SET DEFAULT nextval('public.arrival_notice_items_id_seq'::regclass);


--
-- Name: arrival_notices id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.arrival_notices ALTER COLUMN id SET DEFAULT nextval('public.arrival_notices_id_seq'::regclass);


--
-- Name: audit_logs id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs ALTER COLUMN id SET DEFAULT nextval('public.audit_logs_id_seq'::regclass);


--
-- Name: backflush_items id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.backflush_items ALTER COLUMN id SET DEFAULT nextval('public.backflush_items_id_seq'::regclass);


--
-- Name: backflush_records id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.backflush_records ALTER COLUMN id SET DEFAULT nextval('public.backflush_records_id_seq'::regclass);


--
-- Name: bins id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.bins ALTER COLUMN id SET DEFAULT nextval('public.bins_id_seq'::regclass);


--
-- Name: bom_categories bom_category_id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.bom_categories ALTER COLUMN bom_category_id SET DEFAULT nextval('public.bom_categories_bom_category_id_seq'::regclass);


--
-- Name: bom_labor_processes id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.bom_labor_processes ALTER COLUMN id SET DEFAULT nextval('public.bom_labor_processes_id_seq'::regclass);


--
-- Name: bom_nodes node_id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.bom_nodes ALTER COLUMN node_id SET DEFAULT nextval('public.bom_nodes_node_id_seq'::regclass);


--
-- Name: bom_routings id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.bom_routings ALTER COLUMN id SET DEFAULT nextval('public.bom_routings_id_seq'::regclass);


--
-- Name: bom_snapshots snapshot_id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.bom_snapshots ALTER COLUMN snapshot_id SET DEFAULT nextval('public.bom_snapshots_snapshot_id_seq'::regclass);


--
-- Name: boms bom_id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.boms ALTER COLUMN bom_id SET DEFAULT nextval('public.boms_bom_id_seq'::regclass);


--
-- Name: cash_journal_lines id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cash_journal_lines ALTER COLUMN id SET DEFAULT nextval('public.cash_journal_lines_id_seq'::regclass);


--
-- Name: cash_journals id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cash_journals ALTER COLUMN id SET DEFAULT nextval('public.cash_journals_id_seq'::regclass);


--
-- Name: categories category_id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.categories ALTER COLUMN category_id SET DEFAULT nextval('public.categories_category_id_seq'::regclass);


--
-- Name: conversion_items id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.conversion_items ALTER COLUMN id SET DEFAULT nextval('public.conversion_items_id_seq'::regclass);


--
-- Name: cost_entries id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cost_entries ALTER COLUMN id SET DEFAULT nextval('public.cost_entries_id_seq'::regclass);


--
-- Name: customer_addresses address_id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.customer_addresses ALTER COLUMN address_id SET DEFAULT nextval('public.customer_addresses_address_id_seq'::regclass);


--
-- Name: customer_contacts contact_id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.customer_contacts ALTER COLUMN contact_id SET DEFAULT nextval('public.customer_contacts_contact_id_seq'::regclass);


--
-- Name: customers customer_id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.customers ALTER COLUMN customer_id SET DEFAULT nextval('public.customers_customer_id_seq'::regclass);


--
-- Name: cycle_count_items id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cycle_count_items ALTER COLUMN id SET DEFAULT nextval('public.cycle_count_items_id_seq'::regclass);


--
-- Name: cycle_counts id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cycle_counts ALTER COLUMN id SET DEFAULT nextval('public.cycle_counts_id_seq'::regclass);


--
-- Name: departments department_id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.departments ALTER COLUMN department_id SET DEFAULT nextval('public.departments_department_id_seq'::regclass);


--
-- Name: document_links id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.document_links ALTER COLUMN id SET DEFAULT nextval('public.document_links_id_seq'::regclass);


--
-- Name: document_sequences id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.document_sequences ALTER COLUMN id SET DEFAULT nextval('public.document_sequences_id_seq'::regclass);


--
-- Name: domain_events id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.domain_events ALTER COLUMN id SET DEFAULT nextval('public.domain_events_id_seq'::regclass);


--
-- Name: entity_state_logs id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.entity_state_logs ALTER COLUMN id SET DEFAULT nextval('public.entity_state_logs_id_seq'::regclass);


--
-- Name: expense_reimbursement_items id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.expense_reimbursement_items ALTER COLUMN id SET DEFAULT nextval('public.expense_reimbursement_items_id_seq'::regclass);


--
-- Name: expense_reimbursements id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.expense_reimbursements ALTER COLUMN id SET DEFAULT nextval('public.expense_reimbursements_id_seq'::regclass);


--
-- Name: form_conversions id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.form_conversions ALTER COLUMN id SET DEFAULT nextval('public.form_conversions_id_seq'::regclass);


--
-- Name: idempotency_records id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.idempotency_records ALTER COLUMN id SET DEFAULT nextval('public.idempotency_records_id_seq'::regclass);


--
-- Name: inspection_results id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.inspection_results ALTER COLUMN id SET DEFAULT nextval('public.inspection_results_id_seq'::regclass);


--
-- Name: inspection_specifications id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.inspection_specifications ALTER COLUMN id SET DEFAULT nextval('public.inspection_specifications_id_seq'::regclass);


--
-- Name: inventory_locks id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.inventory_locks ALTER COLUMN id SET DEFAULT nextval('public.inventory_locks_id_seq'::regclass);


--
-- Name: inventory_reservations id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.inventory_reservations ALTER COLUMN id SET DEFAULT nextval('public.inventory_reservations_id_seq'::regclass);


--
-- Name: inventory_transactions id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.inventory_transactions ALTER COLUMN id SET DEFAULT nextval('public.inventory_transactions_id_seq'::regclass);


--
-- Name: inventory_transfers id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.inventory_transfers ALTER COLUMN id SET DEFAULT nextval('public.inventory_transfers_id_seq'::regclass);


--
-- Name: labor_process_dicts id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.labor_process_dicts ALTER COLUMN id SET DEFAULT nextval('public.labor_process_dicts_id_seq'::regclass);


--
-- Name: material_requisition_items id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.material_requisition_items ALTER COLUMN id SET DEFAULT nextval('public.material_requisition_items_id_seq'::regclass);


--
-- Name: material_requisitions id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.material_requisitions ALTER COLUMN id SET DEFAULT nextval('public.material_requisitions_id_seq'::regclass);


--
-- Name: misc_request_items id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.misc_request_items ALTER COLUMN id SET DEFAULT nextval('public.misc_request_items_id_seq'::regclass);


--
-- Name: miscellaneous_requests id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.miscellaneous_requests ALTER COLUMN id SET DEFAULT nextval('public.miscellaneous_requests_id_seq'::regclass);


--
-- Name: mrbs id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.mrbs ALTER COLUMN id SET DEFAULT nextval('public.mrbs_id_seq'::regclass);


--
-- Name: notifications notification_id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.notifications ALTER COLUMN notification_id SET DEFAULT nextval('public.notifications_notification_id_seq'::regclass);


--
-- Name: outsourcing_materials id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.outsourcing_materials ALTER COLUMN id SET DEFAULT nextval('public.outsourcing_materials_id_seq'::regclass);


--
-- Name: outsourcing_orders id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.outsourcing_orders ALTER COLUMN id SET DEFAULT nextval('public.outsourcing_orders_id_seq'::regclass);


--
-- Name: outsourcing_trackings id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.outsourcing_trackings ALTER COLUMN id SET DEFAULT nextval('public.outsourcing_trackings_id_seq'::regclass);


--
-- Name: payment_requests id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.payment_requests ALTER COLUMN id SET DEFAULT nextval('public.payment_requests_id_seq'::regclass);


--
-- Name: pick_strategies id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.pick_strategies ALTER COLUMN id SET DEFAULT nextval('public.pick_strategies_id_seq'::regclass);


--
-- Name: price_log log_id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.price_log ALTER COLUMN log_id SET DEFAULT nextval('public.price_log_log_id_seq'::regclass);


--
-- Name: production_batches id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.production_batches ALTER COLUMN id SET DEFAULT nextval('public.production_batches_id_seq'::regclass);


--
-- Name: production_inspections id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.production_inspections ALTER COLUMN id SET DEFAULT nextval('public.production_inspections_id_seq'::regclass);


--
-- Name: production_plan_items id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.production_plan_items ALTER COLUMN id SET DEFAULT nextval('public.production_plan_items_id_seq'::regclass);


--
-- Name: production_plans id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.production_plans ALTER COLUMN id SET DEFAULT nextval('public.production_plans_id_seq'::regclass);


--
-- Name: production_receipts id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.production_receipts ALTER COLUMN id SET DEFAULT nextval('public.production_receipts_id_seq'::regclass);


--
-- Name: products product_id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.products ALTER COLUMN product_id SET DEFAULT nextval('public.products_product_id_seq'::regclass);


--
-- Name: purchase_order_items id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.purchase_order_items ALTER COLUMN id SET DEFAULT nextval('public.purchase_order_items_id_seq'::regclass);


--
-- Name: purchase_orders id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.purchase_orders ALTER COLUMN id SET DEFAULT nextval('public.purchase_orders_id_seq'::regclass);


--
-- Name: purchase_quotation_items id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.purchase_quotation_items ALTER COLUMN id SET DEFAULT nextval('public.purchase_quotation_items_id_seq'::regclass);


--
-- Name: purchase_quotations id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.purchase_quotations ALTER COLUMN id SET DEFAULT nextval('public.purchase_quotations_id_seq'::regclass);


--
-- Name: purchase_recon_items id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.purchase_recon_items ALTER COLUMN id SET DEFAULT nextval('public.purchase_recon_items_id_seq'::regclass);


--
-- Name: purchase_reconciliations id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.purchase_reconciliations ALTER COLUMN id SET DEFAULT nextval('public.purchase_reconciliations_id_seq'::regclass);


--
-- Name: purchase_return_items id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.purchase_return_items ALTER COLUMN id SET DEFAULT nextval('public.purchase_return_items_id_seq'::regclass);


--
-- Name: purchase_returns id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.purchase_returns ALTER COLUMN id SET DEFAULT nextval('public.purchase_returns_id_seq'::regclass);


--
-- Name: putaway_strategies id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.putaway_strategies ALTER COLUMN id SET DEFAULT nextval('public.putaway_strategies_id_seq'::regclass);


--
-- Name: quotation_items id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.quotation_items ALTER COLUMN id SET DEFAULT nextval('public.quotation_items_id_seq'::regclass);


--
-- Name: quotations id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.quotations ALTER COLUMN id SET DEFAULT nextval('public.quotations_id_seq'::regclass);


--
-- Name: reconciliation_items id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.reconciliation_items ALTER COLUMN id SET DEFAULT nextval('public.reconciliation_items_id_seq'::regclass);


--
-- Name: reconciliations id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.reconciliations ALTER COLUMN id SET DEFAULT nextval('public.reconciliations_id_seq'::regclass);


--
-- Name: rmas id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.rmas ALTER COLUMN id SET DEFAULT nextval('public.rmas_id_seq'::regclass);


--
-- Name: roles role_id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.roles ALTER COLUMN role_id SET DEFAULT nextval('public.roles_role_id_seq'::regclass);


--
-- Name: routing_steps id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.routing_steps ALTER COLUMN id SET DEFAULT nextval('public.routing_steps_id_seq'::regclass);


--
-- Name: routings id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.routings ALTER COLUMN id SET DEFAULT nextval('public.routings_id_seq'::regclass);


--
-- Name: sales_order_items id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.sales_order_items ALTER COLUMN id SET DEFAULT nextval('public.sales_order_items_id_seq'::regclass);


--
-- Name: sales_orders id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.sales_orders ALTER COLUMN id SET DEFAULT nextval('public.sales_orders_id_seq'::regclass);


--
-- Name: sales_return_items id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.sales_return_items ALTER COLUMN id SET DEFAULT nextval('public.sales_return_items_id_seq'::regclass);


--
-- Name: sales_returns id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.sales_returns ALTER COLUMN id SET DEFAULT nextval('public.sales_returns_id_seq'::regclass);


--
-- Name: scheduled_task_defs task_id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.scheduled_task_defs ALTER COLUMN task_id SET DEFAULT nextval('public.scheduled_task_defs_task_id_seq'::regclass);


--
-- Name: shipping_request_items id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.shipping_request_items ALTER COLUMN id SET DEFAULT nextval('public.shipping_request_items_id_seq'::regclass);


--
-- Name: shipping_requests id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.shipping_requests ALTER COLUMN id SET DEFAULT nextval('public.shipping_requests_id_seq'::regclass);


--
-- Name: state_definitions id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.state_definitions ALTER COLUMN id SET DEFAULT nextval('public.state_definitions_id_seq'::regclass);


--
-- Name: state_transition_defs id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.state_transition_defs ALTER COLUMN id SET DEFAULT nextval('public.state_transition_defs_id_seq'::regclass);


--
-- Name: stock_ledger id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.stock_ledger ALTER COLUMN id SET DEFAULT nextval('public.stock_ledger_id_seq'::regclass);


--
-- Name: supplier_bank_accounts account_id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.supplier_bank_accounts ALTER COLUMN account_id SET DEFAULT nextval('public.supplier_bank_accounts_account_id_seq'::regclass);


--
-- Name: supplier_contacts contact_id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.supplier_contacts ALTER COLUMN contact_id SET DEFAULT nextval('public.supplier_contacts_contact_id_seq'::regclass);


--
-- Name: suppliers supplier_id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.suppliers ALTER COLUMN supplier_id SET DEFAULT nextval('public.suppliers_supplier_id_seq'::regclass);


--
-- Name: task_run_logs run_id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.task_run_logs ALTER COLUMN run_id SET DEFAULT nextval('public.task_run_logs_run_id_seq'::regclass);


--
-- Name: transfer_items id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transfer_items ALTER COLUMN id SET DEFAULT nextval('public.transfer_items_id_seq'::regclass);


--
-- Name: users user_id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users ALTER COLUMN user_id SET DEFAULT nextval('public.users_user_id_seq'::regclass);


--
-- Name: warehouses id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.warehouses ALTER COLUMN id SET DEFAULT nextval('public.warehouses_id_seq'::regclass);


--
-- Name: work_order_routings id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.work_order_routings ALTER COLUMN id SET DEFAULT nextval('public.work_order_routings_id_seq'::regclass);


--
-- Name: work_orders id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.work_orders ALTER COLUMN id SET DEFAULT nextval('public.work_orders_id_seq'::regclass);


--
-- Name: work_reports id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.work_reports ALTER COLUMN id SET DEFAULT nextval('public.work_reports_id_seq'::regclass);


--
-- Name: workflow_history id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.workflow_history ALTER COLUMN id SET DEFAULT nextval('public.workflow_history_id_seq'::regclass);


--
-- Name: workflow_instances id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.workflow_instances ALTER COLUMN id SET DEFAULT nextval('public.workflow_instances_id_seq'::regclass);


--
-- Name: workflow_tasks id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.workflow_tasks ALTER COLUMN id SET DEFAULT nextval('public.workflow_tasks_id_seq'::regclass);


--
-- Name: workflow_templates id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.workflow_templates ALTER COLUMN id SET DEFAULT nextval('public.workflow_templates_id_seq'::regclass);


--
-- Name: write_offs id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.write_offs ALTER COLUMN id SET DEFAULT nextval('public.write_offs_id_seq'::regclass);


--
-- Name: zones id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.zones ALTER COLUMN id SET DEFAULT nextval('public.zones_id_seq'::regclass);


--
-- Name: arrival_notice_items arrival_notice_items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.arrival_notice_items
    ADD CONSTRAINT arrival_notice_items_pkey PRIMARY KEY (id);


--
-- Name: arrival_notices arrival_notices_doc_number_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.arrival_notices
    ADD CONSTRAINT arrival_notices_doc_number_key UNIQUE (doc_number);


--
-- Name: arrival_notices arrival_notices_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.arrival_notices
    ADD CONSTRAINT arrival_notices_pkey PRIMARY KEY (id);


--
-- Name: audit_logs audit_logs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs
    ADD CONSTRAINT audit_logs_pkey PRIMARY KEY (id, created_at);


--
-- Name: audit_logs_2026_01 audit_logs_2026_01_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs_2026_01
    ADD CONSTRAINT audit_logs_2026_01_pkey PRIMARY KEY (id, created_at);


--
-- Name: audit_logs_2026_02 audit_logs_2026_02_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs_2026_02
    ADD CONSTRAINT audit_logs_2026_02_pkey PRIMARY KEY (id, created_at);


--
-- Name: audit_logs_2026_03 audit_logs_2026_03_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs_2026_03
    ADD CONSTRAINT audit_logs_2026_03_pkey PRIMARY KEY (id, created_at);


--
-- Name: audit_logs_2026_04 audit_logs_2026_04_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs_2026_04
    ADD CONSTRAINT audit_logs_2026_04_pkey PRIMARY KEY (id, created_at);


--
-- Name: audit_logs_2026_05 audit_logs_2026_05_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs_2026_05
    ADD CONSTRAINT audit_logs_2026_05_pkey PRIMARY KEY (id, created_at);


--
-- Name: audit_logs_2026_06 audit_logs_2026_06_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs_2026_06
    ADD CONSTRAINT audit_logs_2026_06_pkey PRIMARY KEY (id, created_at);


--
-- Name: audit_logs_2026_07 audit_logs_2026_07_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs_2026_07
    ADD CONSTRAINT audit_logs_2026_07_pkey PRIMARY KEY (id, created_at);


--
-- Name: audit_logs_2026_08 audit_logs_2026_08_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs_2026_08
    ADD CONSTRAINT audit_logs_2026_08_pkey PRIMARY KEY (id, created_at);


--
-- Name: audit_logs_2026_09 audit_logs_2026_09_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs_2026_09
    ADD CONSTRAINT audit_logs_2026_09_pkey PRIMARY KEY (id, created_at);


--
-- Name: audit_logs_2026_10 audit_logs_2026_10_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs_2026_10
    ADD CONSTRAINT audit_logs_2026_10_pkey PRIMARY KEY (id, created_at);


--
-- Name: audit_logs_2026_11 audit_logs_2026_11_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs_2026_11
    ADD CONSTRAINT audit_logs_2026_11_pkey PRIMARY KEY (id, created_at);


--
-- Name: audit_logs_2026_12 audit_logs_2026_12_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs_2026_12
    ADD CONSTRAINT audit_logs_2026_12_pkey PRIMARY KEY (id, created_at);


--
-- Name: backflush_items backflush_items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.backflush_items
    ADD CONSTRAINT backflush_items_pkey PRIMARY KEY (id);


--
-- Name: backflush_records backflush_records_doc_number_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.backflush_records
    ADD CONSTRAINT backflush_records_doc_number_key UNIQUE (doc_number);


--
-- Name: backflush_records backflush_records_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.backflush_records
    ADD CONSTRAINT backflush_records_pkey PRIMARY KEY (id);


--
-- Name: bins bins_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.bins
    ADD CONSTRAINT bins_pkey PRIMARY KEY (id);


--
-- Name: bins bins_zone_id_code_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.bins
    ADD CONSTRAINT bins_zone_id_code_key UNIQUE (zone_id, code);


--
-- Name: bom_categories bom_categories_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.bom_categories
    ADD CONSTRAINT bom_categories_pkey PRIMARY KEY (bom_category_id);


--
-- Name: bom_labor_processes bom_labor_processes_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.bom_labor_processes
    ADD CONSTRAINT bom_labor_processes_pkey PRIMARY KEY (id);


--
-- Name: bom_nodes bom_nodes_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.bom_nodes
    ADD CONSTRAINT bom_nodes_pkey PRIMARY KEY (node_id);


--
-- Name: bom_routings bom_routings_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.bom_routings
    ADD CONSTRAINT bom_routings_pkey PRIMARY KEY (id);


--
-- Name: bom_snapshots bom_snapshots_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.bom_snapshots
    ADD CONSTRAINT bom_snapshots_pkey PRIMARY KEY (snapshot_id);


--
-- Name: boms boms_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.boms
    ADD CONSTRAINT boms_pkey PRIMARY KEY (bom_id);


--
-- Name: cash_journal_lines cash_journal_lines_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cash_journal_lines
    ADD CONSTRAINT cash_journal_lines_pkey PRIMARY KEY (id);


--
-- Name: cash_journals cash_journals_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cash_journals
    ADD CONSTRAINT cash_journals_pkey PRIMARY KEY (id);


--
-- Name: categories categories_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.categories
    ADD CONSTRAINT categories_pkey PRIMARY KEY (category_id);


--
-- Name: conversion_items conversion_items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.conversion_items
    ADD CONSTRAINT conversion_items_pkey PRIMARY KEY (id);


--
-- Name: cost_entries cost_entries_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cost_entries
    ADD CONSTRAINT cost_entries_pkey PRIMARY KEY (id);


--
-- Name: customer_addresses customer_addresses_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.customer_addresses
    ADD CONSTRAINT customer_addresses_pkey PRIMARY KEY (address_id);


--
-- Name: customer_contacts customer_contacts_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.customer_contacts
    ADD CONSTRAINT customer_contacts_pkey PRIMARY KEY (contact_id);


--
-- Name: customers customers_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.customers
    ADD CONSTRAINT customers_pkey PRIMARY KEY (customer_id);


--
-- Name: cycle_count_items cycle_count_items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cycle_count_items
    ADD CONSTRAINT cycle_count_items_pkey PRIMARY KEY (id);


--
-- Name: cycle_counts cycle_counts_doc_number_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cycle_counts
    ADD CONSTRAINT cycle_counts_doc_number_key UNIQUE (doc_number);


--
-- Name: cycle_counts cycle_counts_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cycle_counts
    ADD CONSTRAINT cycle_counts_pkey PRIMARY KEY (id);


--
-- Name: departments departments_department_code_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.departments
    ADD CONSTRAINT departments_department_code_key UNIQUE (department_code);


--
-- Name: departments departments_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.departments
    ADD CONSTRAINT departments_pkey PRIMARY KEY (department_id);


--
-- Name: document_links document_links_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.document_links
    ADD CONSTRAINT document_links_pkey PRIMARY KEY (id);


--
-- Name: document_sequences document_sequences_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.document_sequences
    ADD CONSTRAINT document_sequences_pkey PRIMARY KEY (id);


--
-- Name: document_sequences document_sequences_prefix_seq_date_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.document_sequences
    ADD CONSTRAINT document_sequences_prefix_seq_date_key UNIQUE (prefix, seq_date);


--
-- Name: domain_events domain_events_idempotency_key_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.domain_events
    ADD CONSTRAINT domain_events_idempotency_key_key UNIQUE (idempotency_key);


--
-- Name: domain_events domain_events_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.domain_events
    ADD CONSTRAINT domain_events_pkey PRIMARY KEY (id);


--
-- Name: entity_state_logs entity_state_logs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.entity_state_logs
    ADD CONSTRAINT entity_state_logs_pkey PRIMARY KEY (id);


--
-- Name: expense_reimbursement_items expense_reimbursement_items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.expense_reimbursement_items
    ADD CONSTRAINT expense_reimbursement_items_pkey PRIMARY KEY (id);


--
-- Name: expense_reimbursements expense_reimbursements_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.expense_reimbursements
    ADD CONSTRAINT expense_reimbursements_pkey PRIMARY KEY (id);


--
-- Name: form_conversions form_conversions_doc_number_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.form_conversions
    ADD CONSTRAINT form_conversions_doc_number_key UNIQUE (doc_number);


--
-- Name: form_conversions form_conversions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.form_conversions
    ADD CONSTRAINT form_conversions_pkey PRIMARY KEY (id);


--
-- Name: idempotency_records idempotency_records_idempotency_key_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.idempotency_records
    ADD CONSTRAINT idempotency_records_idempotency_key_key UNIQUE (idempotency_key);


--
-- Name: idempotency_records idempotency_records_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.idempotency_records
    ADD CONSTRAINT idempotency_records_pkey PRIMARY KEY (id);


--
-- Name: inspection_results inspection_results_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.inspection_results
    ADD CONSTRAINT inspection_results_pkey PRIMARY KEY (id);


--
-- Name: inspection_specifications inspection_specifications_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.inspection_specifications
    ADD CONSTRAINT inspection_specifications_pkey PRIMARY KEY (id);


--
-- Name: inventory_locks inventory_locks_doc_number_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.inventory_locks
    ADD CONSTRAINT inventory_locks_doc_number_key UNIQUE (doc_number);


--
-- Name: inventory_locks inventory_locks_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.inventory_locks
    ADD CONSTRAINT inventory_locks_pkey PRIMARY KEY (id);


--
-- Name: inventory_reservations inventory_reservations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.inventory_reservations
    ADD CONSTRAINT inventory_reservations_pkey PRIMARY KEY (id);


--
-- Name: inventory_transactions inventory_transactions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.inventory_transactions
    ADD CONSTRAINT inventory_transactions_pkey PRIMARY KEY (id);


--
-- Name: inventory_transfers inventory_transfers_doc_number_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.inventory_transfers
    ADD CONSTRAINT inventory_transfers_doc_number_key UNIQUE (doc_number);


--
-- Name: inventory_transfers inventory_transfers_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.inventory_transfers
    ADD CONSTRAINT inventory_transfers_pkey PRIMARY KEY (id);


--
-- Name: labor_process_dicts labor_process_dicts_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.labor_process_dicts
    ADD CONSTRAINT labor_process_dicts_pkey PRIMARY KEY (id);


--
-- Name: material_requisition_items material_requisition_items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.material_requisition_items
    ADD CONSTRAINT material_requisition_items_pkey PRIMARY KEY (id);


--
-- Name: material_requisitions material_requisitions_doc_number_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.material_requisitions
    ADD CONSTRAINT material_requisitions_doc_number_key UNIQUE (doc_number);


--
-- Name: material_requisitions material_requisitions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.material_requisitions
    ADD CONSTRAINT material_requisitions_pkey PRIMARY KEY (id);


--
-- Name: misc_request_items misc_request_items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.misc_request_items
    ADD CONSTRAINT misc_request_items_pkey PRIMARY KEY (id);


--
-- Name: miscellaneous_requests miscellaneous_requests_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.miscellaneous_requests
    ADD CONSTRAINT miscellaneous_requests_pkey PRIMARY KEY (id);


--
-- Name: mrbs mrbs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.mrbs
    ADD CONSTRAINT mrbs_pkey PRIMARY KEY (id);


--
-- Name: notifications notifications_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.notifications
    ADD CONSTRAINT notifications_pkey PRIMARY KEY (notification_id);


--
-- Name: outsourcing_materials outsourcing_materials_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.outsourcing_materials
    ADD CONSTRAINT outsourcing_materials_pkey PRIMARY KEY (id);


--
-- Name: outsourcing_orders outsourcing_orders_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.outsourcing_orders
    ADD CONSTRAINT outsourcing_orders_pkey PRIMARY KEY (id);


--
-- Name: outsourcing_trackings outsourcing_trackings_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.outsourcing_trackings
    ADD CONSTRAINT outsourcing_trackings_pkey PRIMARY KEY (id);


--
-- Name: payment_requests payment_requests_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.payment_requests
    ADD CONSTRAINT payment_requests_pkey PRIMARY KEY (id);


--
-- Name: pick_strategies pick_strategies_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.pick_strategies
    ADD CONSTRAINT pick_strategies_pkey PRIMARY KEY (id);


--
-- Name: price_log price_log_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.price_log
    ADD CONSTRAINT price_log_pkey PRIMARY KEY (log_id);


--
-- Name: product_categories product_categories_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.product_categories
    ADD CONSTRAINT product_categories_pkey PRIMARY KEY (product_id, category_id);


--
-- Name: product_watchers product_watchers_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.product_watchers
    ADD CONSTRAINT product_watchers_pkey PRIMARY KEY (user_id, product_id);


--
-- Name: production_batches production_batches_batch_no_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.production_batches
    ADD CONSTRAINT production_batches_batch_no_key UNIQUE (batch_no);


--
-- Name: production_batches production_batches_card_sn_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.production_batches
    ADD CONSTRAINT production_batches_card_sn_key UNIQUE (card_sn);


--
-- Name: production_batches production_batches_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.production_batches
    ADD CONSTRAINT production_batches_pkey PRIMARY KEY (id);


--
-- Name: production_inspections production_inspections_doc_number_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.production_inspections
    ADD CONSTRAINT production_inspections_doc_number_key UNIQUE (doc_number);


--
-- Name: production_inspections production_inspections_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.production_inspections
    ADD CONSTRAINT production_inspections_pkey PRIMARY KEY (id);


--
-- Name: production_plan_items production_plan_items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.production_plan_items
    ADD CONSTRAINT production_plan_items_pkey PRIMARY KEY (id);


--
-- Name: production_plan_items production_plan_items_plan_id_product_id_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.production_plan_items
    ADD CONSTRAINT production_plan_items_plan_id_product_id_key UNIQUE (plan_id, product_id);


--
-- Name: production_plans production_plans_doc_number_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.production_plans
    ADD CONSTRAINT production_plans_doc_number_key UNIQUE (doc_number);


--
-- Name: production_plans production_plans_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.production_plans
    ADD CONSTRAINT production_plans_pkey PRIMARY KEY (id);


--
-- Name: production_receipts production_receipts_doc_number_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.production_receipts
    ADD CONSTRAINT production_receipts_doc_number_key UNIQUE (doc_number);


--
-- Name: production_receipts production_receipts_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.production_receipts
    ADD CONSTRAINT production_receipts_pkey PRIMARY KEY (id);


--
-- Name: products products_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.products
    ADD CONSTRAINT products_pkey PRIMARY KEY (product_id);


--
-- Name: purchase_order_items purchase_order_items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.purchase_order_items
    ADD CONSTRAINT purchase_order_items_pkey PRIMARY KEY (id);


--
-- Name: purchase_orders purchase_orders_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.purchase_orders
    ADD CONSTRAINT purchase_orders_pkey PRIMARY KEY (id);


--
-- Name: purchase_quotation_items purchase_quotation_items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.purchase_quotation_items
    ADD CONSTRAINT purchase_quotation_items_pkey PRIMARY KEY (id);


--
-- Name: purchase_quotations purchase_quotations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.purchase_quotations
    ADD CONSTRAINT purchase_quotations_pkey PRIMARY KEY (id);


--
-- Name: purchase_recon_items purchase_recon_items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.purchase_recon_items
    ADD CONSTRAINT purchase_recon_items_pkey PRIMARY KEY (id);


--
-- Name: purchase_reconciliations purchase_reconciliations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.purchase_reconciliations
    ADD CONSTRAINT purchase_reconciliations_pkey PRIMARY KEY (id);


--
-- Name: purchase_return_items purchase_return_items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.purchase_return_items
    ADD CONSTRAINT purchase_return_items_pkey PRIMARY KEY (id);


--
-- Name: purchase_returns purchase_returns_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.purchase_returns
    ADD CONSTRAINT purchase_returns_pkey PRIMARY KEY (id);


--
-- Name: putaway_strategies putaway_strategies_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.putaway_strategies
    ADD CONSTRAINT putaway_strategies_pkey PRIMARY KEY (id);


--
-- Name: quotation_items quotation_items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.quotation_items
    ADD CONSTRAINT quotation_items_pkey PRIMARY KEY (id);


--
-- Name: quotations quotations_doc_number_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.quotations
    ADD CONSTRAINT quotations_doc_number_key UNIQUE (doc_number);


--
-- Name: quotations quotations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.quotations
    ADD CONSTRAINT quotations_pkey PRIMARY KEY (id);


--
-- Name: reconciliation_items reconciliation_items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.reconciliation_items
    ADD CONSTRAINT reconciliation_items_pkey PRIMARY KEY (id);


--
-- Name: reconciliations reconciliations_customer_id_period_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.reconciliations
    ADD CONSTRAINT reconciliations_customer_id_period_key UNIQUE (customer_id, period);


--
-- Name: reconciliations reconciliations_doc_number_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.reconciliations
    ADD CONSTRAINT reconciliations_doc_number_key UNIQUE (doc_number);


--
-- Name: reconciliations reconciliations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.reconciliations
    ADD CONSTRAINT reconciliations_pkey PRIMARY KEY (id);


--
-- Name: rmas rmas_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.rmas
    ADD CONSTRAINT rmas_pkey PRIMARY KEY (id);


--
-- Name: role_permissions role_permissions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.role_permissions
    ADD CONSTRAINT role_permissions_pkey PRIMARY KEY (role_id, resource_code, action);


--
-- Name: roles roles_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.roles
    ADD CONSTRAINT roles_pkey PRIMARY KEY (role_id);


--
-- Name: roles roles_role_code_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.roles
    ADD CONSTRAINT roles_role_code_key UNIQUE (role_code);


--
-- Name: routing_steps routing_steps_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.routing_steps
    ADD CONSTRAINT routing_steps_pkey PRIMARY KEY (id);


--
-- Name: routings routings_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.routings
    ADD CONSTRAINT routings_pkey PRIMARY KEY (id);


--
-- Name: sales_order_items sales_order_items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.sales_order_items
    ADD CONSTRAINT sales_order_items_pkey PRIMARY KEY (id);


--
-- Name: sales_orders sales_orders_doc_number_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.sales_orders
    ADD CONSTRAINT sales_orders_doc_number_key UNIQUE (doc_number);


--
-- Name: sales_orders sales_orders_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.sales_orders
    ADD CONSTRAINT sales_orders_pkey PRIMARY KEY (id);


--
-- Name: sales_return_items sales_return_items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.sales_return_items
    ADD CONSTRAINT sales_return_items_pkey PRIMARY KEY (id);


--
-- Name: sales_returns sales_returns_doc_number_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.sales_returns
    ADD CONSTRAINT sales_returns_doc_number_key UNIQUE (doc_number);


--
-- Name: sales_returns sales_returns_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.sales_returns
    ADD CONSTRAINT sales_returns_pkey PRIMARY KEY (id);


--
-- Name: scheduled_task_defs scheduled_task_defs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.scheduled_task_defs
    ADD CONSTRAINT scheduled_task_defs_pkey PRIMARY KEY (task_id);


--
-- Name: shipping_request_items shipping_request_items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.shipping_request_items
    ADD CONSTRAINT shipping_request_items_pkey PRIMARY KEY (id);


--
-- Name: shipping_requests shipping_requests_doc_number_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.shipping_requests
    ADD CONSTRAINT shipping_requests_doc_number_key UNIQUE (doc_number);


--
-- Name: shipping_requests shipping_requests_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.shipping_requests
    ADD CONSTRAINT shipping_requests_pkey PRIMARY KEY (id);


--
-- Name: state_definitions state_definitions_entity_type_state_name_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.state_definitions
    ADD CONSTRAINT state_definitions_entity_type_state_name_key UNIQUE (entity_type, state_name);


--
-- Name: state_definitions state_definitions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.state_definitions
    ADD CONSTRAINT state_definitions_pkey PRIMARY KEY (id);


--
-- Name: state_transition_defs state_transition_defs_entity_type_from_state_to_state_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.state_transition_defs
    ADD CONSTRAINT state_transition_defs_entity_type_from_state_to_state_key UNIQUE (entity_type, from_state, to_state);


--
-- Name: state_transition_defs state_transition_defs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.state_transition_defs
    ADD CONSTRAINT state_transition_defs_pkey PRIMARY KEY (id);


--
-- Name: stock_ledger stock_ledger_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.stock_ledger
    ADD CONSTRAINT stock_ledger_pkey PRIMARY KEY (id);


--
-- Name: supplier_bank_accounts supplier_bank_accounts_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.supplier_bank_accounts
    ADD CONSTRAINT supplier_bank_accounts_pkey PRIMARY KEY (account_id);


--
-- Name: supplier_contacts supplier_contacts_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.supplier_contacts
    ADD CONSTRAINT supplier_contacts_pkey PRIMARY KEY (contact_id);


--
-- Name: suppliers suppliers_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.suppliers
    ADD CONSTRAINT suppliers_pkey PRIMARY KEY (supplier_id);


--
-- Name: task_run_logs task_run_logs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.task_run_logs
    ADD CONSTRAINT task_run_logs_pkey PRIMARY KEY (run_id);


--
-- Name: transfer_items transfer_items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transfer_items
    ADD CONSTRAINT transfer_items_pkey PRIMARY KEY (id);


--
-- Name: user_departments user_departments_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_departments
    ADD CONSTRAINT user_departments_pkey PRIMARY KEY (user_id, department_id);


--
-- Name: user_roles user_roles_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_roles
    ADD CONSTRAINT user_roles_pkey PRIMARY KEY (user_id, role_id);


--
-- Name: users users_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_pkey PRIMARY KEY (user_id);


--
-- Name: users users_username_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_username_key UNIQUE (username);


--
-- Name: warehouses warehouses_code_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.warehouses
    ADD CONSTRAINT warehouses_code_key UNIQUE (code);


--
-- Name: warehouses warehouses_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.warehouses
    ADD CONSTRAINT warehouses_pkey PRIMARY KEY (id);


--
-- Name: work_order_routings work_order_routings_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.work_order_routings
    ADD CONSTRAINT work_order_routings_pkey PRIMARY KEY (id);


--
-- Name: work_order_routings work_order_routings_work_order_id_step_no_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.work_order_routings
    ADD CONSTRAINT work_order_routings_work_order_id_step_no_key UNIQUE (work_order_id, step_no);


--
-- Name: work_orders work_orders_doc_number_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.work_orders
    ADD CONSTRAINT work_orders_doc_number_key UNIQUE (doc_number);


--
-- Name: work_orders work_orders_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.work_orders
    ADD CONSTRAINT work_orders_pkey PRIMARY KEY (id);


--
-- Name: work_reports work_reports_batch_id_routing_id_worker_id_shift_report_dat_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.work_reports
    ADD CONSTRAINT work_reports_batch_id_routing_id_worker_id_shift_report_dat_key UNIQUE (batch_id, routing_id, worker_id, shift, report_date);


--
-- Name: work_reports work_reports_doc_number_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.work_reports
    ADD CONSTRAINT work_reports_doc_number_key UNIQUE (doc_number);


--
-- Name: work_reports work_reports_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.work_reports
    ADD CONSTRAINT work_reports_pkey PRIMARY KEY (id);


--
-- Name: workflow_history workflow_history_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.workflow_history
    ADD CONSTRAINT workflow_history_pkey PRIMARY KEY (id);


--
-- Name: workflow_instances workflow_instances_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.workflow_instances
    ADD CONSTRAINT workflow_instances_pkey PRIMARY KEY (id);


--
-- Name: workflow_tasks workflow_tasks_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.workflow_tasks
    ADD CONSTRAINT workflow_tasks_pkey PRIMARY KEY (id);


--
-- Name: workflow_templates workflow_templates_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.workflow_templates
    ADD CONSTRAINT workflow_templates_pkey PRIMARY KEY (id);


--
-- Name: write_offs write_offs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.write_offs
    ADD CONSTRAINT write_offs_pkey PRIMARY KEY (id);


--
-- Name: zones zones_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.zones
    ADD CONSTRAINT zones_pkey PRIMARY KEY (id);


--
-- Name: zones zones_warehouse_id_code_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.zones
    ADD CONSTRAINT zones_warehouse_id_code_key UNIQUE (warehouse_id, code);


--
-- Name: idx_audit_logs_created_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_logs_created_at ON ONLY public.audit_logs USING btree (created_at);


--
-- Name: audit_logs_2026_01_created_at_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_01_created_at_idx ON public.audit_logs_2026_01 USING btree (created_at);


--
-- Name: idx_audit_logs_entity; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_logs_entity ON ONLY public.audit_logs USING btree (entity_type, entity_id);


--
-- Name: audit_logs_2026_01_entity_type_entity_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_01_entity_type_entity_id_idx ON public.audit_logs_2026_01 USING btree (entity_type, entity_id);


--
-- Name: idx_audit_logs_operator; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_logs_operator ON ONLY public.audit_logs USING btree (operator_id);


--
-- Name: audit_logs_2026_01_operator_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_01_operator_id_idx ON public.audit_logs_2026_01 USING btree (operator_id);


--
-- Name: audit_logs_2026_02_created_at_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_02_created_at_idx ON public.audit_logs_2026_02 USING btree (created_at);


--
-- Name: audit_logs_2026_02_entity_type_entity_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_02_entity_type_entity_id_idx ON public.audit_logs_2026_02 USING btree (entity_type, entity_id);


--
-- Name: audit_logs_2026_02_operator_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_02_operator_id_idx ON public.audit_logs_2026_02 USING btree (operator_id);


--
-- Name: audit_logs_2026_03_created_at_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_03_created_at_idx ON public.audit_logs_2026_03 USING btree (created_at);


--
-- Name: audit_logs_2026_03_entity_type_entity_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_03_entity_type_entity_id_idx ON public.audit_logs_2026_03 USING btree (entity_type, entity_id);


--
-- Name: audit_logs_2026_03_operator_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_03_operator_id_idx ON public.audit_logs_2026_03 USING btree (operator_id);


--
-- Name: audit_logs_2026_04_created_at_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_04_created_at_idx ON public.audit_logs_2026_04 USING btree (created_at);


--
-- Name: audit_logs_2026_04_entity_type_entity_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_04_entity_type_entity_id_idx ON public.audit_logs_2026_04 USING btree (entity_type, entity_id);


--
-- Name: audit_logs_2026_04_operator_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_04_operator_id_idx ON public.audit_logs_2026_04 USING btree (operator_id);


--
-- Name: audit_logs_2026_05_created_at_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_05_created_at_idx ON public.audit_logs_2026_05 USING btree (created_at);


--
-- Name: audit_logs_2026_05_entity_type_entity_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_05_entity_type_entity_id_idx ON public.audit_logs_2026_05 USING btree (entity_type, entity_id);


--
-- Name: audit_logs_2026_05_operator_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_05_operator_id_idx ON public.audit_logs_2026_05 USING btree (operator_id);


--
-- Name: audit_logs_2026_06_created_at_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_06_created_at_idx ON public.audit_logs_2026_06 USING btree (created_at);


--
-- Name: audit_logs_2026_06_entity_type_entity_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_06_entity_type_entity_id_idx ON public.audit_logs_2026_06 USING btree (entity_type, entity_id);


--
-- Name: audit_logs_2026_06_operator_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_06_operator_id_idx ON public.audit_logs_2026_06 USING btree (operator_id);


--
-- Name: audit_logs_2026_07_created_at_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_07_created_at_idx ON public.audit_logs_2026_07 USING btree (created_at);


--
-- Name: audit_logs_2026_07_entity_type_entity_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_07_entity_type_entity_id_idx ON public.audit_logs_2026_07 USING btree (entity_type, entity_id);


--
-- Name: audit_logs_2026_07_operator_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_07_operator_id_idx ON public.audit_logs_2026_07 USING btree (operator_id);


--
-- Name: audit_logs_2026_08_created_at_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_08_created_at_idx ON public.audit_logs_2026_08 USING btree (created_at);


--
-- Name: audit_logs_2026_08_entity_type_entity_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_08_entity_type_entity_id_idx ON public.audit_logs_2026_08 USING btree (entity_type, entity_id);


--
-- Name: audit_logs_2026_08_operator_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_08_operator_id_idx ON public.audit_logs_2026_08 USING btree (operator_id);


--
-- Name: audit_logs_2026_09_created_at_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_09_created_at_idx ON public.audit_logs_2026_09 USING btree (created_at);


--
-- Name: audit_logs_2026_09_entity_type_entity_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_09_entity_type_entity_id_idx ON public.audit_logs_2026_09 USING btree (entity_type, entity_id);


--
-- Name: audit_logs_2026_09_operator_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_09_operator_id_idx ON public.audit_logs_2026_09 USING btree (operator_id);


--
-- Name: audit_logs_2026_10_created_at_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_10_created_at_idx ON public.audit_logs_2026_10 USING btree (created_at);


--
-- Name: audit_logs_2026_10_entity_type_entity_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_10_entity_type_entity_id_idx ON public.audit_logs_2026_10 USING btree (entity_type, entity_id);


--
-- Name: audit_logs_2026_10_operator_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_10_operator_id_idx ON public.audit_logs_2026_10 USING btree (operator_id);


--
-- Name: audit_logs_2026_11_created_at_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_11_created_at_idx ON public.audit_logs_2026_11 USING btree (created_at);


--
-- Name: audit_logs_2026_11_entity_type_entity_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_11_entity_type_entity_id_idx ON public.audit_logs_2026_11 USING btree (entity_type, entity_id);


--
-- Name: audit_logs_2026_11_operator_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_11_operator_id_idx ON public.audit_logs_2026_11 USING btree (operator_id);


--
-- Name: audit_logs_2026_12_created_at_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_12_created_at_idx ON public.audit_logs_2026_12 USING btree (created_at);


--
-- Name: audit_logs_2026_12_entity_type_entity_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_12_entity_type_entity_id_idx ON public.audit_logs_2026_12 USING btree (entity_type, entity_id);


--
-- Name: audit_logs_2026_12_operator_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX audit_logs_2026_12_operator_id_idx ON public.audit_logs_2026_12 USING btree (operator_id);


--
-- Name: idx_ani_notice; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_ani_notice ON public.arrival_notice_items USING btree (notice_id);


--
-- Name: idx_arrival_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_arrival_status ON public.arrival_notices USING btree (status) WHERE (deleted_at IS NULL);


--
-- Name: idx_arrival_supplier; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_arrival_supplier ON public.arrival_notices USING btree (supplier_id) WHERE (deleted_at IS NULL);


--
-- Name: idx_batches_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_batches_status ON public.production_batches USING btree (status);


--
-- Name: idx_batches_work_order; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_batches_work_order ON public.production_batches USING btree (work_order_id);


--
-- Name: idx_bi_record; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_bi_record ON public.backflush_items USING btree (record_id);


--
-- Name: idx_bins_unique_active; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_bins_unique_active ON public.bins USING btree (zone_id, code) WHERE (deleted_at IS NULL);


--
-- Name: idx_bom_labor_processes_product_code; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_bom_labor_processes_product_code ON public.bom_labor_processes USING btree (product_code) WHERE (deleted_at IS NULL);


--
-- Name: idx_bom_labor_processes_sort_order; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_bom_labor_processes_sort_order ON public.bom_labor_processes USING btree (sort_order);


--
-- Name: idx_bom_nodes_bom_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_bom_nodes_bom_id ON public.bom_nodes USING btree (bom_id);


--
-- Name: idx_bom_nodes_bom_id_parent; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_bom_nodes_bom_id_parent ON public.bom_nodes USING btree (bom_id, parent_id);


--
-- Name: idx_bom_nodes_bom_id_product; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_bom_nodes_bom_id_product ON public.bom_nodes USING btree (bom_id, product_id);


--
-- Name: idx_bom_nodes_parent_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_bom_nodes_parent_id ON public.bom_nodes USING btree (parent_id);


--
-- Name: idx_bom_routings_product_code; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_bom_routings_product_code ON public.bom_routings USING btree (product_code);


--
-- Name: idx_bom_routings_routing_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_bom_routings_routing_id ON public.bom_routings USING btree (routing_id);


--
-- Name: idx_bom_snapshots_bom_version; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_bom_snapshots_bom_version ON public.bom_snapshots USING btree (bom_id, version DESC);


--
-- Name: idx_boms_bom_category_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_boms_bom_category_id ON public.boms USING btree (bom_category_id);


--
-- Name: idx_boms_bom_name; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_boms_bom_name ON public.boms USING gin (bom_name public.gin_trgm_ops);


--
-- Name: idx_boms_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_boms_status ON public.boms USING btree (status);


--
-- Name: idx_categories_name_parent; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_categories_name_parent ON public.categories USING btree (category_name, parent_id);


--
-- Name: idx_categories_parent; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_categories_parent ON public.categories USING btree (parent_id);


--
-- Name: idx_categories_path; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_categories_path ON public.categories USING btree (path);


--
-- Name: idx_cci_count; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cci_count ON public.cycle_count_items USING btree (count_id);


--
-- Name: idx_ci_conversion; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_ci_conversion ON public.conversion_items USING btree (conversion_id);


--
-- Name: idx_cj_counterparty; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cj_counterparty ON public.cash_journals USING btree (counterparty_type, counterparty_id);


--
-- Name: idx_cj_doc_number; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_cj_doc_number ON public.cash_journals USING btree (doc_number) WHERE (deleted_at IS NULL);


--
-- Name: idx_cj_period; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cj_period ON public.cash_journals USING btree (period) WHERE (deleted_at IS NULL);


--
-- Name: idx_cj_transaction_date; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cj_transaction_date ON public.cash_journals USING btree (transaction_date);


--
-- Name: idx_cj_type_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cj_type_status ON public.cash_journals USING btree (journal_type, status) WHERE (deleted_at IS NULL);


--
-- Name: idx_cjl_journal; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cjl_journal ON public.cash_journal_lines USING btree (journal_id);


--
-- Name: idx_cost_entries_entity; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cost_entries_entity ON public.cost_entries USING btree (entity_type, entity_id);


--
-- Name: idx_cost_entries_period; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cost_entries_period ON public.cost_entries USING btree (period);


--
-- Name: idx_customer_addresses_customer_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_customer_addresses_customer_id ON public.customer_addresses USING btree (customer_id);


--
-- Name: idx_customer_contacts_customer_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_customer_contacts_customer_id ON public.customer_contacts USING btree (customer_id);


--
-- Name: idx_customers_category; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_customers_category ON public.customers USING btree (category) WHERE (deleted_at IS NULL);


--
-- Name: idx_customers_customer_code; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_customers_customer_code ON public.customers USING btree (customer_code) WHERE (deleted_at IS NULL);


--
-- Name: idx_customers_department_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_customers_department_id ON public.customers USING btree (department_id) WHERE (deleted_at IS NULL);


--
-- Name: idx_customers_name_trgm; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_customers_name_trgm ON public.customers USING gin (customer_name public.gin_trgm_ops);


--
-- Name: idx_customers_owner_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_customers_owner_id ON public.customers USING btree (owner_id) WHERE (deleted_at IS NULL);


--
-- Name: idx_customers_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_customers_status ON public.customers USING btree (status) WHERE (deleted_at IS NULL);


--
-- Name: idx_doc_links_path; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_doc_links_path ON public.document_links USING gin (path public.gin_trgm_ops);


--
-- Name: idx_doc_links_source; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_doc_links_source ON public.document_links USING btree (source_type, source_id);


--
-- Name: idx_doc_links_target; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_doc_links_target ON public.document_links USING btree (target_type, target_id);


--
-- Name: idx_domain_events_aggregate; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_domain_events_aggregate ON public.domain_events USING btree (aggregate_type, aggregate_id);


--
-- Name: idx_domain_events_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_domain_events_status ON public.domain_events USING btree (status, created_at);


--
-- Name: idx_er_applicant; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_er_applicant ON public.expense_reimbursements USING btree (applicant_id) WHERE (deleted_at IS NULL);


--
-- Name: idx_er_doc_number; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_er_doc_number ON public.expense_reimbursements USING btree (doc_number) WHERE (deleted_at IS NULL);


--
-- Name: idx_er_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_er_status ON public.expense_reimbursements USING btree (status) WHERE (deleted_at IS NULL);


--
-- Name: idx_eri_reimbursement; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_eri_reimbursement ON public.expense_reimbursement_items USING btree (reimbursement_id);


--
-- Name: idx_idempotency_event_handler; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_idempotency_event_handler ON public.idempotency_records USING btree (event_id, handler_name);


--
-- Name: idx_inspection_results_doc_number; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_inspection_results_doc_number ON public.inspection_results USING btree (doc_number) WHERE (deleted_at IS NULL);


--
-- Name: idx_inspection_results_idempotent; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_inspection_results_idempotent ON public.inspection_results USING btree (source_type, source_id, inspection_type) WHERE (deleted_at IS NULL);


--
-- Name: idx_inspection_results_source; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_inspection_results_source ON public.inspection_results USING btree (source_type, source_id) WHERE (deleted_at IS NULL);


--
-- Name: idx_inspection_results_spec; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_inspection_results_spec ON public.inspection_results USING btree (spec_id) WHERE (deleted_at IS NULL);


--
-- Name: idx_inspection_results_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_inspection_results_type ON public.inspection_results USING btree (inspection_type) WHERE (deleted_at IS NULL);


--
-- Name: idx_inspection_specs_doc_number; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_inspection_specs_doc_number ON public.inspection_specifications USING btree (doc_number) WHERE (deleted_at IS NULL);


--
-- Name: idx_inspection_specs_product; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_inspection_specs_product ON public.inspection_specifications USING btree (product_id) WHERE (deleted_at IS NULL);


--
-- Name: idx_inspection_specs_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_inspection_specs_status ON public.inspection_specifications USING btree (status) WHERE (deleted_at IS NULL);


--
-- Name: idx_inspection_specs_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_inspection_specs_type ON public.inspection_specifications USING btree (inspection_type) WHERE (deleted_at IS NULL);


--
-- Name: idx_inspections_product; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_inspections_product ON public.production_inspections USING btree (product_id);


--
-- Name: idx_inspections_work_order; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_inspections_work_order ON public.production_inspections USING btree (work_order_id);


--
-- Name: idx_inv_res_product; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_inv_res_product ON public.inventory_reservations USING btree (product_id, warehouse_id, status);


--
-- Name: idx_inv_res_source; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_inv_res_source ON public.inventory_reservations USING btree (source_type, source_id);


--
-- Name: idx_labor_process_dicts_code; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_labor_process_dicts_code ON public.labor_process_dicts USING btree (code) WHERE (deleted_at IS NULL);


--
-- Name: idx_labor_process_dicts_sort_order; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_labor_process_dicts_sort_order ON public.labor_process_dicts USING btree (sort_order);


--
-- Name: idx_misc_department; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_misc_department ON public.miscellaneous_requests USING btree (department_id);


--
-- Name: idx_misc_doc_number; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_misc_doc_number ON public.miscellaneous_requests USING btree (doc_number) WHERE (deleted_at IS NULL);


--
-- Name: idx_misc_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_misc_status ON public.miscellaneous_requests USING btree (status) WHERE (deleted_at IS NULL);


--
-- Name: idx_mrbs_doc_number; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_mrbs_doc_number ON public.mrbs USING btree (doc_number) WHERE (deleted_at IS NULL);


--
-- Name: idx_mrbs_inspection_result; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_mrbs_inspection_result ON public.mrbs USING btree (inspection_result_id) WHERE (deleted_at IS NULL);


--
-- Name: idx_mrbs_product; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_mrbs_product ON public.mrbs USING btree (product_id) WHERE (deleted_at IS NULL);


--
-- Name: idx_mrbs_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_mrbs_status ON public.mrbs USING btree (status) WHERE (deleted_at IS NULL);


--
-- Name: idx_mri_request; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_mri_request ON public.misc_request_items USING btree (request_id);


--
-- Name: idx_mri_requisition; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_mri_requisition ON public.material_requisition_items USING btree (requisition_id);


--
-- Name: idx_notifications_user_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_notifications_user_id ON public.notifications USING btree (user_id);


--
-- Name: idx_notifications_user_read; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_notifications_user_read ON public.notifications USING btree (user_id, is_read);


--
-- Name: idx_notifications_user_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_notifications_user_type ON public.notifications USING btree (user_id, notification_type);


--
-- Name: idx_outsourcing_materials_order_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_outsourcing_materials_order_id ON public.outsourcing_materials USING btree (outsourcing_id);


--
-- Name: idx_outsourcing_orders_doc_number; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_outsourcing_orders_doc_number ON public.outsourcing_orders USING btree (doc_number);


--
-- Name: idx_outsourcing_orders_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_outsourcing_orders_status ON public.outsourcing_orders USING btree (status);


--
-- Name: idx_outsourcing_orders_supplier_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_outsourcing_orders_supplier_id ON public.outsourcing_orders USING btree (supplier_id);


--
-- Name: idx_outsourcing_orders_work_order; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_outsourcing_orders_work_order ON public.outsourcing_orders USING btree (work_order_id) WHERE (work_order_id IS NOT NULL);


--
-- Name: idx_outsourcing_trackings_node_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_outsourcing_trackings_node_type ON public.outsourcing_trackings USING btree (outsourcing_id, node_type);


--
-- Name: idx_outsourcing_trackings_order_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_outsourcing_trackings_order_id ON public.outsourcing_trackings USING btree (outsourcing_id);


--
-- Name: idx_outsourcing_trackings_overdue; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_outsourcing_trackings_overdue ON public.outsourcing_trackings USING btree (planned_at) WHERE (planned_at IS NOT NULL);


--
-- Name: idx_pay_doc_number; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_pay_doc_number ON public.payment_requests USING btree (doc_number) WHERE (deleted_at IS NULL);


--
-- Name: idx_pay_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_pay_status ON public.payment_requests USING btree (status) WHERE (deleted_at IS NULL);


--
-- Name: idx_pay_supplier; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_pay_supplier ON public.payment_requests USING btree (supplier_id);


--
-- Name: idx_plan_items_plan; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_plan_items_plan ON public.production_plan_items USING btree (plan_id);


--
-- Name: idx_po_doc_number; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_po_doc_number ON public.purchase_orders USING btree (doc_number) WHERE (deleted_at IS NULL);


--
-- Name: idx_po_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_po_status ON public.purchase_orders USING btree (status) WHERE (deleted_at IS NULL);


--
-- Name: idx_po_supplier; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_po_supplier ON public.purchase_orders USING btree (supplier_id);


--
-- Name: idx_poi_order; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_poi_order ON public.purchase_order_items USING btree (order_id);


--
-- Name: idx_poi_product; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_poi_product ON public.purchase_order_items USING btree (product_id);


--
-- Name: idx_pq_doc_number; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_pq_doc_number ON public.purchase_quotations USING btree (doc_number) WHERE (deleted_at IS NULL);


--
-- Name: idx_pq_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_pq_status ON public.purchase_quotations USING btree (status) WHERE (deleted_at IS NULL);


--
-- Name: idx_pq_supplier; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_pq_supplier ON public.purchase_quotations USING btree (supplier_id);


--
-- Name: idx_pqi_product; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_pqi_product ON public.purchase_quotation_items USING btree (product_id);


--
-- Name: idx_pqi_quotation; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_pqi_quotation ON public.purchase_quotation_items USING btree (quotation_id);


--
-- Name: idx_prc_doc_number; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_prc_doc_number ON public.purchase_reconciliations USING btree (doc_number) WHERE (deleted_at IS NULL);


--
-- Name: idx_prc_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_prc_status ON public.purchase_reconciliations USING btree (status) WHERE (deleted_at IS NULL);


--
-- Name: idx_prc_supplier_period; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_prc_supplier_period ON public.purchase_reconciliations USING btree (supplier_id, period) WHERE (deleted_at IS NULL);


--
-- Name: idx_prci_reconciliation; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_prci_reconciliation ON public.purchase_recon_items USING btree (reconciliation_id);


--
-- Name: idx_pri_return; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_pri_return ON public.purchase_return_items USING btree (return_id);


--
-- Name: idx_price_log_product_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_price_log_product_type ON public.price_log USING btree (product_id, price_type, created_at DESC);


--
-- Name: idx_product_watchers_product; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_product_watchers_product ON public.product_watchers USING btree (product_id);


--
-- Name: idx_production_plans_date; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_production_plans_date ON public.production_plans USING btree (plan_date);


--
-- Name: idx_production_plans_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_production_plans_status ON public.production_plans USING btree (status);


--
-- Name: idx_products_owner_department; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_products_owner_department ON public.products USING btree (owner_department_id) WHERE (deleted_at IS NULL);


--
-- Name: idx_products_pdt_name; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_products_pdt_name ON public.products USING gin (pdt_name public.gin_trgm_ops);


--
-- Name: idx_products_product_code; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_products_product_code ON public.products USING btree (product_code) WHERE (deleted_at IS NULL);


--
-- Name: idx_products_product_code_trgm; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_products_product_code_trgm ON public.products USING gin (product_code public.gin_trgm_ops);


--
-- Name: idx_products_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_products_status ON public.products USING btree (status) WHERE (deleted_at IS NULL);


--
-- Name: idx_prt_doc_number; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_prt_doc_number ON public.purchase_returns USING btree (doc_number) WHERE (deleted_at IS NULL);


--
-- Name: idx_prt_order; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_prt_order ON public.purchase_returns USING btree (order_id);


--
-- Name: idx_prt_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_prt_status ON public.purchase_returns USING btree (status) WHERE (deleted_at IS NULL);


--
-- Name: idx_prt_supplier; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_prt_supplier ON public.purchase_returns USING btree (supplier_id);


--
-- Name: idx_quotation_items_quotation; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_quotation_items_quotation ON public.quotation_items USING btree (quotation_id);


--
-- Name: idx_quotations_customer; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_quotations_customer ON public.quotations USING btree (customer_id);


--
-- Name: idx_quotations_doc_number; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_quotations_doc_number ON public.quotations USING btree (doc_number);


--
-- Name: idx_quotations_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_quotations_status ON public.quotations USING btree (status);


--
-- Name: idx_receipts_batch; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_receipts_batch ON public.production_receipts USING btree (batch_id);


--
-- Name: idx_receipts_work_order; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_receipts_work_order ON public.production_receipts USING btree (work_order_id);


--
-- Name: idx_reconciliation_items_rec; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_reconciliation_items_rec ON public.reconciliation_items USING btree (reconciliation_id);


--
-- Name: idx_reconciliations_customer; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_reconciliations_customer ON public.reconciliations USING btree (customer_id);


--
-- Name: idx_reconciliations_doc_number; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_reconciliations_doc_number ON public.reconciliations USING btree (doc_number);


--
-- Name: idx_reconciliations_period; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_reconciliations_period ON public.reconciliations USING btree (period);


--
-- Name: idx_reconciliations_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_reconciliations_status ON public.reconciliations USING btree (status);


--
-- Name: idx_req_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_req_status ON public.material_requisitions USING btree (status) WHERE (deleted_at IS NULL);


--
-- Name: idx_req_wo; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_req_wo ON public.material_requisitions USING btree (work_order_id) WHERE (deleted_at IS NULL);


--
-- Name: idx_rmas_customer; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_rmas_customer ON public.rmas USING btree (customer_id) WHERE (deleted_at IS NULL);


--
-- Name: idx_rmas_doc_number; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_rmas_doc_number ON public.rmas USING btree (doc_number) WHERE (deleted_at IS NULL);


--
-- Name: idx_rmas_inspection_result; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_rmas_inspection_result ON public.rmas USING btree (linked_inspection_result_id) WHERE (deleted_at IS NULL);


--
-- Name: idx_rmas_product; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_rmas_product ON public.rmas USING btree (product_id) WHERE (deleted_at IS NULL);


--
-- Name: idx_rmas_severity; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_rmas_severity ON public.rmas USING btree (severity) WHERE (deleted_at IS NULL);


--
-- Name: idx_rmas_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_rmas_status ON public.rmas USING btree (status) WHERE (deleted_at IS NULL);


--
-- Name: idx_routing_steps_routing_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_steps_routing_id ON public.routing_steps USING btree (routing_id);


--
-- Name: idx_routing_steps_routing_process; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_steps_routing_process ON public.routing_steps USING btree (routing_id, process_code);


--
-- Name: idx_routings_name_trgm; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routings_name_trgm ON public.routings USING gin (name public.gin_trgm_ops);


--
-- Name: idx_routings_work_order; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routings_work_order ON public.work_order_routings USING btree (work_order_id);


--
-- Name: idx_sales_order_items_order; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_sales_order_items_order ON public.sales_order_items USING btree (order_id);


--
-- Name: idx_sales_orders_customer; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_sales_orders_customer ON public.sales_orders USING btree (customer_id);


--
-- Name: idx_sales_orders_doc_number; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_sales_orders_doc_number ON public.sales_orders USING btree (doc_number);


--
-- Name: idx_sales_orders_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_sales_orders_status ON public.sales_orders USING btree (status);


--
-- Name: idx_sales_return_items_return; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_sales_return_items_return ON public.sales_return_items USING btree (return_id);


--
-- Name: idx_sales_returns_customer; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_sales_returns_customer ON public.sales_returns USING btree (customer_id);


--
-- Name: idx_sales_returns_doc_number; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_sales_returns_doc_number ON public.sales_returns USING btree (doc_number);


--
-- Name: idx_sales_returns_order; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_sales_returns_order ON public.sales_returns USING btree (order_id);


--
-- Name: idx_sales_returns_shipping; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_sales_returns_shipping ON public.sales_returns USING btree (shipping_request_id);


--
-- Name: idx_sales_returns_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_sales_returns_status ON public.sales_returns USING btree (status);


--
-- Name: idx_scheduled_task_defs_name; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_scheduled_task_defs_name ON public.scheduled_task_defs USING btree (name);


--
-- Name: idx_shipping_request_items_sr; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_shipping_request_items_sr ON public.shipping_request_items USING btree (shipping_request_id);


--
-- Name: idx_shipping_requests_customer; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_shipping_requests_customer ON public.shipping_requests USING btree (customer_id);


--
-- Name: idx_shipping_requests_doc_number; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_shipping_requests_doc_number ON public.shipping_requests USING btree (doc_number);


--
-- Name: idx_shipping_requests_order; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_shipping_requests_order ON public.shipping_requests USING btree (order_id);


--
-- Name: idx_shipping_requests_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_shipping_requests_status ON public.shipping_requests USING btree (status);


--
-- Name: idx_state_logs_entity; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_state_logs_entity ON public.entity_state_logs USING btree (entity_type, entity_id, created_at DESC);


--
-- Name: idx_state_trans; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_state_trans ON public.state_transition_defs USING btree (entity_type, from_state);


--
-- Name: idx_stock_ledger_unique; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_stock_ledger_unique ON public.stock_ledger USING btree (product_id, warehouse_id, zone_id, bin_id, COALESCE(batch_no, ''::character varying));


--
-- Name: idx_stock_product; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_stock_product ON public.stock_ledger USING btree (product_id);


--
-- Name: idx_stock_warehouse; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_stock_warehouse ON public.stock_ledger USING btree (warehouse_id);


--
-- Name: idx_supplier_bank_accounts_supplier_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_supplier_bank_accounts_supplier_id ON public.supplier_bank_accounts USING btree (supplier_id);


--
-- Name: idx_supplier_contacts_supplier_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_supplier_contacts_supplier_id ON public.supplier_contacts USING btree (supplier_id);


--
-- Name: idx_suppliers_category; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_suppliers_category ON public.suppliers USING btree (category) WHERE (deleted_at IS NULL);


--
-- Name: idx_suppliers_name_trgm; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_suppliers_name_trgm ON public.suppliers USING gin (supplier_name public.gin_trgm_ops);


--
-- Name: idx_suppliers_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_suppliers_status ON public.suppliers USING btree (status) WHERE (deleted_at IS NULL);


--
-- Name: idx_suppliers_supplier_code; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_suppliers_supplier_code ON public.suppliers USING btree (supplier_code) WHERE (deleted_at IS NULL);


--
-- Name: idx_task_run_logs_started_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_task_run_logs_started_at ON public.task_run_logs USING btree (started_at DESC);


--
-- Name: idx_task_run_logs_task_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_task_run_logs_task_id ON public.task_run_logs USING btree (task_id);


--
-- Name: idx_ti_transfer; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_ti_transfer ON public.transfer_items USING btree (transfer_id);


--
-- Name: idx_txn_created; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_txn_created ON public.inventory_transactions USING btree (created_at);


--
-- Name: idx_txn_product; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_txn_product ON public.inventory_transactions USING btree (product_id);


--
-- Name: idx_txn_source; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_txn_source ON public.inventory_transactions USING btree (source_type, source_id);


--
-- Name: idx_txn_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_txn_type ON public.inventory_transactions USING btree (transaction_type);


--
-- Name: idx_warehouses_active; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_warehouses_active ON public.warehouses USING btree (id) WHERE (deleted_at IS NULL);


--
-- Name: idx_warehouses_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_warehouses_type ON public.warehouses USING btree (warehouse_type) WHERE (deleted_at IS NULL);


--
-- Name: idx_wo_journal; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_wo_journal ON public.write_offs USING btree (cash_journal_id);


--
-- Name: idx_wo_source; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_wo_source ON public.write_offs USING btree (source_type, source_id);


--
-- Name: idx_work_orders_doc_active; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_work_orders_doc_active ON public.work_orders USING btree (doc_number) WHERE (deleted_at IS NULL);


--
-- Name: idx_work_orders_plan_item; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_work_orders_plan_item ON public.work_orders USING btree (plan_item_id) WHERE (deleted_at IS NULL);


--
-- Name: idx_work_orders_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_work_orders_status ON public.work_orders USING btree (status) WHERE (deleted_at IS NULL);


--
-- Name: idx_work_reports_batch; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_work_reports_batch ON public.work_reports USING btree (batch_id);


--
-- Name: idx_work_reports_work_order; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_work_reports_work_order ON public.work_reports USING btree (work_order_id);


--
-- Name: idx_work_reports_worker; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_work_reports_worker ON public.work_reports USING btree (worker_id, report_date);


--
-- Name: idx_workflow_history_instance_time; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_workflow_history_instance_time ON public.workflow_history USING btree (instance_id, created_at);


--
-- Name: idx_workflow_instances_entity; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_workflow_instances_entity ON public.workflow_instances USING btree (entity_type, entity_id, status);


--
-- Name: idx_workflow_tasks_assignee_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_workflow_tasks_assignee_status ON public.workflow_tasks USING btree (assignee_id, status, due_at);


--
-- Name: idx_workflow_tasks_instance_node; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_workflow_tasks_instance_node ON public.workflow_tasks USING btree (instance_id, node_id, status);


--
-- Name: idx_workflow_tasks_pending_due; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_workflow_tasks_pending_due ON public.workflow_tasks USING btree (status, due_at) WHERE ((status)::text = 'pending'::text);


--
-- Name: idx_workflow_templates_entity_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_workflow_templates_entity_status ON public.workflow_templates USING btree (entity_type, status) WHERE (deleted_at IS NULL);


--
-- Name: idx_workflow_templates_trigger; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_workflow_templates_trigger ON public.workflow_templates USING btree (trigger_event) WHERE (((status)::text = 'active'::text) AND (deleted_at IS NULL));


--
-- Name: idx_zones_unique_active; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_zones_unique_active ON public.zones USING btree (warehouse_id, code) WHERE (deleted_at IS NULL);


--
-- Name: uk_wo_idempotency; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX uk_wo_idempotency ON public.write_offs USING btree (idempotency_key) WHERE (idempotency_key IS NOT NULL);


--
-- Name: audit_logs_2026_01_created_at_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_created_at ATTACH PARTITION public.audit_logs_2026_01_created_at_idx;


--
-- Name: audit_logs_2026_01_entity_type_entity_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_entity ATTACH PARTITION public.audit_logs_2026_01_entity_type_entity_id_idx;


--
-- Name: audit_logs_2026_01_operator_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_operator ATTACH PARTITION public.audit_logs_2026_01_operator_id_idx;


--
-- Name: audit_logs_2026_01_pkey; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.audit_logs_pkey ATTACH PARTITION public.audit_logs_2026_01_pkey;


--
-- Name: audit_logs_2026_02_created_at_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_created_at ATTACH PARTITION public.audit_logs_2026_02_created_at_idx;


--
-- Name: audit_logs_2026_02_entity_type_entity_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_entity ATTACH PARTITION public.audit_logs_2026_02_entity_type_entity_id_idx;


--
-- Name: audit_logs_2026_02_operator_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_operator ATTACH PARTITION public.audit_logs_2026_02_operator_id_idx;


--
-- Name: audit_logs_2026_02_pkey; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.audit_logs_pkey ATTACH PARTITION public.audit_logs_2026_02_pkey;


--
-- Name: audit_logs_2026_03_created_at_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_created_at ATTACH PARTITION public.audit_logs_2026_03_created_at_idx;


--
-- Name: audit_logs_2026_03_entity_type_entity_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_entity ATTACH PARTITION public.audit_logs_2026_03_entity_type_entity_id_idx;


--
-- Name: audit_logs_2026_03_operator_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_operator ATTACH PARTITION public.audit_logs_2026_03_operator_id_idx;


--
-- Name: audit_logs_2026_03_pkey; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.audit_logs_pkey ATTACH PARTITION public.audit_logs_2026_03_pkey;


--
-- Name: audit_logs_2026_04_created_at_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_created_at ATTACH PARTITION public.audit_logs_2026_04_created_at_idx;


--
-- Name: audit_logs_2026_04_entity_type_entity_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_entity ATTACH PARTITION public.audit_logs_2026_04_entity_type_entity_id_idx;


--
-- Name: audit_logs_2026_04_operator_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_operator ATTACH PARTITION public.audit_logs_2026_04_operator_id_idx;


--
-- Name: audit_logs_2026_04_pkey; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.audit_logs_pkey ATTACH PARTITION public.audit_logs_2026_04_pkey;


--
-- Name: audit_logs_2026_05_created_at_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_created_at ATTACH PARTITION public.audit_logs_2026_05_created_at_idx;


--
-- Name: audit_logs_2026_05_entity_type_entity_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_entity ATTACH PARTITION public.audit_logs_2026_05_entity_type_entity_id_idx;


--
-- Name: audit_logs_2026_05_operator_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_operator ATTACH PARTITION public.audit_logs_2026_05_operator_id_idx;


--
-- Name: audit_logs_2026_05_pkey; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.audit_logs_pkey ATTACH PARTITION public.audit_logs_2026_05_pkey;


--
-- Name: audit_logs_2026_06_created_at_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_created_at ATTACH PARTITION public.audit_logs_2026_06_created_at_idx;


--
-- Name: audit_logs_2026_06_entity_type_entity_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_entity ATTACH PARTITION public.audit_logs_2026_06_entity_type_entity_id_idx;


--
-- Name: audit_logs_2026_06_operator_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_operator ATTACH PARTITION public.audit_logs_2026_06_operator_id_idx;


--
-- Name: audit_logs_2026_06_pkey; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.audit_logs_pkey ATTACH PARTITION public.audit_logs_2026_06_pkey;


--
-- Name: audit_logs_2026_07_created_at_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_created_at ATTACH PARTITION public.audit_logs_2026_07_created_at_idx;


--
-- Name: audit_logs_2026_07_entity_type_entity_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_entity ATTACH PARTITION public.audit_logs_2026_07_entity_type_entity_id_idx;


--
-- Name: audit_logs_2026_07_operator_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_operator ATTACH PARTITION public.audit_logs_2026_07_operator_id_idx;


--
-- Name: audit_logs_2026_07_pkey; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.audit_logs_pkey ATTACH PARTITION public.audit_logs_2026_07_pkey;


--
-- Name: audit_logs_2026_08_created_at_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_created_at ATTACH PARTITION public.audit_logs_2026_08_created_at_idx;


--
-- Name: audit_logs_2026_08_entity_type_entity_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_entity ATTACH PARTITION public.audit_logs_2026_08_entity_type_entity_id_idx;


--
-- Name: audit_logs_2026_08_operator_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_operator ATTACH PARTITION public.audit_logs_2026_08_operator_id_idx;


--
-- Name: audit_logs_2026_08_pkey; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.audit_logs_pkey ATTACH PARTITION public.audit_logs_2026_08_pkey;


--
-- Name: audit_logs_2026_09_created_at_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_created_at ATTACH PARTITION public.audit_logs_2026_09_created_at_idx;


--
-- Name: audit_logs_2026_09_entity_type_entity_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_entity ATTACH PARTITION public.audit_logs_2026_09_entity_type_entity_id_idx;


--
-- Name: audit_logs_2026_09_operator_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_operator ATTACH PARTITION public.audit_logs_2026_09_operator_id_idx;


--
-- Name: audit_logs_2026_09_pkey; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.audit_logs_pkey ATTACH PARTITION public.audit_logs_2026_09_pkey;


--
-- Name: audit_logs_2026_10_created_at_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_created_at ATTACH PARTITION public.audit_logs_2026_10_created_at_idx;


--
-- Name: audit_logs_2026_10_entity_type_entity_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_entity ATTACH PARTITION public.audit_logs_2026_10_entity_type_entity_id_idx;


--
-- Name: audit_logs_2026_10_operator_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_operator ATTACH PARTITION public.audit_logs_2026_10_operator_id_idx;


--
-- Name: audit_logs_2026_10_pkey; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.audit_logs_pkey ATTACH PARTITION public.audit_logs_2026_10_pkey;


--
-- Name: audit_logs_2026_11_created_at_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_created_at ATTACH PARTITION public.audit_logs_2026_11_created_at_idx;


--
-- Name: audit_logs_2026_11_entity_type_entity_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_entity ATTACH PARTITION public.audit_logs_2026_11_entity_type_entity_id_idx;


--
-- Name: audit_logs_2026_11_operator_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_operator ATTACH PARTITION public.audit_logs_2026_11_operator_id_idx;


--
-- Name: audit_logs_2026_11_pkey; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.audit_logs_pkey ATTACH PARTITION public.audit_logs_2026_11_pkey;


--
-- Name: audit_logs_2026_12_created_at_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_created_at ATTACH PARTITION public.audit_logs_2026_12_created_at_idx;


--
-- Name: audit_logs_2026_12_entity_type_entity_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_entity ATTACH PARTITION public.audit_logs_2026_12_entity_type_entity_id_idx;


--
-- Name: audit_logs_2026_12_operator_id_idx; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.idx_audit_logs_operator ATTACH PARTITION public.audit_logs_2026_12_operator_id_idx;


--
-- Name: audit_logs_2026_12_pkey; Type: INDEX ATTACH; Schema: public; Owner: -
--

ALTER INDEX public.audit_logs_pkey ATTACH PARTITION public.audit_logs_2026_12_pkey;


--
-- Name: arrival_notice_items arrival_notice_items_notice_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.arrival_notice_items
    ADD CONSTRAINT arrival_notice_items_notice_id_fkey FOREIGN KEY (notice_id) REFERENCES public.arrival_notices(id);


--
-- Name: arrival_notices arrival_notices_warehouse_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.arrival_notices
    ADD CONSTRAINT arrival_notices_warehouse_id_fkey FOREIGN KEY (warehouse_id) REFERENCES public.warehouses(id);


--
-- Name: arrival_notices arrival_notices_zone_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.arrival_notices
    ADD CONSTRAINT arrival_notices_zone_id_fkey FOREIGN KEY (zone_id) REFERENCES public.zones(id);


--
-- Name: backflush_items backflush_items_record_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.backflush_items
    ADD CONSTRAINT backflush_items_record_id_fkey FOREIGN KEY (record_id) REFERENCES public.backflush_records(id);


--
-- Name: bins bins_zone_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.bins
    ADD CONSTRAINT bins_zone_id_fkey FOREIGN KEY (zone_id) REFERENCES public.zones(id);


--
-- Name: conversion_items conversion_items_conversion_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.conversion_items
    ADD CONSTRAINT conversion_items_conversion_id_fkey FOREIGN KEY (conversion_id) REFERENCES public.form_conversions(id);


--
-- Name: cycle_count_items cycle_count_items_count_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cycle_count_items
    ADD CONSTRAINT cycle_count_items_count_id_fkey FOREIGN KEY (count_id) REFERENCES public.cycle_counts(id);


--
-- Name: cycle_counts cycle_counts_warehouse_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cycle_counts
    ADD CONSTRAINT cycle_counts_warehouse_id_fkey FOREIGN KEY (warehouse_id) REFERENCES public.warehouses(id);


--
-- Name: cycle_counts cycle_counts_zone_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cycle_counts
    ADD CONSTRAINT cycle_counts_zone_id_fkey FOREIGN KEY (zone_id) REFERENCES public.zones(id);


--
-- Name: form_conversions form_conversions_warehouse_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.form_conversions
    ADD CONSTRAINT form_conversions_warehouse_id_fkey FOREIGN KEY (warehouse_id) REFERENCES public.warehouses(id);


--
-- Name: inventory_locks inventory_locks_warehouse_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.inventory_locks
    ADD CONSTRAINT inventory_locks_warehouse_id_fkey FOREIGN KEY (warehouse_id) REFERENCES public.warehouses(id);


--
-- Name: inventory_transfers inventory_transfers_from_bin_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.inventory_transfers
    ADD CONSTRAINT inventory_transfers_from_bin_id_fkey FOREIGN KEY (from_bin_id) REFERENCES public.bins(id);


--
-- Name: inventory_transfers inventory_transfers_from_warehouse_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.inventory_transfers
    ADD CONSTRAINT inventory_transfers_from_warehouse_id_fkey FOREIGN KEY (from_warehouse_id) REFERENCES public.warehouses(id);


--
-- Name: inventory_transfers inventory_transfers_from_zone_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.inventory_transfers
    ADD CONSTRAINT inventory_transfers_from_zone_id_fkey FOREIGN KEY (from_zone_id) REFERENCES public.zones(id);


--
-- Name: inventory_transfers inventory_transfers_to_bin_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.inventory_transfers
    ADD CONSTRAINT inventory_transfers_to_bin_id_fkey FOREIGN KEY (to_bin_id) REFERENCES public.bins(id);


--
-- Name: inventory_transfers inventory_transfers_to_warehouse_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.inventory_transfers
    ADD CONSTRAINT inventory_transfers_to_warehouse_id_fkey FOREIGN KEY (to_warehouse_id) REFERENCES public.warehouses(id);


--
-- Name: inventory_transfers inventory_transfers_to_zone_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.inventory_transfers
    ADD CONSTRAINT inventory_transfers_to_zone_id_fkey FOREIGN KEY (to_zone_id) REFERENCES public.zones(id);


--
-- Name: material_requisition_items material_requisition_items_requisition_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.material_requisition_items
    ADD CONSTRAINT material_requisition_items_requisition_id_fkey FOREIGN KEY (requisition_id) REFERENCES public.material_requisitions(id);


--
-- Name: material_requisitions material_requisitions_warehouse_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.material_requisitions
    ADD CONSTRAINT material_requisitions_warehouse_id_fkey FOREIGN KEY (warehouse_id) REFERENCES public.warehouses(id);


--
-- Name: outsourcing_materials outsourcing_materials_outsourcing_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.outsourcing_materials
    ADD CONSTRAINT outsourcing_materials_outsourcing_id_fkey FOREIGN KEY (outsourcing_id) REFERENCES public.outsourcing_orders(id);


--
-- Name: outsourcing_trackings outsourcing_trackings_outsourcing_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.outsourcing_trackings
    ADD CONSTRAINT outsourcing_trackings_outsourcing_id_fkey FOREIGN KEY (outsourcing_id) REFERENCES public.outsourcing_orders(id);


--
-- Name: pick_strategies pick_strategies_warehouse_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.pick_strategies
    ADD CONSTRAINT pick_strategies_warehouse_id_fkey FOREIGN KEY (warehouse_id) REFERENCES public.warehouses(id);


--
-- Name: product_categories product_categories_category_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.product_categories
    ADD CONSTRAINT product_categories_category_id_fkey FOREIGN KEY (category_id) REFERENCES public.categories(category_id) ON DELETE CASCADE;


--
-- Name: production_batches production_batches_work_order_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.production_batches
    ADD CONSTRAINT production_batches_work_order_id_fkey FOREIGN KEY (work_order_id) REFERENCES public.work_orders(id);


--
-- Name: production_inspections production_inspections_routing_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.production_inspections
    ADD CONSTRAINT production_inspections_routing_id_fkey FOREIGN KEY (routing_id) REFERENCES public.work_order_routings(id);


--
-- Name: production_inspections production_inspections_work_order_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.production_inspections
    ADD CONSTRAINT production_inspections_work_order_id_fkey FOREIGN KEY (work_order_id) REFERENCES public.work_orders(id);


--
-- Name: production_plan_items production_plan_items_plan_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.production_plan_items
    ADD CONSTRAINT production_plan_items_plan_id_fkey FOREIGN KEY (plan_id) REFERENCES public.production_plans(id);


--
-- Name: production_receipts production_receipts_batch_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.production_receipts
    ADD CONSTRAINT production_receipts_batch_id_fkey FOREIGN KEY (batch_id) REFERENCES public.production_batches(id);


--
-- Name: production_receipts production_receipts_work_order_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.production_receipts
    ADD CONSTRAINT production_receipts_work_order_id_fkey FOREIGN KEY (work_order_id) REFERENCES public.work_orders(id);


--
-- Name: putaway_strategies putaway_strategies_warehouse_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.putaway_strategies
    ADD CONSTRAINT putaway_strategies_warehouse_id_fkey FOREIGN KEY (warehouse_id) REFERENCES public.warehouses(id);


--
-- Name: quotation_items quotation_items_quotation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.quotation_items
    ADD CONSTRAINT quotation_items_quotation_id_fkey FOREIGN KEY (quotation_id) REFERENCES public.quotations(id);


--
-- Name: reconciliation_items reconciliation_items_reconciliation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.reconciliation_items
    ADD CONSTRAINT reconciliation_items_reconciliation_id_fkey FOREIGN KEY (reconciliation_id) REFERENCES public.reconciliations(id);


--
-- Name: sales_order_items sales_order_items_order_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.sales_order_items
    ADD CONSTRAINT sales_order_items_order_id_fkey FOREIGN KEY (order_id) REFERENCES public.sales_orders(id);


--
-- Name: sales_return_items sales_return_items_return_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.sales_return_items
    ADD CONSTRAINT sales_return_items_return_id_fkey FOREIGN KEY (return_id) REFERENCES public.sales_returns(id);


--
-- Name: shipping_request_items shipping_request_items_shipping_request_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.shipping_request_items
    ADD CONSTRAINT shipping_request_items_shipping_request_id_fkey FOREIGN KEY (shipping_request_id) REFERENCES public.shipping_requests(id);


--
-- Name: transfer_items transfer_items_transfer_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transfer_items
    ADD CONSTRAINT transfer_items_transfer_id_fkey FOREIGN KEY (transfer_id) REFERENCES public.inventory_transfers(id);


--
-- Name: work_order_routings work_order_routings_work_order_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.work_order_routings
    ADD CONSTRAINT work_order_routings_work_order_id_fkey FOREIGN KEY (work_order_id) REFERENCES public.work_orders(id);


--
-- Name: work_orders work_orders_plan_item_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.work_orders
    ADD CONSTRAINT work_orders_plan_item_id_fkey FOREIGN KEY (plan_item_id) REFERENCES public.production_plan_items(id);


--
-- Name: work_reports work_reports_batch_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.work_reports
    ADD CONSTRAINT work_reports_batch_id_fkey FOREIGN KEY (batch_id) REFERENCES public.production_batches(id);


--
-- Name: work_reports work_reports_routing_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.work_reports
    ADD CONSTRAINT work_reports_routing_id_fkey FOREIGN KEY (routing_id) REFERENCES public.work_order_routings(id);


--
-- Name: work_reports work_reports_work_order_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.work_reports
    ADD CONSTRAINT work_reports_work_order_id_fkey FOREIGN KEY (work_order_id) REFERENCES public.work_orders(id);


--
-- Name: zones zones_warehouse_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.zones
    ADD CONSTRAINT zones_warehouse_id_fkey FOREIGN KEY (warehouse_id) REFERENCES public.warehouses(id);


--
-- PostgreSQL database dump complete
--

\unrestrict 9s9pLQpMwGWyp7iXO27d9I5rbfJOg5lRumDqytaEOkcGCgbgzPh6CRCDiS6xe8o

