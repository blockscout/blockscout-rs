use std::ops::Range;

use crate::{
    data_source::kinds::{
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

pub struct AverageGasLimitStatement;

impl StatementFromRange for AverageGasLimitStatement {
    fn get_statement(range: Option<Range<DateTimeUtc>>) -> Statement {
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

pub type AverageGasLimitRemote =
    RemoteDatabaseSource<PullAllWithAndSort<AverageGasLimitStatement, NaiveDate, String>>;

pub struct AverageGasLimitProperties;

impl Named for AverageGasLimitProperties {
    const NAME: &'static str = "averageGasLimit";
}

impl ChartProperties for AverageGasLimitProperties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

pub type AverageGasLimit =
    DirectVecLocalDbChartSource<AverageGasLimitRemote, Batch30Days, AverageGasLimitProperties>;

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
