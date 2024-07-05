//! Total transaction fees for an interval

use std::ops::Range;

use crate::{
    charts::types::DateValueDouble,
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

const ETHER: i64 = i64::pow(10, 18);

pub struct TxnsFeeStatement;

impl StatementFromRange for TxnsFeeStatement {
    fn get_statement(range: Option<Range<DateTimeUtc>>) -> Statement {
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

pub type TxnsFeeRemote =
    RemoteDatabaseSource<PullAllWithAndSort<TxnsFeeStatement, DateValueDouble>>;

pub type TxnsFeeRemoteString = MapToString<TxnsFeeRemote>;

pub struct TxnsFeeProperties;

impl Named for TxnsFeeProperties {
    const NAME: &'static str = "txnsFee";
}

impl ChartProperties for TxnsFeeProperties {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

pub type TxnsFee = DirectVecLocalDbChartSource<TxnsFeeRemoteString, TxnsFeeProperties>;

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
