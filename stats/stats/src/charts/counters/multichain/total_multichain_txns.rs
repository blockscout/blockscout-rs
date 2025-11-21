use crate::{chart_prelude::*, utils::sql_with_multichain_filter_opt};

pub struct TotalMultichainTxnsStatement;
impl_db_choice!(TotalMultichainTxnsStatement, UsePrimaryDB);

impl StatementFromUpdateTime for TotalMultichainTxnsStatement {
    fn get_statement_with_context(cx: &UpdateContext<'_>) -> sea_orm::Statement {
        sql_with_multichain_filter_opt!(
            DbBackend::Postgres,
            r#"
            SELECT COALESCE(SUM(total_transactions_number), 0)::bigint AS value
            FROM (
                SELECT DISTINCT ON (chain_id) chain_id, total_transactions_number
                FROM counters_global_imported
                WHERE date <= $1{multichain_filter}
                ORDER BY chain_id, date DESC
            ) t
            "#,
            [cx.time.into()],
            "chain_id",
            &cx.multichain_filter,
        )
    }
}

pub type TotalMultichainTxnsRemote =
    RemoteDatabaseSource<PullOneNowValue<TotalMultichainTxnsStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalMultichainTxns".into()
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

pub type TotalMultichainTxns =
    DirectPointLocalDbChartSource<MapToString<TotalMultichainTxnsRemote>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter_multichain};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_multichain_txns() {
        simple_test_counter_multichain::<TotalMultichainTxns>(
            "update_total_multichain_txns",
            "210",
            None,
            None,
        )
        .await;

        simple_test_counter_multichain::<TotalMultichainTxns>(
            "update_total_multichain_txns",
            "101",
            Some(dt("2023-02-02T00:00:00")),
            None,
        )
        .await;

        simple_test_counter_multichain::<TotalMultichainTxns>(
            "update_total_multichain_txns",
            "155",
            None,
            Some(vec![1, 3]),
        )
        .await;
    }
}
