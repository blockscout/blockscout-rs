// SPDX-License-Identifier: LicenseRef-Blockscout

use interchain_indexer_entity::{crosschain_messages, crosschain_transfers};
use sea_orm::{ColumnTrait, Condition, sea_query::Expr};

/// Optional read-time chain/bridge filter applied to list and counter queries.
///
/// Invariants: `counterparty_chain_ids` / `src_chain_ids` / `dst_chain_ids` /
/// `bridge_ids` are `Some` only when non-empty.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChainBridgeFilter {
    pub home_chain_id: Option<i64>,
    pub counterparty_chain_ids: Option<Vec<i64>>,
    pub src_chain_ids: Option<Vec<i64>>,
    pub dst_chain_ids: Option<Vec<i64>>,
    pub bridge_ids: Option<Vec<i32>>,
    /// When `Some`, restricts results to rows that their own bridge could have
    /// fully observed: either the bridge is **not** in this list at all (removed
    /// from config — its already-indexed history must not be reinterpreted, so it
    /// stays visible), or it is listed and *both* endpoints are in its indexed set
    /// (src/dst for messages, token_src/token_dst for transfers). Sorted by bridge
    /// id then chain id.
    ///
    /// `None` = no restriction (`include_unindexed_chains=true`, or an
    /// `AllIndexed` configuration). `Some(empty)` = no bridge is configured, which
    /// restricts nothing either (every bridge is then "absent"); it is defensive
    /// only, since the startup guard rejects that config. A bridge listed with an
    /// **empty** chain set is the opposite case: it observes nothing, so all its
    /// rows are excluded.
    pub only_indexed_by_bridge: Option<Vec<(i32, Vec<i64>)>>,
}

impl ChainBridgeFilter {
    pub fn is_empty(&self) -> bool {
        self.home_chain_id.is_none()
            && self.counterparty_chain_ids.is_none()
            && self.src_chain_ids.is_none()
            && self.dst_chain_ids.is_none()
            && self.bridge_ids.is_none()
            && self.only_indexed_by_bridge.is_none()
    }

    /// Predicate over `crosschain_messages` src/dst/bridge columns.
    pub fn messages_condition(&self) -> Condition {
        let src = crosschain_messages::Column::SrcChainId;
        let dst = crosschain_messages::Column::DstChainId;
        let bridge = crosschain_messages::Column::BridgeId;

        let chain = match (self.home_chain_id, self.counterparty_chain_ids.as_deref()) {
            (Some(n), Some(s)) => Condition::any()
                .add(Condition::all().add(src.eq(n)).add(dst.is_in(s.to_vec())))
                .add(Condition::all().add(dst.eq(n)).add(src.is_in(s.to_vec()))),
            (Some(n), None) => Condition::any().add(src.eq(n)).add(dst.eq(n)),
            (None, Some(s)) => Condition::all()
                .add(src.is_in(s.to_vec()))
                .add(dst.is_in(s.to_vec())),
            (None, None) => Condition::all(),
        };

        // Directional predicates refine the focal view; they are appended to the
        // outer AND and must never be inserted inside the focal `OR` above.
        let mut cond = Condition::all().add(chain);
        if let Some(s) = self.src_chain_ids.as_deref() {
            cond = cond.add(src.is_in(s.to_vec()));
        }
        if let Some(d) = self.dst_chain_ids.as_deref() {
            cond = cond.add(dst.is_in(d.to_vec()));
        }
        if let Some(b) = self.bridge_ids.as_deref() {
            cond = cond.add(bridge.is_in(b.to_vec()));
        }
        if let Some(pairs) = self.only_indexed_by_bridge.as_deref() {
            // One nested OR appended to the outer AND. It must never be spliced into
            // the focal `OR` above, or SQL precedence would admit rows from bridges
            // the caller did not select — and with the permissive arm below, a
            // flattened version would satisfy the focal chain predicate all by
            // itself for any bridge missing from the list.
            let listed: Vec<i32> = pairs.iter().map(|(b, _)| *b).collect();
            // Permissive arm: a bridge that is not in the current config tells us
            // nothing about what was observable back when it was. Its rows stay
            // visible, so deleting a bridge from `bridges.json` never changes what
            // this endpoint returns. A NULL destination is still excluded: that
            // record was never fully observed, whoever indexed it. See ADR-004
            // Decision 5 — this arm is not a leak, it is the decision.
            let mut permissive = Condition::all();
            permissive = if listed.is_empty() {
                // No bridge configured at all ⇒ every bridge is "absent". Spell the
                // literal out; an empty `Condition::all()` renders to nothing.
                permissive.add(Expr::value(true))
            } else {
                permissive.add(bridge.is_not_in(listed.clone()))
            };
            let mut indexed = Condition::any().add(permissive.add(dst.is_not_null()));
            for (bridge_id, chains) in pairs {
                indexed = indexed.add(
                    Condition::all()
                        .add(bridge.eq(*bridge_id))
                        .add(src.is_in(chains.clone()))
                        // NULL dst yields NULL here, so the row is excluded — the
                        // required "dst NULL counts as non-indexed" behavior. No
                        // explicit IS NULL / IS NOT NULL term is needed *inside* a
                        // disjunct; the permissive arm above is the one place that
                        // needs it spelled out.
                        .add(dst.is_in(chains.clone())),
                );
            }
            cond = cond.add(indexed);
        }
        cond
    }

    /// Predicate over transfer token chain columns and qualified transfer `bridge_id`.
    pub fn transfers_condition(&self) -> Condition {
        let src = (
            crosschain_transfers::Entity,
            crosschain_transfers::Column::TokenSrcChainId,
        );
        let dst = (
            crosschain_transfers::Entity,
            crosschain_transfers::Column::TokenDstChainId,
        );
        let bridge = (
            crosschain_transfers::Entity,
            crosschain_transfers::Column::BridgeId,
        );

        let chain = match (self.home_chain_id, self.counterparty_chain_ids.as_deref()) {
            (Some(n), Some(s)) => Condition::any()
                .add(
                    Condition::all()
                        .add(Expr::col(src).eq(n))
                        .add(Expr::col(dst).is_in(s.to_vec())),
                )
                .add(
                    Condition::all()
                        .add(Expr::col(dst).eq(n))
                        .add(Expr::col(src).is_in(s.to_vec())),
                ),
            (Some(n), None) => Condition::any()
                .add(Expr::col(src).eq(n))
                .add(Expr::col(dst).eq(n)),
            (None, Some(s)) => Condition::all()
                .add(Expr::col(src).is_in(s.to_vec()))
                .add(Expr::col(dst).is_in(s.to_vec())),
            (None, None) => Condition::all(),
        };

        // Directional predicates refine the focal view; they are appended to the
        // outer AND and must never be inserted inside the focal `OR` above.
        let mut cond = Condition::all().add(chain);
        if let Some(s) = self.src_chain_ids.as_deref() {
            cond = cond.add(Expr::col(src).is_in(s.to_vec()));
        }
        if let Some(d) = self.dst_chain_ids.as_deref() {
            cond = cond.add(Expr::col(dst).is_in(d.to_vec()));
        }
        if let Some(b) = self.bridge_ids.as_deref() {
            cond = cond.add(Expr::col(bridge).is_in(b.to_vec()));
        }
        if let Some(pairs) = self.only_indexed_by_bridge.as_deref() {
            // Same shape as `messages_condition`, but both token chain columns are
            // NOT NULL, so the permissive arm carries no NULL guard.
            let listed: Vec<i32> = pairs.iter().map(|(b, _)| *b).collect();
            let permissive = if listed.is_empty() {
                Condition::all().add(Expr::value(true))
            } else {
                Condition::all().add(Expr::col(bridge).is_not_in(listed.clone()))
            };
            let mut indexed = Condition::any().add(permissive);
            for (bridge_id, chains) in pairs {
                indexed = indexed.add(
                    Condition::all()
                        .add(Expr::col(bridge).eq(*bridge_id))
                        .add(Expr::col(src).is_in(chains.clone()))
                        .add(Expr::col(dst).is_in(chains.clone())),
                );
            }
            cond = cond.add(indexed);
        }
        cond
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DatabaseBackend, EntityTrait, QueryFilter, QueryTrait};

    fn sql_messages(filter: &ChainBridgeFilter) -> String {
        crosschain_messages::Entity::find()
            .filter(filter.messages_condition())
            .build(DatabaseBackend::Postgres)
            .to_string()
    }

    fn sql_transfers(filter: &ChainBridgeFilter) -> String {
        crosschain_transfers::Entity::find()
            .filter(filter.transfers_condition())
            .build(DatabaseBackend::Postgres)
            .to_string()
    }

    #[test]
    fn test_messages_condition_empty_filter_is_noop() {
        let filter = ChainBridgeFilter::default();
        assert!(filter.is_empty());
        let sql = sql_messages(&filter);
        // SeaORM emits `WHERE TRUE` for an empty Condition::all(); that is fine.
        // Restrictive chain/bridge predicates must not appear in the WHERE clause.
        let where_sql = sql
            .split_once(" WHERE ")
            .map(|(_, w)| w.to_ascii_lowercase())
            .unwrap_or_default();
        assert!(
            !where_sql.contains("src_chain_id")
                && !where_sql.contains("dst_chain_id")
                && !where_sql.contains("bridge_id"),
            "empty filter must not add chain/bridge predicates; got: {sql}"
        );
    }

    #[test]
    fn test_messages_condition_home_only() {
        let filter = ChainBridgeFilter {
            home_chain_id: Some(100),
            ..Default::default()
        };
        let sql = sql_messages(&filter);
        assert!(sql.contains("src_chain_id") && sql.contains("dst_chain_id"));
        assert!(sql.contains("100"));
    }

    #[test]
    fn test_messages_condition_home_and_counterparties() {
        let filter = ChainBridgeFilter {
            home_chain_id: Some(1),
            counterparty_chain_ids: Some(vec![100, 250]),
            ..Default::default()
        };
        let sql = sql_messages(&filter);
        assert!(sql.contains("100"));
        assert!(sql.contains("250"));
    }

    #[test]
    fn test_messages_condition_counterparties_only() {
        let filter = ChainBridgeFilter {
            counterparty_chain_ids: Some(vec![1, 100]),
            ..Default::default()
        };
        let sql = sql_messages(&filter);
        assert!(sql.contains("1"));
        assert!(sql.contains("100"));
    }

    #[test]
    fn test_messages_condition_bridge_only() {
        let filter = ChainBridgeFilter {
            bridge_ids: Some(vec![1, 2]),
            ..Default::default()
        };
        let sql = sql_messages(&filter);
        assert!(sql.contains("bridge_id"));
        assert!(sql.contains("1"));
        assert!(sql.contains("2"));
    }

    #[test]
    fn test_messages_condition_full_triple() {
        let filter = ChainBridgeFilter {
            home_chain_id: Some(1),
            counterparty_chain_ids: Some(vec![100]),
            bridge_ids: Some(vec![2]),
            ..Default::default()
        };
        let sql = sql_messages(&filter);
        assert!(sql.contains("bridge_id"));
        assert!(sql.contains("src_chain_id"));
        assert!(sql.contains("dst_chain_id"));
    }

    #[test]
    fn test_transfers_condition_qualifies_bridge_and_token_columns() {
        let filter = ChainBridgeFilter {
            home_chain_id: Some(1),
            bridge_ids: Some(vec![2]),
            ..Default::default()
        };
        let sql = sql_transfers(&filter);
        assert!(
            sql.contains("\"crosschain_transfers\".\"bridge_id\"")
                || sql.contains("crosschain_transfers.bridge_id"),
            "bridge_id must be table-qualified; got: {sql}"
        );
        assert!(
            sql.contains("token_src_chain_id") && sql.contains("token_dst_chain_id"),
            "expected token chain columns; got: {sql}"
        );
    }

    #[test]
    fn test_transfers_condition_within_set() {
        let filter = ChainBridgeFilter {
            counterparty_chain_ids: Some(vec![1, 250]),
            ..Default::default()
        };
        let sql = sql_transfers(&filter);
        assert!(sql.contains("token_src_chain_id"));
        assert!(sql.contains("token_dst_chain_id"));
    }

    #[test]
    fn test_is_empty() {
        assert!(ChainBridgeFilter::default().is_empty());
        assert!(
            !ChainBridgeFilter {
                home_chain_id: Some(1),
                ..Default::default()
            }
            .is_empty()
        );
    }

    #[test]
    fn test_is_empty_false_for_source_only_and_destination_only() {
        assert!(
            !ChainBridgeFilter {
                src_chain_ids: Some(vec![1]),
                ..Default::default()
            }
            .is_empty(),
            "source-only filter must be non-empty"
        );
        assert!(
            !ChainBridgeFilter {
                dst_chain_ids: Some(vec![100]),
                ..Default::default()
            }
            .is_empty(),
            "destination-only filter must be non-empty"
        );
    }

    /// Returns the WHERE clause (lowercased) of a message-condition query.
    fn messages_where(filter: &ChainBridgeFilter) -> String {
        sql_messages(filter)
            .split_once(" WHERE ")
            .map(|(_, w)| w.to_ascii_lowercase())
            .unwrap_or_default()
    }

    #[test]
    fn test_messages_condition_source_only_has_no_focal_or() {
        let filter = ChainBridgeFilter {
            src_chain_ids: Some(vec![1]),
            ..Default::default()
        };
        let where_sql = messages_where(&filter);
        assert!(where_sql.contains("src_chain_id"));
        assert!(!where_sql.contains("dst_chain_id"));
        // No focal fields => no OR; the directional term is a standalone AND.
        assert!(
            !where_sql.contains(" or "),
            "source-only must not emit a focal OR; got: {where_sql}"
        );
    }

    #[test]
    fn test_messages_condition_destination_only_has_no_focal_or() {
        let filter = ChainBridgeFilter {
            dst_chain_ids: Some(vec![100]),
            ..Default::default()
        };
        let where_sql = messages_where(&filter);
        assert!(where_sql.contains("dst_chain_id"));
        assert!(!where_sql.contains("src_chain_id"));
        assert!(
            !where_sql.contains(" or "),
            "destination-only must not emit a focal OR; got: {where_sql}"
        );
    }

    #[test]
    fn test_messages_condition_both_directions_no_focal() {
        let filter = ChainBridgeFilter {
            src_chain_ids: Some(vec![11]),
            dst_chain_ids: Some(vec![22]),
            ..Default::default()
        };
        let where_sql = messages_where(&filter);
        assert!(where_sql.contains("src_chain_id") && where_sql.contains("11"));
        assert!(where_sql.contains("dst_chain_id") && where_sql.contains("22"));
        assert!(
            !where_sql.contains(" or "),
            "both-directions without focal must not emit a focal OR; got: {where_sql}"
        );
    }

    #[test]
    fn test_messages_condition_focal_plus_both_directions_keeps_focal_or() {
        // X <-> {A,B,C,D} narrowed to X -> {B,C}; directional ids are disjoint
        // from focal ids so their presence proves they sit outside the focal OR.
        let filter = ChainBridgeFilter {
            home_chain_id: Some(1),
            counterparty_chain_ids: Some(vec![100, 200, 300, 400]),
            src_chain_ids: Some(vec![777]),
            dst_chain_ids: Some(vec![888]),
            ..Default::default()
        };
        let where_sql = messages_where(&filter);
        // Focal OR is still present.
        assert!(
            where_sql.contains(" or "),
            "focal OR must remain; got: {where_sql}"
        );
        // Directional terms appear as additional (outer AND) predicates.
        assert!(where_sql.contains("777"), "src directional id missing");
        assert!(where_sql.contains("888"), "dst directional id missing");
        // Focal counterparty ids still present.
        assert!(where_sql.contains("100") && where_sql.contains("400"));
    }

    #[test]
    fn test_messages_condition_counterparties_without_home_plus_direction() {
        let filter = ChainBridgeFilter {
            counterparty_chain_ids: Some(vec![100, 250]),
            src_chain_ids: Some(vec![1]),
            ..Default::default()
        };
        let where_sql = messages_where(&filter);
        // Within-set focal uses src IN (set) AND dst IN (set); directional src IN
        // narrows further.
        assert!(where_sql.contains("src_chain_id"));
        assert!(where_sql.contains("dst_chain_id"));
        assert!(where_sql.contains("250"));
        assert!(where_sql.contains("(1)"), "directional src id missing");
    }

    #[test]
    fn test_messages_condition_focal_direction_bridge_together() {
        let filter = ChainBridgeFilter {
            home_chain_id: Some(1),
            counterparty_chain_ids: Some(vec![100]),
            src_chain_ids: Some(vec![777]),
            dst_chain_ids: Some(vec![888]),
            bridge_ids: Some(vec![2]),
            ..Default::default()
        };
        let where_sql = messages_where(&filter);
        assert!(where_sql.contains(" or "), "focal OR must remain");
        assert!(where_sql.contains("777") && where_sql.contains("888"));
        assert!(where_sql.contains("bridge_id"));
    }

    #[test]
    fn test_transfers_condition_source_only_qualified_token_column() {
        let filter = ChainBridgeFilter {
            src_chain_ids: Some(vec![1]),
            ..Default::default()
        };
        let sql = sql_transfers(&filter);
        assert!(
            sql.contains("token_src_chain_id"),
            "expected token source column; got: {sql}"
        );
        assert!(
            !sql.to_ascii_lowercase()
                .split_once(" where ")
                .map(|(_, w)| w.contains("token_dst_chain_id"))
                .unwrap_or(false),
            "source-only must not filter destination; got: {sql}"
        );
    }

    #[test]
    fn test_transfers_condition_destination_only_qualified_token_column() {
        let filter = ChainBridgeFilter {
            dst_chain_ids: Some(vec![100]),
            ..Default::default()
        };
        let sql = sql_transfers(&filter);
        assert!(
            sql.contains("token_dst_chain_id"),
            "expected token destination column; got: {sql}"
        );
    }

    #[test]
    fn test_transfers_condition_both_directions_and_bridge_qualified() {
        let filter = ChainBridgeFilter {
            home_chain_id: Some(1),
            counterparty_chain_ids: Some(vec![100]),
            src_chain_ids: Some(vec![777]),
            dst_chain_ids: Some(vec![888]),
            bridge_ids: Some(vec![2]),
            ..Default::default()
        };
        let sql = sql_transfers(&filter);
        assert!(sql.contains("token_src_chain_id") && sql.contains("token_dst_chain_id"));
        assert!(sql.contains("777") && sql.contains("888"));
        assert!(
            sql.contains("\"crosschain_transfers\".\"bridge_id\"")
                || sql.contains("crosschain_transfers.bridge_id"),
            "bridge_id must be table-qualified; got: {sql}"
        );
    }

    // --- only_indexed_by_bridge (default-hide unindexed-chain predicate) ---

    fn transfers_where(filter: &ChainBridgeFilter) -> String {
        sql_transfers(filter)
            .split_once(" WHERE ")
            .map(|(_, w)| w.to_ascii_lowercase())
            .unwrap_or_default()
    }

    #[test]
    fn test_messages_condition_only_indexed_by_bridge_renders_permissive_and_per_bridge_disjuncts()
    {
        let filter = ChainBridgeFilter {
            only_indexed_by_bridge: Some(vec![(1, vec![1, 100]), (2, vec![1, 250])]),
            ..Default::default()
        };
        let where_sql = messages_where(&filter);
        assert!(
            where_sql.contains("not in"),
            "expected the permissive `bridge_id NOT IN (..)` arm; got: {where_sql}"
        );
        assert!(
            where_sql.contains("is not null"),
            "expected the `dst_chain_id IS NOT NULL` guard on the permissive arm; got: {where_sql}"
        );
        assert!(where_sql.contains("bridge_id"));
        // One conjunct per bridge naming each bridge id and each chain id.
        assert!(where_sql.contains('1') && where_sql.contains('2'));
        assert!(where_sql.contains("100") && where_sql.contains("250"));
    }

    #[test]
    fn test_messages_condition_only_indexed_by_bridge_none_renders_no_predicate() {
        let filter = ChainBridgeFilter {
            home_chain_id: Some(1),
            ..Default::default()
        };
        let where_sql = messages_where(&filter);
        assert!(!where_sql.contains("not in"), "got: {where_sql}");
        assert!(!where_sql.contains("is not null"), "got: {where_sql}");
    }

    #[test]
    fn test_messages_condition_only_indexed_by_bridge_empty_pairs_is_permissive_not_false() {
        let filter = ChainBridgeFilter {
            only_indexed_by_bridge: Some(vec![]),
            ..Default::default()
        };
        let where_sql = messages_where(&filter);
        assert!(
            where_sql.contains("is not null"),
            "empty pair list must render the permissive arm (dst IS NOT NULL); got: {where_sql}"
        );
        assert!(
            !where_sql.contains("false"),
            "empty pair list must not render a FALSE literal; got: {where_sql}"
        );
    }

    #[test]
    fn test_messages_condition_only_indexed_by_bridge_present_but_empty_bridge_cannot_match() {
        let filter = ChainBridgeFilter {
            only_indexed_by_bridge: Some(vec![(1, vec![])]),
            ..Default::default()
        };
        let where_sql = messages_where(&filter);
        // Bridge 1 is present, so it must sit in the restrictive `NOT IN` list...
        assert!(where_sql.contains("not in"));
        assert!(where_sql.contains("bridge_id"));
        // ...and its own disjunct can never match: sea_query renders `is_in([])`
        // as `1 = 2`, which is what makes an empty-set bridge restrictive.
        assert!(
            where_sql.contains("1 = 2"),
            "expected an unsatisfiable disjunct for the empty chain set; got: {where_sql}"
        );
    }

    #[test]
    fn test_messages_condition_only_indexed_by_bridge_composes_with_focal_directional_and_bridge_filter()
     {
        let filter = ChainBridgeFilter {
            home_chain_id: Some(1),
            counterparty_chain_ids: Some(vec![100, 250]),
            src_chain_ids: Some(vec![1]),
            bridge_ids: Some(vec![1]),
            only_indexed_by_bridge: Some(vec![(1, vec![1, 100])]),
            ..Default::default()
        };
        let where_sql = messages_where(&filter);
        assert!(
            where_sql.contains(" or "),
            "focal OR must remain intact; got: {where_sql}"
        );
        assert!(where_sql.contains("100") && where_sql.contains("250"));
        assert!(
            where_sql.contains("not in"),
            "the unindexed-chain disjunction must still be a separate AND term; got: {where_sql}"
        );
    }

    #[test]
    fn test_is_empty_false_for_only_indexed_by_bridge_only() {
        assert!(
            !ChainBridgeFilter {
                only_indexed_by_bridge: Some(vec![(1, vec![1, 100])]),
                ..Default::default()
            }
            .is_empty(),
            "only_indexed_by_bridge-only filter must be non-empty"
        );
    }

    #[test]
    fn test_transfers_condition_only_indexed_by_bridge_permissive_without_null_guard() {
        let filter = ChainBridgeFilter {
            only_indexed_by_bridge: Some(vec![(1, vec![1, 100]), (2, vec![1, 250])]),
            ..Default::default()
        };
        let where_sql = transfers_where(&filter);
        assert!(
            where_sql.contains("not in"),
            "expected the permissive `bridge_id NOT IN (..)` arm; got: {where_sql}"
        );
        assert!(
            !where_sql.contains("is not null"),
            "both transfer token chain columns are NOT NULL, so no NULL guard is needed; got: {where_sql}"
        );
        assert!(where_sql.contains("100") && where_sql.contains("250"));
    }

    #[test]
    fn test_transfers_condition_only_indexed_by_bridge_empty_pairs_is_permissive_not_false() {
        let filter = ChainBridgeFilter {
            only_indexed_by_bridge: Some(vec![]),
            ..Default::default()
        };
        let sql = sql_transfers(&filter);
        assert!(
            !sql.to_ascii_lowercase().contains("false"),
            "empty pair list must not render a FALSE literal; got: {sql}"
        );
    }

    #[test]
    fn test_transfers_condition_only_indexed_by_bridge_present_but_empty_bridge_cannot_match() {
        let filter = ChainBridgeFilter {
            only_indexed_by_bridge: Some(vec![(1, vec![])]),
            ..Default::default()
        };
        let where_sql = transfers_where(&filter);
        assert!(where_sql.contains("not in"));
        assert!(
            where_sql.contains("1 = 2"),
            "expected an unsatisfiable disjunct for the empty chain set; got: {where_sql}"
        );
    }
}
