use crate::{
    data_source::kinds::{
        data_manipulation::{
            map::{Map, MapParseTo, MapToString, StripExt},
            resolutions::sum::SumLowerResolution,
        },
        local_db::{
            parameters::update::batching::parameters::{
                Batch30Days, Batch30Weeks, Batch30Years, Batch36Months,
            },
            DirectVecLocalDbChartSource,
        },
    },
    define_and_impl_resolution_properties,
    types::{
        new_txns::ExtractOpStackTxns,
        timespans::{Month, Week, Year},
    },
    ChartProperties, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

use super::NewTxnsCombinedRemote;

pub const ATTRIBUTES_DEPOSITED_FROM_HASH: &str = "deaddeaddeaddeaddeaddeaddeaddeaddead0001";
pub const ATTRIBUTES_DEPOSITED_TO_HASH: &str = "4200000000000000000000000000000000000015";

pub fn transactions_filter(transactions_table_name: &str) -> String {
    // https://specs.optimism.io/protocol/deposits.html#l1-attributes-deposited-transaction
    format!(
        "NOT (
            {transactions_table_name}.from_address_hash = decode('{ATTRIBUTES_DEPOSITED_FROM_HASH}', 'hex') AND
            {transactions_table_name}.to_address_hash = decode('{ATTRIBUTES_DEPOSITED_TO_HASH}', 'hex')
        )"
    )
}

pub type OpStackNewOperationalTxnsRemote = Map<NewTxnsCombinedRemote, ExtractOpStackTxns>;

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
