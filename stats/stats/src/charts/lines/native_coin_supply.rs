use std::ops::Range;

use crate::{
    charts::types::DateValueDouble,
    data_source::kinds::{
        data_manipulation::map::MapToString,
        local_db::DirectVecLocalDbChartSource,
        remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
    },
    ChartProperties, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

const ETH: i64 = 1_000_000_000_000_000_000;

pub struct NativeCoinSupplyStatement;

impl StatementFromRange for NativeCoinSupplyStatement {
    fn get_statement(range: Option<Range<DateTimeUtc>>) -> Statement {
        let day_range: Option<Range<NaiveDate>> = range.map(|r| {
            let Range { start, end } = r;
            // chart is off anyway, so shouldn't be a big deal
            start.date_naive()..end.date_naive()
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
                vec![ETH.into(), range.start.into(), range.end.into()],
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

// query returns float value
pub type NativeCoinSupplyRemote =
    RemoteDatabaseSource<PullAllWithAndSort<NativeCoinSupplyStatement, DateValueDouble>>;

pub type NativeCoinSupplyRemoteString = MapToString<NativeCoinSupplyRemote>;

pub struct NativeCoinSupplyProperties;

impl Named for NativeCoinSupplyProperties {
    const NAME: &'static str = "nativeCoinSupply";
}

impl ChartProperties for NativeCoinSupplyProperties {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

pub type NativeCoinSupply =
    DirectVecLocalDbChartSource<NativeCoinSupplyRemoteString, NativeCoinSupplyProperties>;

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
