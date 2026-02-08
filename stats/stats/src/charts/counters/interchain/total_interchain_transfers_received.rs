//! Total interchain transfers received (by associated message: dst_tx_hash IS NOT NULL).
//! When interchain_primary_id is set, filters by message's dst_chain_id; otherwise counts all received.

use crate::chart_prelude::*;

pub struct TotalInterchainTransfersReceivedStatement;
impl_db_choice!(TotalInterchainTransfersReceivedStatement, UsePrimaryDB);

impl StatementFromUpdateTime for TotalInterchainTransfersReceivedStatement {
    fn get_statement_with_context(cx: &UpdateContext<'_>) -> sea_orm::Statement {
        match cx.interchain_primary_id {
            Some(primary_id) => sea_orm::Statement::from_sql_and_values(
                sea_orm::DbBackend::Postgres,
                r#"
                SELECT COUNT(*)::bigint AS value
                FROM crosschain_transfers t
                INNER JOIN crosschain_messages m ON t.message_id = m.id
                WHERE m.dst_tx_hash IS NOT NULL AND m.dst_chain_id = $1
                "#,
                [sea_orm::Value::BigInt(Some(primary_id as i64))],
            ),
            None => sea_orm::Statement::from_sql_and_values(
                sea_orm::DbBackend::Postgres,
                r#"
                SELECT COUNT(*)::bigint AS value
                FROM crosschain_transfers t
                INNER JOIN crosschain_messages m ON t.message_id = m.id
                WHERE m.dst_tx_hash IS NOT NULL
                "#,
                [],
            ),
        }
    }
}

pub type TotalInterchainTransfersReceivedRemote = RemoteDatabaseSource<
    PullOneNowValue<TotalInterchainTransfersReceivedStatement, NaiveDate, i64>,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalInterchainTransfersReceived".into()
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

pub type TotalInterchainTransfersReceived =
    DirectPointLocalDbChartSource<MapToString<TotalInterchainTransfersReceivedRemote>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter_interchain;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_interchain_transfers_received() {
        simple_test_counter_interchain::<TotalInterchainTransfersReceived>(
            "update_total_interchain_transfers_received",
            "20",
            None,
            None,
        )
        .await;

        simple_test_counter_interchain::<TotalInterchainTransfersReceived>(
            "update_total_interchain_transfers_received_primary_1",
            "6",
            None,
            Some(1),
        )
        .await;
    }
}
