use std::{collections::HashSet, ops::Range};

use crate::{
    charts::{
        db_interaction::read::{find_all_points, QueryAllBlockTimestampRange},
        types::timespans::DateValue,
    },
    data_source::{
        kinds::{
            data_manipulation::{
                map::{MapParseTo, MapToString, StripExt},
                resolutions::sum::SumLowerResolution,
            },
            local_db::{
                parameters::update::batching::parameters::{
                    Batch30Weeks, Batch30Years, Batch36Months, BatchMaxDays,
                },
                DirectVecLocalDbChartSource,
            },
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour, StatementFromRange},
        },
        types::BlockscoutMigrations,
        UpdateContext,
    },
    define_and_impl_resolution_properties,
    indexing_status::{BlockscoutIndexingStatus, UserOpsIndexingStatus},
    missing_date::trim_out_of_range_sorted,
    range::{data_source_query_range_to_db_statement_range, UniversalRange},
    types::timespans::{Month, Week, Year},
    ChartError, ChartKey, ChartProperties, IndexingStatus, Named,
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

pub struct NewBuilderAccountsStatement;

impl StatementFromRange for NewBuilderAccountsStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        completed_migrations: &BlockscoutMigrations,
        _enabled_update_charts_recursive: &HashSet<ChartKey>,
    ) -> Statement {
        if !completed_migrations.denormalization {
            // the query is very complex + the chart is disabled by default
            // + it is expected to be used in 1-2 networks, therefore
            // won't bother to support pre-migration (that was done long before)
            tracing::error!("builder accounts charts are not supported without denormalized database; the chart will show zeroes");

            return Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT 
                    now()::date as date,
                    0::TEXT as value
                "#,
                [],
            );
        }

        // `MIN_UTC` does not fit into postgres' timestamp. Unix epoch start should be enough
        let min_timestamp = DateTime::<Utc>::UNIX_EPOCH;
        // All transactions from the beginning must be considered to calculate new builder accounts correctly.
        // E.g. if account was first active both before `range.start()` and within the range,
        // we don't want to count it within the range (as it's not a *new* account).
        let range = range.map(|r| (min_timestamp..r.end));

        let mut args = vec![];
        let (tx_filter, new_args) = crate::utils::produce_filter_and_values(
            range.clone(),
            "t.block_timestamp",
            args.len() + 1,
        );
        args.extend(new_args);
        let (block_filter, new_args) = crate::utils::produce_filter_and_values(
            range.clone(),
            "tt.block_timestamp",
            args.len() + 1,
        );
        args.extend(new_args);
        let sql = format!(
            r#"
                SELECT 
                    first_tx.date as date,
                    count(*)::TEXT as value
                FROM (
                    SELECT 
                        DISTINCT ON (txns_plus_internal_txns.from_address_hash)
                        txns_plus_internal_txns.day as date
                    FROM (
                        SELECT
                            t.from_address_hash AS from_address_hash,
                            t.block_timestamp::date AS day
                        FROM transactions t
                        WHERE
                            t.created_contract_address_hash NOTNULL AND
                            t.block_consensus = TRUE AND
                            t.block_timestamp != to_timestamp(0) {tx_filter}
                        UNION
                        SELECT
                            tt.from_address_hash AS from_address_hash,
                            tt.block_timestamp::date AS day
                        FROM internal_transactions it
                            JOIN transactions tt ON tt.hash = it.transaction_hash
                        WHERE
                            it.created_contract_address_hash NOTNULL AND
                            tt.block_consensus = TRUE AND
                            tt.block_timestamp != to_timestamp(0) {block_filter}
                    ) txns_plus_internal_txns
                    ORDER BY txns_plus_internal_txns.from_address_hash, txns_plus_internal_txns.day
                ) first_tx
                GROUP BY first_tx.date;
            "#,
        );
        Statement::from_sql_and_values(DbBackend::Postgres, sql, args)
    }
}

pub struct NewBuilderAccountsQueryBehaviour;

impl RemoteQueryBehaviour for NewBuilderAccountsQueryBehaviour {
    type Output = Vec<DateValue<String>>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Vec<DateValue<String>>, ChartError> {
        let statement_range =
            data_source_query_range_to_db_statement_range::<QueryAllBlockTimestampRange>(cx, range)
                .await?;
        let statement = NewBuilderAccountsStatement::get_statement(
            statement_range.clone(),
            &cx.blockscout_applied_migrations,
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
pub type NewBuilderAccountsRemote = RemoteDatabaseSource<NewBuilderAccountsQueryBehaviour>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newBuilderAccounts".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }

    fn indexing_status_requirement() -> IndexingStatus {
        IndexingStatus {
            blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
            // don't need user ops at all
            user_ops: UserOpsIndexingStatus::IndexingPastOperations,
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

pub type NewBuilderAccounts =
    DirectVecLocalDbChartSource<NewBuilderAccountsRemote, BatchMaxDays, Properties>;
pub type NewBuilderAccountsInt = MapParseTo<StripExt<NewBuilderAccounts>, i64>;
pub type NewBuilderAccountsWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewBuilderAccountsInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NewBuilderAccountsMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewBuilderAccountsInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NewBuilderAccountsMonthlyInt = MapParseTo<StripExt<NewBuilderAccountsMonthly>, i64>;
pub type NewBuilderAccountsYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewBuilderAccountsMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_builder_accounts() {
        simple_test_chart::<NewBuilderAccounts>(
            "update_new_builder_accounts",
            // internal txns case is not actually tested,
            // but it's not trivial to add, so leaving it as is;
            // it seems to work while testing queries in prod :)
            vec![
                ("2022-11-09", "1"),
                ("2022-11-10", "3"),
                ("2022-11-11", "4"),
            ],
        )
        .await;
    }
}
