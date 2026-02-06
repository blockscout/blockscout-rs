//! Cumulative total number of interchain messages sent (by associated message: src_tx_hash set).

use super::new_messages_sent_interchain::NewMessagesSentInterchainInt;
use crate::chart_prelude::*;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "messagesGrowthSentInterchain".into()
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

pub type MessagesGrowthSentInterchain =
    DailyCumulativeLocalDbChartSource<NewMessagesSentInterchainInt, Properties>;
type MessagesGrowthSentInterchainS = StripExt<MessagesGrowthSentInterchain>;

pub type MessagesGrowthSentInterchainWeekly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<MessagesGrowthSentInterchainS, Week>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type MessagesGrowthSentInterchainMonthly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<MessagesGrowthSentInterchainS, Month>,
    Batch36Months,
    MonthlyProperties,
>;
type MessagesGrowthSentInterchainMonthlyS = StripExt<MessagesGrowthSentInterchainMonthly>;
pub type MessagesGrowthSentInterchainYearly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<MessagesGrowthSentInterchainMonthlyS, Year>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart_interchain;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_messages_growth_sent_interchain() {
        simple_test_chart_interchain::<MessagesGrowthSentInterchain>(
            "update_messages_growth_sent_interchain",
            vec![
                ("2022-12-20", "1"),
                ("2022-12-21", "3"),
                ("2022-12-26", "4"),
                ("2022-12-27", "5"),
                ("2023-01-01", "7"),
                ("2023-01-04", "8"),
                ("2023-01-10", "10"),
                ("2023-01-20", "11"),
                ("2023-01-21", "12"),
                ("2023-02-01", "14"),
                ("2023-02-10", "15"),
            ],
            None,
        )
        .await;

        simple_test_chart_interchain::<MessagesGrowthSentInterchain>(
            "update_messages_growth_sent_interchain_primary_1",
            vec![
                ("2022-12-20", "1"),
                ("2022-12-21", "2"),
                ("2022-12-26", "3"),
                ("2023-01-01", "5"),
                ("2023-01-04", "6"),
                ("2023-01-10", "7"),
                ("2023-01-20", "8"),
                ("2023-02-01", "10"),
                ("2023-02-10", "11"),
            ],
            Some(1),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_messages_growth_sent_interchain_weekly() {
        simple_test_chart_interchain::<MessagesGrowthSentInterchainWeekly>(
            "update_messages_growth_sent_interchain_weekly",
            vec![
                ("2022-12-19", "3"),
                ("2022-12-26", "7"),
                ("2023-01-02", "8"),
                ("2023-01-09", "10"),
                ("2023-01-16", "12"),
                ("2023-01-30", "14"),
                ("2023-02-06", "15"),
            ],
            None,
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_messages_growth_sent_interchain_monthly() {
        simple_test_chart_interchain::<MessagesGrowthSentInterchainMonthly>(
            "update_messages_growth_sent_interchain_monthly",
            vec![
                ("2022-12-01", "5"),
                ("2023-01-01", "12"),
                ("2023-02-01", "15"),
            ],
            None,
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_messages_growth_sent_interchain_yearly() {
        simple_test_chart_interchain::<MessagesGrowthSentInterchainYearly>(
            "update_messages_growth_sent_interchain_yearly",
            vec![("2022-01-01", "5"), ("2023-01-01", "15")],
            None,
        )
        .await;
    }
}
