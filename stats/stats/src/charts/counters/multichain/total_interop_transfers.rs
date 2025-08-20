use crate::{
    ChartProperties, IndexingStatus, MissingDatePolicy, Named,
    data_source::{
        kinds::{
            data_manipulation::map::MapToString,
            local_db::DirectPointLocalDbChartSource,
            remote_db::{PullOneNowValue, RemoteDatabaseSource, StatementFromUpdateTime},
        },
        types::IndexerMigrations,
    },
    indexing_status::IndexingStatusTrait,
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use migration::{Asterisk, Func, IntoColumnRef};
use multichain_aggregator_entity::{interop_messages, interop_messages_transfers};
use sea_orm::{ColumnTrait, DbBackend, EntityTrait, QueryFilter, QuerySelect, QueryTrait};

pub struct TotalInteropTransfersStatement;

impl StatementFromUpdateTime for TotalInteropTransfersStatement {
    fn get_statement(
        update_time: DateTime<Utc>,
        _completed_migrations: &IndexerMigrations,
    ) -> sea_orm::Statement {
        interop_messages_transfers::Entity::find()
            .select_only()
            .inner_join(interop_messages::Entity)
            .filter(interop_messages::Column::Timestamp.lte(update_time))
            .expr_as(Func::count(Asterisk.into_column_ref()), "value")
            .build(DbBackend::Postgres)
    }
}

pub type TotalInteropTransfersRemote =
    RemoteDatabaseSource<PullOneNowValue<TotalInteropTransfersStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalInteropTransfers".into()
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
    }
}

pub type TotalInteropTransfers =
    DirectPointLocalDbChartSource<MapToString<TotalInteropTransfersRemote>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter_multichain};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_interop_transfers() {
        simple_test_counter_multichain::<TotalInteropTransfers>(
            "update_total_interop_transfers",
            "3",
            None,
        )
        .await;

        simple_test_counter_multichain::<TotalInteropTransfers>(
            "update_total_interop_transfers",
            "1",
            Some(dt("2022-11-09T23:59:59")),
        )
        .await;
    }
}
