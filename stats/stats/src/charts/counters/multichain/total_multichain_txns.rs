use crate::{
    ChartError, ChartProperties, IndexingStatus, MissingDatePolicy, Named,
    data_source::{
        UpdateContext,
        kinds::{
            local_db::DirectPointLocalDbChartSource,
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour},
        },
    },
    indexing_status::IndexingStatusTrait,
    range::UniversalRange,
    types::timespans::DateValue,
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, FromQueryResult, Statement, prelude::BigDecimal};

pub struct TotalMultichainTxnsQueryBehaviour;

#[derive(Debug, FromQueryResult)]
struct SumResult {
    sum_total: Option<BigDecimal>,
}

impl RemoteQueryBehaviour for TotalMultichainTxnsQueryBehaviour {
    type Output = DateValue<String>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Self::Output, ChartError> {
        let db = cx.indexer_db;
        let timespan = cx.time;

        let stmt = Statement::from_string(
            DbBackend::Postgres,
            r#"
            SELECT SUM(total_transactions_number) AS sum_total
            FROM (
                SELECT DISTINCT ON (chain_id) chain_id, total_transactions_number
                FROM counters_global_imported
                ORDER BY chain_id, date DESC
            ) t
            "#
            .to_string(),
        );

        let result = SumResult::find_by_statement(stmt)
            .one(db)
            .await
            .map_err(ChartError::IndexerDB)?
            .map(|r| r.sum_total.unwrap_or(BigDecimal::from(0)))
            .unwrap_or(BigDecimal::from(0));

        let data = DateValue::<String> {
            timespan: timespan.date_naive(),
            value: result.to_string(),
        };
        Ok(data)
    }
}

pub type TotalMultichainTxnsRemote = RemoteDatabaseSource<TotalMultichainTxnsQueryBehaviour>;

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

pub type TotalMultichainTxns = DirectPointLocalDbChartSource<TotalMultichainTxnsRemote, Properties>;

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
            Some(dt("2022-08-06T00:00:00")),
        )
        .await;
    }
}
