// SPDX-License-Identifier: LicenseRef-Blockscout

use interchain_indexer_entity::{crosschain_messages, crosschain_transfers};
use sea_orm::{ColumnTrait, Condition, sea_query::Expr};

/// Optional read-time chain/bridge filter applied to list and counter queries.
///
/// Invariants: `counterparty_chain_ids` / `bridge_ids` are `Some` only when non-empty.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChainBridgeFilter {
    pub home_chain_id: Option<i64>,
    pub counterparty_chain_ids: Option<Vec<i64>>,
    pub bridge_ids: Option<Vec<i32>>,
}

impl ChainBridgeFilter {
    pub fn is_empty(&self) -> bool {
        self.home_chain_id.is_none()
            && self.counterparty_chain_ids.is_none()
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

        let mut cond = Condition::all().add(chain);
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

        let mut cond = Condition::all().add(chain);
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
}
