use std::ops::RangeInclusive;

use crate::{
    charts::db_interaction::types::DateValueDouble,
    data_source::kinds::{
        adapter::{ToStringAdapter, ToStringAdapterWrapper},
        remote::{RemoteSource, RemoteSourceWrapper},
        updateable_chart::batch::clone::{CloneChart, CloneChartWrapper},
    },
    Chart, Named,
};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

const ETH: i64 = 1_000_000_000_000_000_000;

pub struct NativeCoinSupplyRemote;

impl RemoteSource for NativeCoinSupplyRemote {
    type Point = DateValueDouble;
    fn get_query(range: Option<RangeInclusive<DateTimeUtc>>) -> Statement {
        // todo: test if breaking types in sql query breaks test
        let day_range = range.map(|r| {
            let (start, end) = r.into_inner();
            // chart is off anyway, so shouldn't be a big deal
            start.date_naive()..=end.date_naive()
        });
        // query uses date, therefore `sql_with_range_filter_opt` does not quite fit
        // (making it parameter-agnostic seems not straightforward, let's keep it as-is)
        match day_range {
            Some(range) => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r"
                    SELECT date, value FROM 
                    (
                        SELECT
                            day as date,
                            (sum(
                                CASE 
                                    WHEN address_hash = '\x0000000000000000000000000000000000000000' THEN -value
                                    ELSE value
                                END
                            ) / $1)::float AS value
                        FROM address_coin_balances_daily
                        WHERE  day != to_timestamp(0) AND
                                            day <= $3 AND
                                            day >= $2
                        GROUP BY day
                    ) as intermediate
                    WHERE value is not NULL;
                ",
                vec![ETH.into(), (*range.start()).into(), (*range.end()).into()],
            ),
            None => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r"
                    SELECT date, value FROM 
                    (
                        SELECT
                            day as date,
                            (sum(
                                CASE 
                                    WHEN address_hash = '\x0000000000000000000000000000000000000000' THEN -value
                                    ELSE value
                                END
                            ) / $1)::float AS value
                        FROM address_coin_balances_daily
                        WHERE  day != to_timestamp(0)
                        GROUP BY day
                    ) as intermediate
                    WHERE value is not NULL;
                ",
                vec![ETH.into()],
            ),
        }
    }
}

// for some reason it was queried as double and then converted to string.
// keeping this behaviour just in case. can be removed after checking
// for correctness.
pub struct NativeCoinSupplyRemoteString;

impl ToStringAdapter for NativeCoinSupplyRemoteString {
    type InnerSource = RemoteSourceWrapper<NativeCoinSupplyRemote>;
    type ConvertFrom = DateValueDouble;
}

pub struct NativeCoinSupplyInner;

impl Named for NativeCoinSupplyInner {
    const NAME: &'static str = "nativeCoinSupply";
}

impl Chart for NativeCoinSupplyInner {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

impl CloneChart for NativeCoinSupplyInner {
    type Dependency = ToStringAdapterWrapper<NativeCoinSupplyRemoteString>;
}

pub type NativeCoinSupply = CloneChartWrapper<NativeCoinSupplyInner>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_native_coin_supply() {
        simple_test_chart::<NativeCoinSupply>(
            "update_native_coin_supply",
            vec![
                ("2022-11-09", "6666.666666666667"),
                ("2022-11-10", "6000"),
                ("2022-11-11", "5000"),
            ],
        )
        .await;
    }
}
