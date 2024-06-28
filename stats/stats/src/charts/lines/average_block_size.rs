use std::ops::RangeInclusive;

use crate::{
    data_source::kinds::{
        local_db::DirectVecLocalDbChartSource,
        remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
    },
    utils::sql_with_range_filter_opt,
    ChartProperties, DateValueString, Named,
};

use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

pub struct AverageBlockSizeStatement;

impl StatementFromRange for AverageBlockSizeStatement {
    fn get_statement(range: Option<RangeInclusive<DateTimeUtc>>) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT
                    DATE(blocks.timestamp) as date,
                    ROUND(AVG(blocks.size))::TEXT as value
                FROM blocks
                WHERE
                    blocks.timestamp != to_timestamp(0) AND
                    consensus = true {filter}
                GROUP BY date
            "#,
            [],
            "blocks.timestamp",
            range,
        )
    }
}

pub type AverageBlockSizeRemote =
    RemoteDatabaseSource<PullAllWithAndSort<AverageBlockSizeStatement, DateValueString>>;

pub struct AverageBlockSizeProperties;

impl Named for AverageBlockSizeProperties {
    const NAME: &'static str = "averageBlockSize";
}

impl ChartProperties for AverageBlockSizeProperties {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

pub type AverageBlockSize =
    DirectVecLocalDbChartSource<AverageBlockSizeRemote, AverageBlockSizeProperties>;

#[cfg(test)]
mod tests {
    use super::AverageBlockSize;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_block_size() {
        simple_test_chart::<AverageBlockSize>(
            "update_average_block_size",
            vec![
                ("2022-11-09", "1000"),
                ("2022-11-10", "2726"),
                ("2022-11-11", "3247"),
                ("2022-11-12", "2904"),
                ("2022-12-01", "3767"),
                ("2023-01-01", "4630"),
                ("2023-02-01", "5493"),
                ("2023-03-01", "1356"),
            ],
        )
        .await;
    }
}
