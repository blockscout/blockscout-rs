// SPDX-License-Identifier: LicenseRef-Blockscout

//! The interchain read filter's coverage guard.
//!
//! This module exists so that "every interchain chart applies the shared
//! predicate" is a property a test checks rather than a claim a reviewer
//! re-verifies by reading thirteen files. Before the filter was introduced, four
//! of the fifteen interchain chart families silently ignored filtering
//! altogether — precisely because coverage was procedural.
//!
//! It lives under `charts` rather than next to the filter so that it can see
//! both [`crate::counters`] and [`crate::lines`].
//!
//! ## What the two layers actually guarantee
//!
//! - **Layer 1** proves that every statement *in the registry* renders the
//!   shared predicate, byte for byte, exactly as many times as it declares.
//! - **Layer 2** proves that every interchain chart id in
//!   `config/interchain/charts.json` is either in the registry or explicitly
//!   listed as deriving its data from another interchain chart. Since a chart
//!   that is not in `charts.json` cannot be enabled at all, a *new* interchain
//!   chart cannot reach production without passing through here.
//!
//! ## What they do not guarantee
//!
//! Full structural enforcement is not possible inside one crate.
//! `interchain-indexer-entity` is an ordinary `stats` dependency, so any module
//! could call `crosschain_messages::Entity::find()` directly and bypass
//! [`InterchainFilter`]'s entry points. The entry points make the right thing
//! the easy thing; these two tests are the actual guard; and neither can stop a
//! statement that is deliberately written to evade both. A registered statement
//! that renders the predicate and then ORs it away would also pass layer 1 —
//! the count is a coverage check, not a semantic proof. The per-chart
//! `statement_is_correct` snapshots and the DB-backed expectations are what
//! cover meaning.

#![cfg(test)]

use interchain_indexer_entity::{crosschain_messages, crosschain_transfers};
use interchain_indexer_filters::ChainBridgeFilter;
use sea_orm::{DbBackend, EntityTrait, QueryFilter, QueryTrait, Statement, sea_query::Expr};

use crate::{
    charts::db_interaction::filters::interchain::{
        InterchainFilter, InterchainFilterConfig, InterchainFilterTarget, InterchainFiltered,
    },
    counters::interchain::{
        TotalInterchainMessagesReceivedStatement, TotalInterchainMessagesSentStatement,
        TotalInterchainMessagesStatement, TotalInterchainTransferUsersStatement,
        TotalInterchainTransfersReceivedStatement, TotalInterchainTransfersSentStatement,
        TotalInterchainTransfersStatement,
    },
    lines::interchain::{
        new_messages_interchain::NewMessagesInterchainStatement,
        new_messages_received_interchain::NewMessagesReceivedInterchainStatement,
        new_messages_sent_interchain::NewMessagesSentInterchainStatement,
        new_transfers_interchain::NewTransfersInterchainStatement,
        new_transfers_received_interchain::NewTransfersReceivedInterchainStatement,
        new_transfers_sent_interchain::NewTransfersSentInterchainStatement,
    },
    tests::normalize_sql,
};

type RenderFn = fn(&InterchainFilter) -> Statement;

struct CoverageEntry {
    chart_name: &'static str,
    target: InterchainFilterTarget,
    expected_applications: usize,
    render: RenderFn,
}

macro_rules! coverage_entries {
    ($($ty:ty),+ $(,)?) => {
        vec![$(CoverageEntry {
            chart_name: <$ty as InterchainFiltered>::CHART_NAME,
            target: <$ty as InterchainFiltered>::TARGET,
            expected_applications: <$ty as InterchainFiltered>::EXPECTED_APPLICATIONS,
            render: <$ty as InterchainFiltered>::render as RenderFn,
        }),+]
    };
}

/// Every interchain statement that builds its own SQL.
fn registry() -> Vec<CoverageEntry> {
    coverage_entries![
        TotalInterchainMessagesStatement,
        TotalInterchainMessagesSentStatement,
        TotalInterchainMessagesReceivedStatement,
        TotalInterchainTransfersStatement,
        TotalInterchainTransfersSentStatement,
        TotalInterchainTransfersReceivedStatement,
        TotalInterchainTransferUsersStatement,
        NewMessagesInterchainStatement,
        NewMessagesSentInterchainStatement,
        NewMessagesReceivedInterchainStatement,
        NewTransfersInterchainStatement,
        NewTransfersSentInterchainStatement,
        NewTransfersReceivedInterchainStatement,
    ]
}

/// Charts with no SQL of their own: they read another interchain chart's stored
/// rows and inherit its filter transitively. Adding an id here is a claim that
/// must be true.
const DERIVED_WITHOUT_OWN_STATEMENT: &[&str] = &[
    // cumulative sums of `new_messages_sent_interchain` /
    // `new_messages_received_interchain`
    "messages_growth_sent_interchain",
    "messages_growth_received_interchain",
];

// Sentinel ids, chosen so that they cannot collide with anything a statement
// renders on its own.
const S_HOME: i64 = 9_100_000_000_001;
const S_CP: i64 = 9_100_000_000_002;
const S_SRC: i64 = 9_100_000_000_003;
const S_DST: i64 = 9_100_000_000_004;
const S_BRIDGE: i32 = 910_000_001;

/// A filter with all six dimensions populated, so the rendered predicate is
/// maximally distinctive and every branch of the shared condition is exercised.
fn sentinel_filter() -> InterchainFilter {
    InterchainFilterConfig::new(
        ChainBridgeFilter {
            home_chain_id: Some(S_HOME),
            counterparty_chain_ids: Some(vec![S_CP]),
            src_chain_ids: Some(vec![S_SRC]),
            dst_chain_ids: Some(vec![S_DST]),
            bridge_ids: Some(vec![S_BRIDGE]),
            only_indexed_by_bridge: None,
        },
        false,
    )
    .with_horizon(Some(vec![(S_BRIDGE, vec![S_HOME, S_CP])]))
}

/// The predicate as the shared crate renders it, derived at test time rather
/// than transcribed.
///
/// `Expr::value(true)` stands in for "the chart's own predicates", so the
/// fragment is rendered at the same nesting depth — and with the same
/// parenthesisation — that it has inside a real statement.
fn expected_fragment(target: InterchainFilterTarget, filter: &InterchainFilter) -> String {
    let built = match target {
        InterchainFilterTarget::Messages => crosschain_messages::Entity::find()
            .filter(Expr::value(true))
            .filter(filter.messages_condition())
            .build(DbBackend::Postgres),
        InterchainFilterTarget::Transfers => crosschain_transfers::Entity::find()
            .filter(Expr::value(true))
            .filter(filter.transfers_condition())
            .build(DbBackend::Postgres),
    };
    let sql = built.to_string();
    let where_clause = sql
        .split_once(" WHERE ")
        .expect("filtered query has a WHERE")
        .1;
    normalize_sql(
        where_clause
            .strip_prefix("TRUE AND ")
            .expect("stand-in predicate renders first"),
    )
}

/// Layer 1 — every registered statement applies its predicate exactly
/// `EXPECTED_APPLICATIONS` times. A forgotten filter gives `0`; a double-applied
/// one gives `2`.
///
/// This is stable because the sentinel condition has enough children that
/// sea-query always parenthesises it (making its rendering position-independent),
/// the `Expr::value(true)` sibling guarantees it is never the sole top-level
/// condition, and both sides come from the same renderer over the same entity.
#[test]
fn every_interchain_statement_applies_the_filter() {
    let filter = sentinel_filter();
    for entry in registry() {
        let fragment = expected_fragment(entry.target, &filter);
        let sql = normalize_sql(&(entry.render)(&filter).to_string());
        let applications = sql.matches(&fragment).count();
        assert_eq!(
            applications, entry.expected_applications,
            "{} applies the {:?} predicate {} time(s), expected {}.\n\
             Start the statement from InterchainFilter::{{messages_query, transfers_query, \
             transfers_joined_query}} exactly once per scanned table.\n\
             expected fragment: {fragment}\n\
             rendered: {sql}",
            entry.chart_name, entry.target, applications, entry.expected_applications
        );
    }
}

/// Layer 2 — the registry covers every interchain chart that can be enabled.
///
/// A new interchain chart must appear in `config/interchain/charts.json` to be
/// enabled at all, so adding one without registering it fails here.
#[test]
fn registry_covers_every_configured_interchain_chart() {
    let config: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../config/interchain/charts.json"
    )))
    .expect("config/interchain/charts.json is valid JSON");

    let registered: Vec<&'static str> = registry().iter().map(|e| e.chart_name).collect();
    let mut configured = Vec::new();
    for section in ["counters", "line_charts"] {
        let charts = config
            .get(section)
            .and_then(|s| s.as_object())
            .unwrap_or_else(|| panic!("charts.json has no `{section}` object"));
        configured.extend(charts.keys().cloned());
    }
    assert!(
        !configured.is_empty(),
        "charts.json parsed to no charts at all"
    );

    for config_key in configured {
        let chart_name = snake_to_camel(&config_key);
        assert!(
            registered.contains(&chart_name.as_str())
                || DERIVED_WITHOUT_OWN_STATEMENT.contains(&config_key.as_str()),
            "interchain chart `{config_key}` (chart id `{chart_name}`) is enabled by \
             config/interchain/charts.json but is not covered by the filter registry.\n\
             Either implement `InterchainFiltered` for its statement and add it to \
             `registry()` in this file, or — if it has no SQL of its own and reads another \
             interchain chart's stored rows — add it to `DERIVED_WITHOUT_OWN_STATEMENT`."
        );
    }
}

/// `charts.json` keys are snake_case; chart ids (`Named::name`) are camelCase.
fn snake_to_camel(key: &str) -> String {
    let mut out = String::with_capacity(key.len());
    let mut capitalize = false;
    for c in key.chars() {
        if c == '_' {
            capitalize = true;
        } else if capitalize {
            out.extend(c.to_uppercase());
            capitalize = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// The parity matrix — the shape properties `interchain-indexer-filters` asserts
/// about itself, re-asserted here through the dependency.
///
/// These duplicate the shared crate's own tests on purpose: they are stats'
/// statement that it still sees that behaviour, and they would fail loudly if a
/// future revision of the shared crate changed the predicate's shape under us.
mod parity {
    use pretty_assertions::assert_eq;

    use super::*;

    fn configured(
        home_chain_id: Option<i64>,
        counterparty_chain_ids: Option<Vec<i64>>,
        src_chain_ids: Option<Vec<i64>>,
        dst_chain_ids: Option<Vec<i64>>,
        bridge_ids: Option<Vec<i32>>,
        only_indexed_by_bridge: Option<Vec<(i32, Vec<i64>)>>,
    ) -> InterchainFilter {
        InterchainFilterConfig::new(
            ChainBridgeFilter {
                home_chain_id,
                counterparty_chain_ids,
                src_chain_ids,
                dst_chain_ids,
                bridge_ids,
                only_indexed_by_bridge: None,
            },
            only_indexed_by_bridge.is_none(),
        )
        .with_horizon(only_indexed_by_bridge)
    }

    fn messages_sql(filter: &InterchainFilter) -> String {
        normalize_sql(
            &filter
                .messages_query()
                .build(DbBackend::Postgres)
                .to_string(),
        )
    }

    fn transfers_sql(filter: &InterchainFilter) -> String {
        normalize_sql(
            &filter
                .transfers_query()
                .build(DbBackend::Postgres)
                .to_string(),
        )
    }

    /// The four focal cases, on both tables: the `OR` appears exactly when a
    /// focal dimension is set, and the within-set conjunction replaces it when
    /// only counterparties are given.
    #[test]
    fn focal_or_is_present_exactly_when_a_focal_dimension_is_set() {
        let cases = [
            // (home, counterparties, expects an OR)
            (None, None, false),
            (Some(1), None, true),
            (Some(1), Some(vec![2, 3]), true),
            (None, Some(vec![2, 3]), false),
        ];
        for (home, counterparties, expects_or) in cases {
            let filter = configured(home, counterparties.clone(), None, None, None, None);
            for (table, sql) in [
                ("messages", messages_sql(&filter)),
                ("transfers", transfers_sql(&filter)),
            ] {
                assert_eq!(
                    sql.contains(" OR "),
                    expects_or,
                    "{table}: home={home:?} counterparties={counterparties:?} rendered {sql}"
                );
            }
        }
    }

    /// Directional ids never enter the focal `OR`: they are separate terms of the
    /// outer `AND`, so `src` and `dst` restrictions compose rather than widen.
    #[test]
    fn directional_ids_are_separate_and_terms() {
        let filter = configured(
            Some(1),
            None,
            Some(vec![7]),
            Some(vec![8]),
            Some(vec![9]),
            None,
        );
        let sql = messages_sql(&filter);
        assert!(
            sql.contains(
                r#"AND "crosschain_messages"."src_chain_id" IN (7) AND "crosschain_messages"."dst_chain_id" IN (8) AND "crosschain_messages"."bridge_id" IN (9)"#
            ),
            "directional terms are not a flat AND tail: {sql}"
        );

        let sql = transfers_sql(&filter);
        assert!(
            sql.contains(
                r#"AND "crosschain_transfers"."token_src_chain_id" IN (7) AND "crosschain_transfers"."token_dst_chain_id" IN (8) AND "crosschain_transfers"."bridge_id" IN (9)"#
            ),
            "directional terms are not a flat AND tail: {sql}"
        );
    }

    /// A listed bridge with an empty chain set observes nothing, and `is_in([])`
    /// renders `1 = 2` — the disjunct that excludes all of its rows.
    #[test]
    fn a_listed_bridge_with_no_chains_renders_a_false_disjunct() {
        let filter = configured(None, None, None, None, None, Some(vec![(5, vec![])]));
        for (table, sql) in [
            ("messages", messages_sql(&filter)),
            ("transfers", transfers_sql(&filter)),
        ] {
            assert!(sql.contains("1 = 2"), "{table}: {sql}");
        }
    }

    /// The messages permissive arm carries an explicit `dst IS NOT NULL`: a
    /// record whose destination was never observed is not "fully indexed",
    /// whoever indexed it. The transfers arm carries no such guard, because both
    /// token chain columns are NOT NULL.
    #[test]
    fn only_the_messages_permissive_arm_guards_against_null() {
        let filter = configured(None, None, None, None, None, Some(vec![(5, vec![1, 2])]));
        assert!(
            messages_sql(&filter).contains(r#""crosschain_messages"."dst_chain_id" IS NOT NULL"#),
            "messages permissive arm lost its NULL guard: {}",
            messages_sql(&filter)
        );
        assert!(
            !transfers_sql(&filter).contains("IS NOT NULL"),
            "transfers arm grew a NULL guard: {}",
            transfers_sql(&filter)
        );
    }

    /// An empty horizon list restricts nothing (every bridge is "absent"), but it
    /// is not the same as `None`: for messages it still carries the NULL guard.
    #[test]
    fn an_empty_horizon_list_is_not_the_same_as_no_horizon() {
        let none = configured(None, None, None, None, None, None);
        let empty = configured(None, None, None, None, None, Some(vec![]));
        assert!(!messages_sql(&none).contains("IS NOT NULL"));
        assert!(messages_sql(&empty).contains(r#""dst_chain_id" IS NOT NULL"#));
        assert!(!messages_sql(&empty).contains("1 = 2"));
    }

    /// Transfer columns are always table-qualified. Six column names are shared
    /// between the two tables, so an unqualified reference on the joined query is
    /// an ambiguity at best.
    #[test]
    fn transfer_predicates_are_table_qualified() {
        let filter = configured(Some(1), Some(vec![2]), None, None, Some(vec![3]), None);
        let sql = normalize_sql(
            &filter
                .transfers_joined_query()
                .build(DbBackend::Postgres)
                .to_string(),
        );
        let where_clause = sql.split_once(" WHERE ").expect("filtered").1;
        for column in ["token_src_chain_id", "token_dst_chain_id", "bridge_id"] {
            assert!(
                where_clause.contains(&format!(r#""crosschain_transfers"."{column}""#)),
                "{column} is not qualified: {where_clause}"
            );
        }
    }
}
