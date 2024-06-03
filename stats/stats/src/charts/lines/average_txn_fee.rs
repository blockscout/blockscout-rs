//! Average fee per transaction
use crate::{
    data_source::kinds::{
        remote::{RemoteSource, RemoteSourceWrapper},
        updateable_chart::batch::clone::{CloneChart, CloneChartWrapper},
    },
    utils::sql_with_range_filter_opt,
    Chart, DateValueString, Named,
};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

const ETHER: i64 = i64::pow(10, 18);

pub struct AverageTxnFeeRemote;

impl RemoteSource for AverageTxnFeeRemote {
    type Point = DateValueString;

    fn get_query(range: Option<std::ops::RangeInclusive<DateTimeUtc>>) -> Statement {
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

pub struct AverageTxnFeeInner;

impl Named for AverageTxnFeeInner {
    const NAME: &'static str = "averageTxnFee";
}

impl Chart for AverageTxnFeeInner {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

impl CloneChart for AverageTxnFeeInner {
    type Dependency = RemoteSourceWrapper<AverageTxnFeeRemote>;
}

pub type AverageTxnFee = CloneChartWrapper<AverageTxnFeeInner>;

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
