// SPDX-License-Identifier: LicenseRef-Blockscout

//! Cumulative interchain messages received, within the configured interchain
//! slice.
//!
//! Has no SQL of its own: it is the running total of
//! [`super::new_messages_received_interchain`]'s stored rows, and therefore
//! inherits that chart's read filter transitively. `interchain_filter_coverage`
//! lists it in `DERIVED_WITHOUT_OWN_STATEMENT` on exactly that basis.

use super::new_messages_received_interchain::NewMessagesReceivedInterchainInt;
use crate::chart_prelude::*;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "messagesGrowthReceivedInterchain".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
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

pub type MessagesGrowthReceivedInterchain =
    DailyCumulativeLocalDbChartSource<NewMessagesReceivedInterchainInt, Properties>;
type MessagesGrowthReceivedInterchainS = StripExt<MessagesGrowthReceivedInterchain>;

pub type MessagesGrowthReceivedInterchainWeekly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<MessagesGrowthReceivedInterchainS, Week>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type MessagesGrowthReceivedInterchainMonthly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<MessagesGrowthReceivedInterchainS, Month>,
    Batch36Months,
    MonthlyProperties,
>;
type MessagesGrowthReceivedInterchainMonthlyS = StripExt<MessagesGrowthReceivedInterchainMonthly>;
pub type MessagesGrowthReceivedInterchainYearly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<MessagesGrowthReceivedInterchainMonthlyS, Year>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        charts::db_interaction::filters::interchain::InterchainFilter,
        tests::{
            mock_interchain::test_interchain_home_chain_filter,
            simple_test::simple_test_chart_interchain,
        },
    };

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_messages_growth_received_interchain() {
        simple_test_chart_interchain::<MessagesGrowthReceivedInterchain>(
            "update_messages_growth_received_interchain",
            vec![
                ("2022-12-20", "1"),
                ("2022-12-21", "2"),
                ("2022-12-23", "3"),
                ("2022-12-27", "5"),
                ("2023-01-01", "6"),
                ("2023-01-02", "7"),
                ("2023-01-04", "8"),
                ("2023-01-11", "9"),
                ("2023-01-21", "11"),
                ("2023-02-01", "12"),
                ("2023-02-05", "13"),
                ("2023-02-06", "14"),
                ("2023-02-08", "15"),
                ("2023-02-09", "16"),
            ],
            InterchainFilter::default(),
        )
        .await;

        simple_test_chart_interchain::<MessagesGrowthReceivedInterchain>(
            "update_messages_growth_received_interchain_primary_1",
            vec![
                ("2022-12-23", "1"),
                ("2022-12-27", "2"),
                ("2023-01-02", "3"),
                ("2023-01-21", "5"),
                ("2023-02-05", "6"),
            ],
            test_interchain_home_chain_filter(1),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_messages_growth_received_interchain_weekly() {
        simple_test_chart_interchain::<MessagesGrowthReceivedInterchainWeekly>(
            "update_messages_growth_received_interchain_weekly",
            vec![
                ("2022-12-19", "3"),
                ("2022-12-26", "6"),
                ("2023-01-02", "8"),
                ("2023-01-09", "9"),
                ("2023-01-16", "11"),
                ("2023-01-30", "13"),
                ("2023-02-06", "16"),
            ],
            InterchainFilter::default(),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_messages_growth_received_interchain_monthly() {
        simple_test_chart_interchain::<MessagesGrowthReceivedInterchainMonthly>(
            "update_messages_growth_received_interchain_monthly",
            vec![
                ("2022-12-01", "5"),
                ("2023-01-01", "11"),
                ("2023-02-01", "16"),
            ],
            InterchainFilter::default(),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_messages_growth_received_interchain_yearly() {
        simple_test_chart_interchain::<MessagesGrowthReceivedInterchainYearly>(
            "update_messages_growth_received_interchain_yearly",
            vec![("2022-01-01", "5"), ("2023-01-01", "16")],
            InterchainFilter::default(),
        )
        .await;
    }
}
