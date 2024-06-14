use std::ops::RangeInclusive;

use crate::{
    charts::db_interaction::types::DateValueDouble,
    data_source::kinds::{
        data_manipulation::map::MapToString,
        local_db::DirectVecLocalDbChartSource,
        remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
    },
    utils::sql_with_range_filter_opt,
    ChartProperties, Named,
};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

pub struct TxnsSuccessRateStatement;

impl StatementFromRange for TxnsSuccessRateStatement {
    fn get_statement(range: Option<RangeInclusive<DateTimeUtc>>) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                    SELECT 
                        DATE(b.timestamp) as date, 
                        COUNT(CASE WHEN t.error IS NULL THEN 1 END)::FLOAT
                            / COUNT(*)::FLOAT as value
                    FROM transactions t
                    JOIN blocks       b ON t.block_hash = b.hash
                    WHERE 
                        b.timestamp != to_timestamp(0) AND
                        b.consensus = true AND
                        t.block_hash IS NOT NULL AND 
                        (t.error IS NULL OR t.error::text != 'dropped/replaced') {filter}
                    GROUP BY DATE(b.timestamp)
                "#,
            [],
            "b.timestamp",
            range
        )
    }
}

pub type TxnsSuccessRateRemote =
    RemoteDatabaseSource<PullAllWithAndSort<TxnsSuccessRateStatement, DateValueDouble>>;

pub type TxnsSuccessRateRemoteString = MapToString<TxnsSuccessRateRemote>;

pub struct TxnsSuccessRateProperties;

impl Named for TxnsSuccessRateProperties {
    const NAME: &'static str = "txnsSuccessRate";
}

impl ChartProperties for TxnsSuccessRateProperties {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

pub type TxnsSuccessRate =
    DirectVecLocalDbChartSource<TxnsSuccessRateRemoteString, TxnsSuccessRateProperties>;

#[cfg(test)]
mod tests {
    use super::TxnsSuccessRate;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_success_rate() {
        simple_test_chart::<TxnsSuccessRate>(
            "update_txns_success_rate",
            vec![
                ("2022-11-09", "1"),
                ("2022-11-10", "1"),
                ("2022-11-11", "1"),
                ("2022-11-12", "1"),
                ("2022-12-01", "1"),
                ("2023-01-01", "1"),
                ("2023-02-01", "1"),
            ],
        )
        .await;
    }
}
