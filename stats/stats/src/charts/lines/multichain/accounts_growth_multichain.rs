use std::ops::Range;

use crate::{chart_prelude::*, utils::sql_with_multichain_filter_opt};

pub struct AccountsGrowthMultichainStatement;
impl_db_choice!(AccountsGrowthMultichainStatement, UsePrimaryDB);

impl StatementFromRange for AccountsGrowthMultichainStatement {
    fn get_statement_with_context(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTime<Utc>>>,
    ) -> Statement {
        let from_timestamp = range.as_ref().map(|r| r.start).unwrap_or_else(Utc::now);
        let to_timestamp = range.as_ref().map(|r| r.end).unwrap_or_else(Utc::now);

        sql_with_multichain_filter_opt!(
            DbBackend::Postgres,
            r#"
            WITH filtered_dates AS (
                    SELECT DISTINCT c.date
                    FROM counters_global_imported c
                    WHERE c.date BETWEEN $1::date AND $2::date
                ),
                latest_per_chain AS (
                    SELECT DISTINCT ON (c.chain_id, d.date)
                        d.date,
                        c.chain_id,
                        c.total_addresses_number
                    FROM filtered_dates d
                    JOIN counters_global_imported c
                        ON c.date <= d.date
                    AND c.total_addresses_number IS NOT NULL
                    {multichain_filter}
                    ORDER BY c.chain_id, d.date, c.date DESC
                )
                SELECT
                    l.date,
                    SUM(l.total_addresses_number)::TEXT AS value
                FROM latest_per_chain l
                GROUP BY l.date
                ORDER BY l.date;
            "#,
            [from_timestamp.into(), to_timestamp.into()],
            "c.chain_id",
            cx.multichain_filter,
        )
    }
}

pub type AccountsGrowthMultichainRemote = RemoteDatabaseSource<
    PullAllWithAndSort<
        AccountsGrowthMultichainStatement,
        NaiveDate,
        String,
        QueryFullIndexerTimestampRange,
    >,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "accountsGrowthMultichain".into()
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

pub type AccountsGrowthMultichain =
    DirectVecLocalDbChartSource<AccountsGrowthMultichainRemote, Batch30Days, Properties>;
pub type AccountsGrowthMultichainInt = MapParseTo<StripExt<AccountsGrowthMultichain>, i64>;
pub type AccountsGrowthMultichainWeekly = DirectVecLocalDbChartSource<
    MapToString<LastValueLowerResolution<AccountsGrowthMultichainInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type AccountsGrowthMultichainMonthly = DirectVecLocalDbChartSource<
    MapToString<LastValueLowerResolution<AccountsGrowthMultichainInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type AccountsGrowthMultichainMonthlyInt =
    MapParseTo<StripExt<AccountsGrowthMultichainMonthly>, i64>;
pub type AccountsGrowthMultichainYearly = DirectVecLocalDbChartSource<
    MapToString<LastValueLowerResolution<AccountsGrowthMultichainMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart_multichain;

    // counters_global_imported mock data:
    // (date, chain_id, daily_transactions_number, total_transactions_number, total_addresses_number)
    // ("2023-02-04", 1, 10, 46, 170),
    // ("2023-02-04", 2, 20, 55, 300),
    // ("2023-02-04", 3, 30, 109, 450),

    // ("2023-02-03", 1, 4, 36, 160),
    // ("2023-02-03", 2, 7, 35, 290),
    // ("2023-02-03", 3, 38, 79, 422),

    // ("2023-02-02", 1, 18, 32, 155),
    // ("2023-02-02", 2, 3, 28, 250),
    // ("2023-02-02", 3, 4, 41, 420),

    // ("2023-01-01", 1, 3, 14, 150),
    // ("2023-01-01", 2, 3, 25, 250),
    // ("2023-01-01", 3, 4, 37, 350),

    // ("2022-12-28", 1, 11, 11, 111),
    // ("2022-12-28", 2, 22, 22, 222),
    // ("2022-12-28", 3, 33, 33, 333),

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_accounts_growth_multichain() {
        simple_test_chart_multichain::<AccountsGrowthMultichain>(
            "update_accounts_growth_multichain",
            vec![
                ("2022-12-28", "666"),
                ("2023-01-01", "750"),
                ("2023-02-02", "825"),
                ("2023-02-03", "872"),
                ("2023-02-04", "920"),
            ],
            None,
        )
        .await;

        simple_test_chart_multichain::<AccountsGrowthMultichain>(
            "update_accounts_growth_multichain",
            vec![
                ("2022-12-28", "444"),
                ("2023-01-01", "500"),
                ("2023-02-02", "575"),
                ("2023-02-03", "582"),
                ("2023-02-04", "620"),
            ],
            Some(vec![1, 3]),
        )
        .await;

        simple_test_chart_multichain::<AccountsGrowthMultichain>(
            "update_accounts_growth_multichain",
            vec![
                ("2022-12-28", "666"),
                ("2023-01-01", "750"),
                ("2023-02-02", "825"),
                ("2023-02-03", "872"),
                ("2023-02-04", "920"),
            ],
            Some(vec![]),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_accounts_growth_multichain_weekly() {
        simple_test_chart_multichain::<AccountsGrowthMultichainWeekly>(
            "update_accounts_growth_multichain_weekly",
            vec![("2022-12-26", "750"), ("2023-01-30", "920")],
            None,
        )
        .await;

        simple_test_chart_multichain::<AccountsGrowthMultichainWeekly>(
            "update_accounts_growth_multichain_weekly",
            vec![("2022-12-26", "150"), ("2023-01-30", "170")],
            Some(vec![1]),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_accounts_growth_multichain_monthly() {
        simple_test_chart_multichain::<AccountsGrowthMultichainMonthly>(
            "update_accounts_growth_multichain_monthly",
            vec![
                ("2022-12-01", "666"),
                ("2023-01-01", "750"),
                ("2023-02-01", "920"),
            ],
            None,
        )
        .await;

        simple_test_chart_multichain::<AccountsGrowthMultichainMonthly>(
            "update_accounts_growth_multichain_monthly",
            vec![
                ("2022-12-01", "333"),
                ("2023-01-01", "350"),
                ("2023-02-01", "450"),
            ],
            Some(vec![3]),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_accounts_growth_multichain_yearly() {
        simple_test_chart_multichain::<AccountsGrowthMultichainYearly>(
            "update_accounts_growth_multichain_yearly",
            vec![("2022-01-01", "666"), ("2023-01-01", "920")],
            None,
        )
        .await;

        simple_test_chart_multichain::<AccountsGrowthMultichainYearly>(
            "update_accounts_growth_multichain_yearly",
            vec![("2022-01-01", "222"), ("2023-01-01", "300")],
            Some(vec![2]),
        )
        .await;
    }
}
