//! Total interchain transfers (total indexed count, no filters).
//! Counts all rows in crosschain_transfers. Does not use interchain_primary_id.

use crate::chart_prelude::*;

pub struct TotalInterchainTransfersStatement;
impl_db_choice!(TotalInterchainTransfersStatement, UsePrimaryDB);

impl StatementFromUpdateTime for TotalInterchainTransfersStatement {
    fn get_statement_with_context(_cx: &UpdateContext<'_>) -> sea_orm::Statement {
        sea_orm::Statement::from_sql_and_values(
            sea_orm::DbBackend::Postgres,
            r#"
            SELECT COUNT(*)::bigint AS value
            FROM crosschain_transfers
            "#,
            [],
        )
    }
}

pub type TotalInterchainTransfersRemote =
    RemoteDatabaseSource<PullOneNowValue<TotalInterchainTransfersStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalInterchainTransfers".into()
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

pub type TotalInterchainTransfers =
    DirectPointLocalDbChartSource<MapToString<TotalInterchainTransfersRemote>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter_interchain;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_interchain_transfers() {
        simple_test_counter_interchain::<TotalInterchainTransfers>(
            "update_total_interchain_transfers",
            "41",
            None,
            None,
        )
        .await;
    }
}
