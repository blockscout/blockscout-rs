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
}

impl ChainBridgeFilter {
    pub fn is_empty(&self) -> bool {
        self.home_chain_id.is_none()
            && self.counterparty_chain_ids.is_none()
            && self.src_chain_ids.is_none()
            && self.dst_chain_ids.is_none()
            && self.bridge_ids.is_none()
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
}
