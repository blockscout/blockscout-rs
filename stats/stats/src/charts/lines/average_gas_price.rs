use crate::data_source::kinds::updateable_chart::clone::CloneChartWrapper;

mod _inner {
    use crate::{
        charts::db_interaction::types::DateValueDouble,
        data_source::kinds::{
            adapter::to_string::MapToString,
            remote::{RemoteSource, RemoteSourceWrapper},
            updateable_chart::clone::CloneChart,
        },
        utils::sql_with_range_filter_opt,
        Chart, Named,
    };
    use entity::sea_orm_active_enums::ChartType;
    use sea_orm::{prelude::*, DbBackend, Statement};

    const GWEI: i64 = 1_000_000_000;

    pub struct AverageGasPriceRemote;

    impl RemoteSource for AverageGasPriceRemote {
        type Point = DateValueDouble;

        fn get_query(range: Option<std::ops::RangeInclusive<DateTimeUtc>>) -> Statement {
            sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                    SELECT
                        blocks.timestamp::date as date,
                        (AVG(
                            COALESCE(
                                transactions.gas_price,
                                blocks.base_fee_per_gas + LEAST(
                                    transactions.max_priority_fee_per_gas,
                                    transactions.max_fee_per_gas - blocks.base_fee_per_gas
                                )
                            )
                        ) / $1)::float as value
                    FROM transactions
                    JOIN blocks ON transactions.block_hash = blocks.hash
                    WHERE 
                        blocks.timestamp != to_timestamp(0) AND
                        blocks.consensus = true {filter}
                    GROUP BY date
                "#,
                [GWEI.into()],
                "blocks.timestamp",
                range,
            )
        }
    }

    pub type AverageGasPriceRemoteString = MapToString<RemoteSourceWrapper<AverageGasPriceRemote>>;

    pub struct AverageGasPriceInner;

    impl Named for AverageGasPriceInner {
        const NAME: &'static str = "averageGasPrice";
    }

    impl Chart for AverageGasPriceInner {
        fn chart_type() -> ChartType {
            ChartType::Line
        }
    }

    impl CloneChart for AverageGasPriceInner {
        type Dependency = AverageGasPriceRemoteString;
    }
}

pub type AverageGasPrice = CloneChartWrapper<_inner::AverageGasPriceInner>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_gas_price() {
        simple_test_chart::<AverageGasPrice>(
            "update_average_gas_price",
            vec![
                ("2022-11-09", "0.4493827156"),
                ("2022-11-10", "1.96604938075"),
                ("2022-11-11", "3.2901234535"),
                ("2022-11-12", "5.8419753028"),
                ("2022-12-01", "6.5160493762"),
                ("2023-01-01", "1.123456789"),
                ("2023-02-01", "9.5493827065"),
                ("2023-03-01", "1.123456789"),
            ],
        )
        .await;
    }
}
