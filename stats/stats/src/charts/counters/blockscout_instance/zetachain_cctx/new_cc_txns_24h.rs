use crate::{
    ChartProperties, IndexingStatus, MissingDatePolicy, Named,
    chart_prelude::*,
    data_source::{
        kinds::{
            data_manipulation::map::MapToString,
            local_db::DirectPointLocalDbChartSource,
            remote_db::{PullOneNowValue, RemoteDatabaseSource, StatementFromUpdateTime},
        },
        types::IndexerMigrations,
    },
    indexing_status::{IndexingStatusTrait, ZetachainCctxIndexingStatus},
    lines::zetachain_cctx_datetime_range_filter,
    utils::interval_24h,
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, QuerySelect, QueryTrait, prelude::*};
use sea_query::{Asterisk, Func, IntoColumnRef};

pub struct NewZetachainCrossChainTxns24hStatement;
impl_db_choice!(NewZetachainCrossChainTxns24hStatement, UseZetachainCctxDB);

impl StatementFromUpdateTime for NewZetachainCrossChainTxns24hStatement {
    fn get_statement(
        update_time: DateTime<Utc>,
        _completed_migrations: &IndexerMigrations,
    ) -> sea_orm::Statement {
        let interval_24h = interval_24h(update_time);
        zetachain_cctx_entity::cross_chain_tx::Entity::find()
            .select_only()
            .expr_as(Func::count(Asterisk.into_column_ref()), "value")
            .left_join(zetachain_cctx_entity::cctx_status::Entity)
            .filter(zetachain_cctx_datetime_range_filter(interval_24h.into()))
            .build(DbBackend::Postgres)
    }
}

pub type NewZetachainCrossChainTxns24hRemote =
    RemoteDatabaseSource<PullOneNowValue<NewZetachainCrossChainTxns24hStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newZetachainCrossChainTxns24h".into()
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

pub type NewZetachainCrossChainTxns24h =
    DirectPointLocalDbChartSource<MapToString<NewZetachainCrossChainTxns24hRemote>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{
        point_construction::dt, simple_test::simple_test_counter_with_zetachain_cctx,
    };

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_zetachain_cross_chain_txns_24h() {
        simple_test_counter_with_zetachain_cctx::<NewZetachainCrossChainTxns24h>(
            "update_new_zetachain_cross_chain_txns_24h",
            "1",
            Some(dt("2022-11-11T11:30:00")),
        )
        .await;
    }
}
