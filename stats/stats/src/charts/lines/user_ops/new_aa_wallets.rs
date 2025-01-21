//! Essentially the same logic as with `NewAccounts`
//! but for account abstraction wallets.
use std::ops::Range;

use crate::{
    charts::{
        db_interaction::{read::QueryAllBlockTimestampRange, utils::datetime_range_filter},
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
    missing_date::trim_out_of_range_sorted,
    range::{data_source_query_range_to_db_statement_range, UniversalRange},
    types::timespans::{Month, Week, Year},
    ChartError, ChartProperties, Named,
};

use blockscout_db::entity::{blocks, user_operations};
use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use migration::{Alias, Asterisk, Func, IntoColumnRef, Query, SelectStatement, SimpleExpr};
use sea_orm::{
    ColumnTrait, DatabaseBackend, EntityTrait, FromQueryResult, IntoIdentity, IntoSimpleExpr,
    Order, QueryFilter, QueryOrder, QuerySelect, QueryTrait, Statement, StatementBuilder,
};

pub struct NewAccountAbstractionWalletsStatement;

impl StatementFromRange for NewAccountAbstractionWalletsStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        _completed_migrations: &BlockscoutMigrations,
    ) -> Statement {
        // `MIN_UTC` does not fit into postgres' timestamp. Unix epoch start should be enough
        let min_timestamp = DateTime::<Utc>::UNIX_EPOCH;
        // All transactions from the beginning must be considered to calculate new wallets correctly.
        // E.g. if a wallet was first active both before `range.start()` and within the range,
        // we don't want to count it within the range (as it's not a *new* wallet).
        let range = range.map(|r| (min_timestamp..r.end));

        // same as `new_accounts` but in sea-query/sea-orm form
        let date_intermediate_col = "date".into_identity();
        let mut first_user_op = user_operations::Entity::find()
            .select_only()
            .join(
                sea_orm::JoinType::InnerJoin,
                user_operations::Entity::belongs_to(blocks::Entity)
                    .from(user_operations::Column::BlockHash)
                    .to(blocks::Column::Hash)
                    .into(),
            )
            .distinct_on([user_operations::Column::Sender])
            .expr_as(
                blocks::Column::Timestamp
                    .into_simple_expr()
                    .cast_as(Alias::new("date")),
                date_intermediate_col,
            )
            .filter(blocks::Column::Consensus.eq(true))
            .filter(blocks::Column::Timestamp.ne(DateTime::UNIX_EPOCH))
            .order_by(user_operations::Column::Sender, Order::Asc)
            .order_by(blocks::Column::Timestamp, Order::Asc);
        if let Some(range) = range {
            first_user_op = datetime_range_filter(first_user_op, blocks::Column::Timestamp, &range);
        }
        let first_user_op = first_user_op.into_query();
        let first_user_op_alias = Alias::new("first_user_op");
        let date_intermediate_col = (first_user_op_alias.clone(), Alias::new("date"));

        let mut query = Query::select();
        query
            .expr_as(
                date_intermediate_col.clone().into_column_ref(),
                Alias::new("date"),
            )
            .expr_as(
                SimpleExpr::from(Func::count(Asterisk.into_column_ref()))
                    .cast_as(Alias::new("text")),
                Alias::new("value"),
            )
            .from_subquery(first_user_op, first_user_op_alias.clone())
            .add_group_by([date_intermediate_col.into_column_ref().into()]);
        <SelectStatement as StatementBuilder>::build(&query, &DatabaseBackend::Postgres)
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
        let query = NewAccountAbstractionWalletsStatement::get_statement(
            statement_range.clone(),
            &cx.blockscout_applied_migrations,
        );
        let mut data = DateValue::<String>::find_by_statement(query)
            .all(cx.blockscout)
            .await
            .map_err(ChartError::BlockscoutDB)?;
        // make sure that it's sorted
        data.sort_by_key(|d| d.timespan);
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
/// Thus, use max batch size in the dependant data sources.
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
