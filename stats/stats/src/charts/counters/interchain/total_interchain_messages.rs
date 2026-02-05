//! Total interchain messages (total indexed count, no filters).
//! Counts all rows in crosschain_messages. Does not use interchain_primary_id.

use crate::chart_prelude::*;

pub struct TotalInterchainMessagesStatement;
impl_db_choice!(TotalInterchainMessagesStatement, UsePrimaryDB);

impl StatementFromUpdateTime for TotalInterchainMessagesStatement {
    fn get_statement_with_context(_cx: &UpdateContext<'_>) -> sea_orm::Statement {
        sea_orm::Statement::from_sql_and_values(
            sea_orm::DbBackend::Postgres,
            r#"
            SELECT COUNT(*)::bigint AS value
            FROM crosschain_messages
            "#,
            [],
        )
    }
}

pub type TotalInterchainMessagesRemote =
    RemoteDatabaseSource<PullOneNowValue<TotalInterchainMessagesStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalInterchainMessages".into()
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

pub type TotalInterchainMessages =
    DirectPointLocalDbChartSource<MapToString<TotalInterchainMessagesRemote>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter_interchain;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_interchain_messages() {
        simple_test_counter_interchain::<TotalInterchainMessages>(
            "update_total_interchain_messages",
            "21",
            None,
            None,
        )
        .await;
    }
}
