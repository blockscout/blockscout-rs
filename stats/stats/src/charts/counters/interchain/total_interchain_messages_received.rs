//! Total interchain messages received on the primary chain.
//! Counts messages where the destination event was indexed (dst_tx_hash IS NOT NULL).
//! When interchain_primary_id is set, filters by dst_chain_id; otherwise counts all received messages.

use crate::chart_prelude::*;

pub struct TotalInterchainMessagesReceivedStatement;
impl_db_choice!(TotalInterchainMessagesReceivedStatement, UsePrimaryDB);

impl StatementFromUpdateTime for TotalInterchainMessagesReceivedStatement {
    fn get_statement_with_context(cx: &UpdateContext<'_>) -> sea_orm::Statement {
        match cx.interchain_primary_id {
            Some(primary_id) => sea_orm::Statement::from_sql_and_values(
                sea_orm::DbBackend::Postgres,
                r#"
                SELECT COUNT(*)::bigint AS value
                FROM crosschain_messages
                WHERE dst_chain_id = $1 AND dst_tx_hash IS NOT NULL
                "#,
                [sea_orm::Value::BigInt(Some(primary_id as i64))],
            ),
            None => sea_orm::Statement::from_sql_and_values(
                sea_orm::DbBackend::Postgres,
                r#"
                SELECT COUNT(*)::bigint AS value
                FROM crosschain_messages
                WHERE dst_tx_hash IS NOT NULL
                "#,
                [],
            ),
        }
    }
}

pub type TotalInterchainMessagesReceivedRemote =
    RemoteDatabaseSource<PullOneNowValue<TotalInterchainMessagesReceivedStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalInterchainMessagesReceived".into()
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

pub type TotalInterchainMessagesReceived =
    DirectPointLocalDbChartSource<MapToString<TotalInterchainMessagesReceivedRemote>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter_interchain;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_interchain_messages_received() {
        simple_test_counter_interchain::<TotalInterchainMessagesReceived>(
            "update_total_interchain_messages_received",
            "13",
            None,
            None,
        )
        .await;

        simple_test_counter_interchain::<TotalInterchainMessagesReceived>(
            "update_total_interchain_messages_received_primary_1",
            "6",
            None,
            Some(1),
        )
        .await;
    }
}
