use std::ops::RangeInclusive;

use crate::{
    charts::db_interaction::types::DateValueDouble,
    data_source::kinds::{
        adapter::{ToStringAdapter, ToStringAdapterWrapper},
        remote::{RemoteSource, RemoteSourceWrapper},
        updateable_chart::batch::clone::{CloneChart, CloneChartWrapper},
    },
    utils::sql_with_range_filter_opt,
    Chart, Named,
};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

pub struct TxnsSuccessRateRemote;

impl RemoteSource for TxnsSuccessRateRemote {
    type Point = DateValueDouble;
    fn get_query(range: Option<RangeInclusive<DateTimeUtc>>) -> Statement {
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

pub struct TxnsSuccessRateRemoteString;

impl ToStringAdapter for TxnsSuccessRateRemoteString {
    type InnerSource = RemoteSourceWrapper<TxnsSuccessRateRemote>;
    type ConvertFrom = <TxnsSuccessRateRemote as RemoteSource>::Point;
}

pub struct TxnsSuccessRateInner;

impl Named for TxnsSuccessRateInner {
    const NAME: &'static str = "txnsSuccessRate";
}

impl Chart for TxnsSuccessRateInner {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

impl CloneChart for TxnsSuccessRateInner {
    type Dependency = ToStringAdapterWrapper<TxnsSuccessRateRemoteString>;
}

pub type TxnsSuccessRate = CloneChartWrapper<TxnsSuccessRateInner>;

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
