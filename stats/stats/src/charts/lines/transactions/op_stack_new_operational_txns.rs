use std::ops::Range;

/// Operational transactions for op stack networks.
///
/// Does not count attributes deposit transactions
/// (https://specs.optimism.io/protocol/deposits.html#l1-attributes-deposited-transaction)
use crate::{
    data_source::{
        kinds::{
            data_manipulation::{
                map::{MapParseTo, MapToString, StripExt},
                resolutions::sum::SumLowerResolution,
            },
            local_db::{
                parameters::update::batching::parameters::{
                    Batch30Days, Batch30Weeks, Batch30Years, Batch36Months,
                },
                DirectVecLocalDbChartSource,
            },
            remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
        },
        types::BlockscoutMigrations,
    },
    define_and_impl_resolution_properties,
    types::timespans::{Month, Week, Year},
    utils::sql_with_range_filter_opt,
    ChartProperties, MissingDatePolicy, Named, QueryAllBlockTimestampRange,
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

pub const ATTRIBUTES_DEPOSITED_FROM_HASH: &str = "deaddeaddeaddeaddeaddeaddeaddeaddead0001";
pub const ATTRIBUTES_DEPOSITED_TO_HASH: &str = "4200000000000000000000000000000000000015";

pub struct OpStackNewOperationalTxnsStatement;

impl StatementFromRange for OpStackNewOperationalTxnsStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        completed_migrations: &BlockscoutMigrations,
    ) -> Statement {
        if completed_migrations.denormalization {
            sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                    SELECT
                        date(t.block_timestamp) as date,
                        COUNT(*)::TEXT as value
                    FROM transactions t
                    WHERE
                        t.block_consensus = true AND
                        NOT (
                            t.from_address_hash = decode($1, 'hex') AND
                            t.to_address_hash = decode($2, 'hex')
                        ) {filter}
                    GROUP BY date;
                "#,
                [
                    ATTRIBUTES_DEPOSITED_FROM_HASH.into(),
                    ATTRIBUTES_DEPOSITED_TO_HASH.into()
                ],
                "t.block_timestamp",
                range
            )
        } else {
            sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                    SELECT
                        date(b.timestamp) as date,
                        COUNT(*)::TEXT as value
                    FROM transactions t
                    JOIN blocks       b ON t.block_hash = b.hash
                    WHERE
                        b.consensus = true AND
                        NOT (
                            t.from_address_hash = decode($1, 'hex') AND
                            t.to_address_hash = decode($2, 'hex')
                        ) {filter}
                    GROUP BY date;
                "#,
                [
                    ATTRIBUTES_DEPOSITED_FROM_HASH.into(),
                    ATTRIBUTES_DEPOSITED_TO_HASH.into()
                ],
                "b.timestamp",
                range
            )
        }
    }
}

pub type OpStackNewOperationalTxnsRemote = RemoteDatabaseSource<
    PullAllWithAndSort<
        OpStackNewOperationalTxnsStatement,
        NaiveDate,
        String,
        QueryAllBlockTimestampRange,
    >,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "opStackNewOperationalTxns".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }

    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillZero
    }
}

define_and_impl_resolution_properties!(
    define_and_impl: {
        WeeklyProperties: Week,
        MonthlyProperties: Month,
        YearlyProperties: Year,
    },
    base_impl: Properties
);

pub type OpStackNewOperationalTxns =
    DirectVecLocalDbChartSource<OpStackNewOperationalTxnsRemote, Batch30Days, Properties>;
pub type OpStackNewOperationalTxnsInt = MapParseTo<StripExt<OpStackNewOperationalTxns>, i64>;
pub type OpStackNewOperationalTxnsWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<OpStackNewOperationalTxnsInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type OpStackNewOperationalTxnsMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<OpStackNewOperationalTxnsInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type OpStackNewOperationalTxnsMonthlyInt =
    MapParseTo<StripExt<OpStackNewOperationalTxnsMonthly>, i64>;
pub type OpStackNewOperationalTxnsYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<OpStackNewOperationalTxnsMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_op_stack_new_operational_txns() {
        simple_test_chart::<OpStackNewOperationalTxns>(
            "update_op_stack_new_operational_txns",
            vec![
                ("2022-11-09", "6"),
                ("2022-11-10", "14"),
                ("2022-11-11", "16"),
                ("2022-11-12", "6"),
                ("2022-12-01", "6"),
                ("2023-01-01", "1"),
                ("2023-02-01", "5"),
                ("2023-03-01", "1"),
            ],
        )
        .await;
    }

    // Tests are kinda slow so I don't see a reason to test `SumLowerResolution` over and over again
}
