use crate::{
    data_source::kinds::{
        remote::{RemoteSource, RemoteSourceWrapper},
        updateable_chart::clone::{CloneChart, CloneChartWrapper},
    },
    utils::sql_with_range_filter_opt,
    Chart, DateValueString, Named,
};

use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

pub struct AverageGasLimitRemote;

impl RemoteSource for AverageGasLimitRemote {
    type Point = DateValueString;

    fn get_query(range: Option<std::ops::RangeInclusive<DateTimeUtc>>) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT 
                    DATE(blocks.timestamp) as date,
                    ROUND(AVG(blocks.gas_limit))::TEXT as value
                FROM blocks
                WHERE
                    blocks.timestamp != to_timestamp(0) AND
                    blocks.consensus = true {filter}
                GROUP BY date
            "#,
            [],
            "blocks.timestamp",
            range
        )
    }
}

pub struct AverageGasLimitInner;

impl Named for AverageGasLimitInner {
    const NAME: &'static str = "averageGasLimit";
}

impl Chart for AverageGasLimitInner {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

impl CloneChart for AverageGasLimitInner {
    type Dependency = RemoteSourceWrapper<AverageGasLimitRemote>;
}

pub type AverageGasLimit = CloneChartWrapper<AverageGasLimitInner>;

#[cfg(test)]
mod tests {
    use super::AverageGasLimit;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_gas_limit() {
        simple_test_chart::<AverageGasLimit>(
            "update_average_gas_limit",
            vec![
                ("2022-11-09", "12500000"),
                ("2022-11-10", "12500000"),
                ("2022-11-11", "30000000"),
                ("2022-11-12", "30000000"),
                ("2022-12-01", "30000000"),
                ("2023-01-01", "30000000"),
                ("2023-02-01", "30000000"),
                ("2023-03-01", "30000000"),
            ],
        )
        .await;
    }
}
