//! Essentially the same logic as with `NewAccounts`
//! but for account abstraction wallets.
use std::{collections::HashSet, ops::Range};

use crate::{
    ChartError, ChartKey, ChartProperties, Named,
    charts::{
        db_interaction::read::{QueryAllBlockTimestampRange, find_all_points},
        types::timespans::DateValue,
    },
    data_source::{
        UpdateContext,
        kinds::{
            data_manipulation::{
                map::{MapParseTo, MapToString, StripExt},
                resolutions::sum::SumLowerResolution,
            },
            local_db::{
                DirectVecLocalDbChartSource,
                parameters::update::batching::parameters::{
                    Batch30Weeks, Batch30Years, Batch36Months, BatchMaxDays,
                },
            },
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour, StatementFromRange},
        },
        types::IndexerMigrations,
    },
    define_and_impl_resolution_properties,
    indexing_status::{BlockscoutIndexingStatus, IndexingStatus, UserOpsIndexingStatus},
    missing_date::trim_out_of_range_sorted,
    range::{UniversalRange, data_source_query_range_to_db_statement_range},
    types::timespans::{Month, Week, Year},
    utils::sql_with_range_filter_opt,
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

pub struct NewAccountAbstractionWalletsStatement;

impl StatementFromRange for NewAccountAbstractionWalletsStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        _completed_migrations: &IndexerMigrations,
        _: &HashSet<ChartKey>,
    ) -> Statement {
        // `MIN_UTC` does not fit into postgres' timestamp. Unix epoch start should be enough
        let min_timestamp = DateTime::<Utc>::UNIX_EPOCH;
        // All transactions from the beginning must be considered to calculate new wallets correctly.
        // E.g. if a wallet was first active both before `range.start()` and within the range,
        // we don't want to count it within the range (as it's not a *new* wallet).
        let range = range.map(|r| (min_timestamp..r.end));

        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT "first_user_op"."date" AS "date",
                    COUNT(*)::TEXT AS "value"
                FROM
                    (SELECT DISTINCT ON ("sender") CAST("blocks"."timestamp" AS date) AS "date"
                    FROM "user_operations"
                    INNER JOIN "blocks" ON "user_operations"."block_hash" = "blocks"."hash"
                    WHERE "blocks"."consensus" = TRUE
                        AND "blocks"."timestamp" != to_timestamp(0) {filter}
                    ORDER BY "user_operations"."sender" ASC,
                            "blocks"."timestamp" ASC
                ) AS "first_user_op"
                GROUP BY "first_user_op"."date"
            "#,
            [],
            "\"blocks\".\"timestamp\"",
            range
        )
    }
}

pub struct NewAccountAbstractionWalletsQueryBehaviour;

impl RemoteQueryBehaviour for NewAccountAbstractionWalletsQueryBehaviour {
    type Output = Vec<DateValue<String>>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Vec<DateValue<String>>, ChartError> {
        let statement_range =
            data_source_query_range_to_db_statement_range::<QueryAllBlockTimestampRange>(cx, range)
                .await?;
        let statement = NewAccountAbstractionWalletsStatement::get_statement(
            statement_range.clone(),
            &cx.indexer_applied_migrations,
            &cx.enabled_update_charts_recursive,
        );
        let mut data = find_all_points::<DateValue<String>>(cx, statement).await?;
        if let Some(range) = statement_range {
            let range = range.start.date_naive()..=range.end.date_naive();
            trim_out_of_range_sorted(&mut data, range);
        }
        Ok(data)
    }
}

/// Note:  The intended strategy is to update whole range at once, even
/// though the implementation allows batching. The batching was done
/// to simplify interface of the data source.
///
/// Thus, use max batch size in the `DirectVecLocalDbChartSource` for it.
pub type NewAccountAbstractionWalletsRemote =
    RemoteDatabaseSource<NewAccountAbstractionWalletsQueryBehaviour>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newAccountAbstractionWallets".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
    fn indexing_status_requirement() -> IndexingStatus {
        IndexingStatus {
            blockscout: BlockscoutIndexingStatus::BlocksIndexed,
            user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
        }
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

pub type NewAccountAbstractionWallets =
    DirectVecLocalDbChartSource<NewAccountAbstractionWalletsRemote, BatchMaxDays, Properties>;
pub type NewAccountAbstractionWalletsInt = MapParseTo<StripExt<NewAccountAbstractionWallets>, i64>;
pub type NewAccountAbstractionWalletsWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewAccountAbstractionWalletsInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NewAccountAbstractionWalletsMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewAccountAbstractionWalletsInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NewAccountAbstractionWalletsMonthlyInt =
    MapParseTo<StripExt<NewAccountAbstractionWalletsMonthly>, i64>;
pub type NewAccountAbstractionWalletsYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewAccountAbstractionWalletsMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_account_abstraction_wallets() {
        simple_test_chart::<NewAccountAbstractionWallets>(
            "update_new_account_abstraction_wallets",
            vec![("2022-11-09", "1")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_account_abstraction_wallets_weekly() {
        simple_test_chart::<NewAccountAbstractionWalletsWeekly>(
            "update_new_account_abstraction_wallets_weekly",
            vec![("2022-11-07", "1")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_account_abstraction_wallets_monthly() {
        simple_test_chart::<NewAccountAbstractionWalletsMonthly>(
            "update_new_account_abstraction_wallets_monthly",
            vec![("2022-11-01", "1")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_account_abstraction_wallets_yearly() {
        simple_test_chart::<NewAccountAbstractionWalletsYearly>(
            "update_new_account_abstraction_wallets_yearly",
            vec![("2022-01-01", "1")],
        )
        .await;
    }
}
