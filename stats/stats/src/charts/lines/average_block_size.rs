use crate::data_source::kinds::updateable_chart::clone::CloneChartWrapper;

mod _inner {
    use crate::{
        data_source::kinds::{
            remote::{RemoteSource, RemoteSourceWrapper},
            updateable_chart::clone::CloneChart,
        },
        utils::sql_with_range_filter_opt,
        Chart, DateValueString, Named,
    };
    use entity::sea_orm_active_enums::ChartType;
    use sea_orm::{prelude::*, DbBackend, Statement};

    pub struct AverageBlockSizeRemote;

    impl RemoteSource for AverageBlockSizeRemote {
        type Point = DateValueString;

        fn get_query(range: Option<std::ops::RangeInclusive<DateTimeUtc>>) -> Statement {
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

    pub struct AverageBlockSizeInner;

    impl Named for AverageBlockSizeInner {
        const NAME: &'static str = "averageBlockSize";
    }

    impl Chart for AverageBlockSizeInner {
        fn chart_type() -> ChartType {
            ChartType::Line
        }
    }

    impl CloneChart for AverageBlockSizeInner {
        type Dependency = RemoteSourceWrapper<AverageBlockSizeRemote>;
    }
}

pub type AverageBlockSize = CloneChartWrapper<_inner::AverageBlockSizeInner>;

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
