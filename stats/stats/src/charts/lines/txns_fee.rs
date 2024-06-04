//! Total transaction fees for an interval
use crate::{
    charts::db_interaction::types::DateValueDouble,
    data_source::kinds::{
        adapter::{ToStringAdapter, ToStringAdapterWrapper},
        remote::{RemoteSource, RemoteSourceWrapper},
        updateable_chart::clone::{CloneChart, CloneChartWrapper},
    },
    utils::sql_with_range_filter_opt,
    Chart, Named,
};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

const ETHER: i64 = i64::pow(10, 18);

pub struct TxnsFeeRemote;

impl RemoteSource for TxnsFeeRemote {
    type Point = DateValueDouble;
    fn get_query(range: Option<std::ops::RangeInclusive<DateTimeUtc>>) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT 
                    DATE(b.timestamp) as date, 
                    (SUM(t.gas_used * t.gas_price) / $1)::FLOAT as value
                FROM transactions t
                JOIN blocks       b ON t.block_hash = b.hash
                WHERE
                    b.timestamp != to_timestamp(0) AND
                    b.consensus = true {filter}
                GROUP BY DATE(b.timestamp)
            "#,
            [ETHER.into()],
            "b.timestamp",
            range
        )
    }
}

pub struct TxnsFeeRemoteString;

impl ToStringAdapter for TxnsFeeRemoteString {
    type InnerSource = RemoteSourceWrapper<TxnsFeeRemote>;
    type ConvertFrom = <TxnsFeeRemote as RemoteSource>::Point;
}

pub struct TxnsFeeInner;

impl Named for TxnsFeeInner {
    const NAME: &'static str = "txnsFee";
}

impl Chart for TxnsFeeInner {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

impl CloneChart for TxnsFeeInner {
    type Dependency = ToStringAdapterWrapper<TxnsFeeRemoteString>;
}

pub type TxnsFee = CloneChartWrapper<TxnsFeeInner>;

#[cfg(test)]
mod tests {
    use super::TxnsFee;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_fee() {
        simple_test_chart::<TxnsFee>(
            "update_txns_fee",
            vec![
                ("2022-11-09", "0.000047185185138"),
                ("2022-11-10", "0.000495444443949"),
                ("2022-11-11", "0.000967296295329"),
                ("2022-11-12", "0.000613407406794"),
                ("2022-12-01", "0.000684185184501"),
                ("2023-01-01", "0.000023592592569"),
                ("2023-02-01", "0.000802148147346"),
                ("2023-03-01", "0.000023592592569"),
            ],
        )
        .await;
    }
}
