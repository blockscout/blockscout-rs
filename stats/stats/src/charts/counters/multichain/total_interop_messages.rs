use multichain_aggregator_entity::interop_messages;

use crate::chart_prelude::*;

pub struct TotalInteropMessagesStatement;
impl_db_choice!(TotalInteropMessagesStatement, UsePrimaryDB);

impl StatementFromUpdateTime for TotalInteropMessagesStatement {
    fn get_statement(
        update_time: DateTime<Utc>,
        _completed_migrations: &IndexerMigrations,
    ) -> sea_orm::Statement {
        interop_messages::Entity::find()
            .select_only()
            .filter(interop_messages::Column::Timestamp.lte(update_time))
            .expr_as(Func::count(Asterisk.into_column_ref()), "value")
            .build(DbBackend::Postgres)
    }
}

pub type TotalInteropMessagesRemote =
    RemoteDatabaseSource<PullOneNowValue<TotalInteropMessagesStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalInteropMessages".into()
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

pub type TotalInteropMessages =
    DirectPointLocalDbChartSource<MapToString<TotalInteropMessagesRemote>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter_multichain};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_interop_messages() {
        simple_test_counter_multichain::<TotalInteropMessages>(
            "update_total_interop_messages",
            "6",
            None,
        )
        .await;

        simple_test_counter_multichain::<TotalInteropMessages>(
            "update_total_interop_messages",
            "4",
            Some(dt("2022-11-15T12:00:00")),
        )
        .await;
    }
}
