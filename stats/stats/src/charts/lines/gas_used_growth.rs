use std::ops::Range;

use crate::{
    data_source::kinds::{
        data_manipulation::map::{Map, MapFunction},
        local_db::DailyCumulativeLocalDbChartSource,
        remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
    },
    types::DateValue,
    utils::sql_with_range_filter_opt,
    ChartProperties, MissingDatePolicy, Named, UpdateError,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

pub struct GasUsedPartialStatement;

impl StatementFromRange for GasUsedPartialStatement {
    fn get_statement(range: Option<Range<DateTimeUtc>>) -> Statement {
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

pub type GasUsedPartialRemote =
    RemoteDatabaseSource<PullAllWithAndSort<GasUsedPartialStatement, NaiveDate, Decimal>>;

pub struct IncrementsFromPartialSum;

impl MapFunction<Vec<DateValue<Decimal>>> for IncrementsFromPartialSum {
    type Output = Vec<DateValue<Decimal>>;
    fn function(inner_data: Vec<DateValue<Decimal>>) -> Result<Self::Output, UpdateError> {
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

pub type NewGasUsedRemote = Map<GasUsedPartialRemote, IncrementsFromPartialSum>;

pub struct GasUsedGrowthProperties;

impl Named for GasUsedGrowthProperties {
    fn name() -> String {
                "gasUsedGrowth".into()
            }
}

impl ChartProperties for GasUsedGrowthProperties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
}

pub type GasUsedGrowth =
    DailyCumulativeLocalDbChartSource<NewGasUsedRemote, GasUsedGrowthProperties>;

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
