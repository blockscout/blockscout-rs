use crate::data_source::kinds::updateable_chart::cumulative::CumulativeChartWrapper;

/// Items in this module are not intended to be used outside. They are only public
/// since the actual public type is just an alias (to wrapper).
///
/// I.e. use [`super`]'s types.
pub mod _inner {
    use crate::{
        charts::db_interaction::types::DateValueDecimal,
        data_source::kinds::{
            adapter::{SourceAdapter, SourceAdapterWrapper},
            remote::{RemoteSource, RemoteSourceWrapper},
            updateable_chart::cumulative::CumulativeChart,
        },
        utils::sql_with_range_filter_opt,
        Chart, MissingDatePolicy, Named, UpdateError,
    };
    use entity::sea_orm_active_enums::ChartType;
    use sea_orm::{prelude::*, DbBackend, Statement};

    pub struct GasUsedPartialRemote;

    impl RemoteSource for GasUsedPartialRemote {
        type Point = DateValueDecimal;

        fn get_query(range: Option<std::ops::RangeInclusive<DateTimeUtc>>) -> Statement {
            sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                    SELECT 
                        DATE(blocks.timestamp) as date, 
                        (sum(sum(blocks.gas_used)) OVER (ORDER BY date(blocks.timestamp))) AS value
                    FROM blocks
                    WHERE 
                        blocks.timestamp != to_timestamp(0) AND 
                        blocks.consensus = true {filter}
                    GROUP BY date(blocks.timestamp)
                    ORDER BY date;
            "#,
                [],
                "blocks.timestamp",
                range
            )
        }
    }

    pub type GasUsedPartial = RemoteSourceWrapper<GasUsedPartialRemote>;

    pub struct NewGasUsedRemote;

    impl SourceAdapter for NewGasUsedRemote {
        type InnerSource = GasUsedPartial;
        type Output = Vec<DateValueDecimal>;

        fn function(
            inner_data: Vec<DateValueDecimal>,
        ) -> Result<Vec<DateValueDecimal>, UpdateError> {
            Ok(inner_data
                .into_iter()
                .scan(Decimal::ZERO, |state, mut next| {
                    let next_diff = next.value.saturating_sub(*state);
                    *state = next.value;
                    next.value = next_diff;
                    Some(next)
                })
                .collect())
        }
    }

    pub struct GasUsedGrowthInner;

    impl Named for GasUsedGrowthInner {
        const NAME: &'static str = "gasUsedGrowth";
    }

    impl Chart for GasUsedGrowthInner {
        fn chart_type() -> ChartType {
            ChartType::Line
        }
        fn missing_date_policy() -> MissingDatePolicy {
            MissingDatePolicy::FillPrevious
        }
    }

    impl CumulativeChart for GasUsedGrowthInner {
        type DeltaChart = SourceAdapterWrapper<NewGasUsedRemote>;
        type DeltaChartPoint = DateValueDecimal;
    }
}

pub type GasUsedGrowth = CumulativeChartWrapper<_inner::GasUsedGrowthInner>;

#[cfg(test)]
mod tests {
    use super::GasUsedGrowth;
    use crate::tests::simple_test::{ranged_test_chart, simple_test_chart};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_gas_used_growth() {
        simple_test_chart::<GasUsedGrowth>(
            "update_gas_used_growth",
            vec![
                ("2022-11-09", "10000"),
                ("2022-11-10", "91780"),
                ("2022-11-11", "221640"),
                ("2022-11-12", "250680"),
                ("2022-12-01", "288350"),
                ("2023-01-01", "334650"),
                ("2023-02-01", "389580"),
                ("2023-03-01", "403140"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn ranged_update_gas_used_growth() {
        let value_2022_11_12 = "250680";
        ranged_test_chart::<GasUsedGrowth>(
            "ranged_update_gas_used_growth",
            vec![("2022-11-20", value_2022_11_12), ("2022-12-01", "288350")],
            "2022-11-20".parse().unwrap(),
            "2022-12-01".parse().unwrap(),
            None,
        )
        .await;
    }
}
