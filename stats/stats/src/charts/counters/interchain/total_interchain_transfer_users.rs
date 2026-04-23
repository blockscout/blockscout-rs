//! Total unique interchain transfer users (distinct sender_address and recipient_address).
//! When interchain_primary_id is set, only transfers whose message has src_chain_id or dst_chain_id = primary_id.

use crate::chart_prelude::*;

pub struct TotalInterchainTransferUsersStatement;
impl_db_choice!(TotalInterchainTransferUsersStatement, UsePrimaryDB);

impl StatementFromUpdateTime for TotalInterchainTransferUsersStatement {
    fn get_statement_with_context(cx: &UpdateContext<'_>) -> sea_orm::Statement {
        match cx.interchain_primary_id {
            Some(primary_id) => sea_orm::Statement::from_sql_and_values(
                sea_orm::DbBackend::Postgres,
                r#"
                SELECT COUNT(*)::bigint AS value
                FROM (
                    SELECT t.sender_address AS addr
                    FROM crosschain_transfers t
                    INNER JOIN crosschain_messages m ON t.message_id = m.id
                    WHERE t.sender_address IS NOT NULL
                      AND (m.src_chain_id = $1 OR m.dst_chain_id = $1)
                    UNION
                    SELECT t.recipient_address AS addr
                    FROM crosschain_transfers t
                    INNER JOIN crosschain_messages m ON t.message_id = m.id
                    WHERE t.recipient_address IS NOT NULL
                      AND (m.src_chain_id = $1 OR m.dst_chain_id = $1)
                ) u
                "#,
                [sea_orm::Value::BigInt(Some(primary_id as i64))],
            ),
            None => sea_orm::Statement::from_sql_and_values(
                sea_orm::DbBackend::Postgres,
                r#"
                SELECT COUNT(*)::bigint AS value
                FROM (
                    SELECT sender_address AS addr
                    FROM crosschain_transfers
                    WHERE sender_address IS NOT NULL
                    UNION
                    SELECT recipient_address AS addr
                    FROM crosschain_transfers
                    WHERE recipient_address IS NOT NULL
                ) u
                "#,
                [],
            ),
        }
    }
}

pub type TotalInterchainTransferUsersRemote =
    RemoteDatabaseSource<PullOneNowValue<TotalInterchainTransferUsersStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalInterchainTransferUsers".into()
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

pub type TotalInterchainTransferUsers =
    DirectPointLocalDbChartSource<MapToString<TotalInterchainTransferUsersRemote>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter_interchain;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_interchain_transfer_users() {
        simple_test_counter_interchain::<TotalInterchainTransferUsers>(
            "update_total_interchain_transfer_users",
            "8",
            None,
            None,
        )
        .await;
    }
}
