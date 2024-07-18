//! Average fee per transaction

use std::ops::Range;

use crate::{
    data_source::kinds::{
        data_manipulation::map::MapToString,
        local_db::{
            parameters::update::batching::parameters::Batch30Days, DirectVecLocalDbChartSource,
        },
        remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
    },
    utils::sql_with_range_filter_opt,
    ChartProperties, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

const ETHER: i64 = i64::pow(10, 18);

pub struct AverageTxnFeeStatement;

impl StatementFromRange for AverageTxnFeeStatement {
    fn get_statement(range: Option<Range<DateTimeUtc>>) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT
                    DATE(b.timestamp) as date,
                    (AVG(t.gas_used * t.gas_price) / $1)::FLOAT as value
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

pub type AverageTxnFeeRemote =
    RemoteDatabaseSource<PullAllWithAndSort<AverageTxnFeeStatement, NaiveDate, f64>>;

pub type AverageTxnFeeRemoteString = MapToString<AverageTxnFeeRemote>;

pub struct AverageTxnFeeProperties;

impl Named for AverageTxnFeeProperties {
    fn name() -> String {
        "averageTxnFee".into()
    }
}

impl ChartProperties for AverageTxnFeeProperties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

pub type AverageTxnFee =
    DirectVecLocalDbChartSource<AverageTxnFeeRemoteString, Batch30Days, AverageTxnFeeProperties>;

#[cfg(test)]
mod tests {
    use super::AverageTxnFee;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_txn_fee() {
        simple_test_chart::<AverageTxnFee>(
            "update_average_txn_fee",
            vec![
                ("2022-11-09", "0.0000094370370276"),
                ("2022-11-10", "0.00004128703699575"),
                ("2022-11-11", "0.0000690925925235"),
                ("2022-11-12", "0.0001226814813588"),
                ("2022-12-01", "0.0001368370369002"),
                ("2023-01-01", "0.000023592592569"),
                ("2023-02-01", "0.0002005370368365"),
                ("2023-03-01", "0.000023592592569"),
            ],
        )
        .await;
    }
}
