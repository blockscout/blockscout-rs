//! Total interchain messages sent from the primary chain.
//! Counts messages where the source event was indexed (src_tx_hash IS NOT NULL).
//! When interchain_primary_id is set, filters by src_chain_id; otherwise counts all sent messages.

use crate::chart_prelude::*;

pub struct TotalInterchainMessagesSentStatement;
impl_db_choice!(TotalInterchainMessagesSentStatement, UsePrimaryDB);

impl StatementFromUpdateTime for TotalInterchainMessagesSentStatement {
    fn get_statement_with_context(cx: &UpdateContext<'_>) -> sea_orm::Statement {
        match cx.interchain_primary_id {
            Some(primary_id) => sea_orm::Statement::from_sql_and_values(
                sea_orm::DbBackend::Postgres,
                r#"
                SELECT COUNT(*)::bigint AS value
                FROM crosschain_messages
                WHERE src_chain_id = $1 AND src_tx_hash IS NOT NULL
                "#,
                [sea_orm::Value::BigInt(Some(primary_id as i64))],
            ),
            None => sea_orm::Statement::from_sql_and_values(
                sea_orm::DbBackend::Postgres,
                r#"
                SELECT COUNT(*)::bigint AS value
                FROM crosschain_messages
                WHERE src_tx_hash IS NOT NULL
                "#,
                [],
            ),
        }
    }
}

pub type TotalInterchainMessagesSentRemote =
    RemoteDatabaseSource<PullOneNowValue<TotalInterchainMessagesSentStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalInterchainMessagesSent".into()
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

pub type TotalInterchainMessagesSent =
    DirectPointLocalDbChartSource<MapToString<TotalInterchainMessagesSentRemote>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter_interchain;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_interchain_messages_sent() {
        simple_test_counter_interchain::<TotalInterchainMessagesSent>(
            "update_total_interchain_messages_sent",
            "15",
            None,
            None,
        )
        .await;

        simple_test_counter_interchain::<TotalInterchainMessagesSent>(
            "update_total_interchain_messages_sent_primary_1",
            "11",
            None,
            Some(1),
        )
        .await;
    }
}
