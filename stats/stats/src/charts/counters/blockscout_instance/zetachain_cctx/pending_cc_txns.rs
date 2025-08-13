use std::ops::Bound;

use crate::{chart_prelude::*, lines::zetachain_cctx_datetime_range_filter};

use zetachain_cctx_entity::sea_orm_active_enums::CctxStatusStatus;

pub struct PendingZetachainCrossChainTxnsStatement;
impl_db_choice!(PendingZetachainCrossChainTxnsStatement, UseZetachainCctxDB);

impl StatementFromUpdateTime for PendingZetachainCrossChainTxnsStatement {
    fn get_statement(
        update_time: DateTime<Utc>,
        _completed_migrations: &IndexerMigrations,
    ) -> sea_orm::Statement {
        zetachain_cctx_entity::cross_chain_tx::Entity::find()
            .select_only()
            .expr_as(Func::count(Asterisk.into_column_ref()), "value")
            .left_join(zetachain_cctx_entity::cctx_status::Entity)
            .filter(zetachain_cctx_entity::cctx_status::Column::Status.is_in([
                CctxStatusStatus::PendingRevert,
                CctxStatusStatus::PendingOutbound,
                CctxStatusStatus::PendingInbound,
            ]))
            .filter(zetachain_cctx_datetime_range_filter(UniversalRange {
                start: None,
                end: Bound::Included(update_time),
            }))
            .build(DbBackend::Postgres)
    }
}

pub type PendingZetachainCrossChainTxnsRemote =
    RemoteDatabaseSource<PullOneNowValue<PendingZetachainCrossChainTxnsStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        // note: not bounded by time
        "pendingZetachainCrossChainTxns".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Counter
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
    fn indexing_status_requirement() -> IndexingStatus {
        IndexingStatus::LEAST_RESTRICTIVE
            .with_zetachain_cctx(ZetachainCctxIndexingStatus::IndexedHistoricalData)
    }
}

pub type PendingZetachainCrossChainTxns =
    DirectPointLocalDbChartSource<MapToString<PendingZetachainCrossChainTxnsRemote>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{
        point_construction::dt, simple_test::simple_test_counter_with_zetachain_cctx,
    };

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_pending_zetachain_cross_chain_txns() {
        simple_test_counter_with_zetachain_cctx::<PendingZetachainCrossChainTxns>(
            "update_pending_zetachain_cross_chain_txns",
            "1",
            Some(dt("2022-11-11T11:30:00")),
        )
        .await;
    }
}
