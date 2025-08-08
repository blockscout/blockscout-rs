use std::{collections::HashSet, ops::Range};

use crate::{
    ChartError, ChartKey, ChartProperties, IndexingStatus, Named,
    charts::db_interaction::read::{find_all_points, zetachain_cctx::QueryAllCctxTimetsampRange},
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
                    Batch30Days, Batch30Weeks, Batch30Years, Batch36Months,
                },
            },
            remote_db::{
                RemoteDatabaseSource, RemoteQueryBehaviour, StatementFromRange,
                query::prepare_range_query_statement,
            },
        },
        types::IndexerMigrations,
    },
    define_and_impl_resolution_properties,
    indexing_status::{IndexingStatusTrait, ZetachainCctxIndexingStatus},
    range::UniversalRange,
    types::{
        TimespanValue,
        timespans::{Month, Week, Year},
    },
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use migration::IntoColumnRef;
use sea_orm::{
    ColumnTrait, DbBackend, EntityTrait, QueryFilter, QuerySelect, QueryTrait, Statement,
    sea_query::{Alias, Asterisk, Expr, ExprTrait, Func},
};

pub struct NewZetachainCrossChainTxnsStatement;

impl StatementFromRange for NewZetachainCrossChainTxnsStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        _: &IndexerMigrations,
        _: &HashSet<ChartKey>,
    ) -> Statement {
        let date_col = "date";
        zetachain_cctx_entity::cross_chain_tx::Entity::find()
            .select_only()
            .expr_as(
                Func::cust("to_timestamp")
                    .arg(zetachain_cctx_entity::cctx_status::Column::CreatedTimestamp.into_expr())
                    .cast_as("date"),
                date_col,
            )
            .expr_as(
                Func::count(Asterisk.into_column_ref()).cast_as("TEXT"),
                "value",
            )
            .left_join(zetachain_cctx_entity::cctx_status::Entity)
            .apply_if(range, |query, range| {
                let timestamp_col =
                    zetachain_cctx_entity::cctx_status::Column::CreatedTimestamp.into_expr();
                let start = Expr::cust_with_values("extract(epoch from $1)", [range.start]);
                let end = Expr::cust_with_values("extract(epoch from $1)", [range.end]);
                query
                    .filter(timestamp_col.clone().lt(end))
                    .filter(timestamp_col.gte(start))
            })
            .group_by(Expr::col(Alias::new(date_col)))
            .build(DbBackend::Postgres)
    }
}

pub(crate) struct NewZetachainCrossChainTxnsRemoteQuery;
impl RemoteQueryBehaviour for NewZetachainCrossChainTxnsRemoteQuery {
    type Output = Vec<TimespanValue<NaiveDate, String>>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Vec<TimespanValue<NaiveDate, String>>, ChartError> {
        let statement = prepare_range_query_statement::<
            NewZetachainCrossChainTxnsStatement,
            QueryAllCctxTimetsampRange,
        >(cx, range)
        .await?;
        let Some(zeta_cctx_db) = cx.second_indexer_db else {
            return Err(ChartError::Internal("Cannot query zetachain crosschain transactions: zetachain indexer DB is not connected".to_string()));
        };
        find_all_points(zeta_cctx_db, statement).await
    }
}

pub type NewZetachainCrossChainTxnsRemote =
    RemoteDatabaseSource<NewZetachainCrossChainTxnsRemoteQuery>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newZetachainCrossChainTxns".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }

    fn indexing_status_requirement() -> IndexingStatus {
        IndexingStatus::LEAST_RESTRICTIVE
            .with_zetachain_cctx(ZetachainCctxIndexingStatus::IndexedHistoricalData)
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

pub type NewZetachainCrossChainTxns =
    DirectVecLocalDbChartSource<NewZetachainCrossChainTxnsRemote, Batch30Days, Properties>;
pub type NewZetachainCrossChainTxnsInt = MapParseTo<StripExt<NewZetachainCrossChainTxns>, i64>;
pub type NewZetachainCrossChainTxnsWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewZetachainCrossChainTxnsInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NewZetachainCrossChainTxnsMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewZetachainCrossChainTxnsInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NewZetachainCrossChainTxnsMonthlyInt =
    MapParseTo<StripExt<NewZetachainCrossChainTxnsMonthly>, i64>;
pub type NewZetachainCrossChainTxnsYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewZetachainCrossChainTxnsMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::tests::{
        normalize_sql, point_construction::dt, simple_test::simple_test_chart_with_zetachain_cctx,
    };

    #[test]
    fn statement_is_correct() {
        // mostly a test for easier comprehension of `NewZetachainCrossChainTxnsStatement`
        let actual = NewZetachainCrossChainTxnsStatement::get_statement(
            Some(dt("2025-01-01T00:00:00").and_utc()..dt("2025-01-02T00:00:00").and_utc()),
            &IndexerMigrations::latest(),
            &HashSet::new(),
        );
        let expected = r#"
            SELECT
                CAST(to_timestamp("cctx_status"."created_timestamp") AS date) AS "date",
                CAST(COUNT(*) AS TEXT) AS "value"
            FROM "cross_chain_tx"
            LEFT JOIN "cctx_status" ON "cross_chain_tx"."id" = "cctx_status"."cross_chain_tx_id"
            WHERE "cctx_status"."created_timestamp" < (extract(epoch from '2025-01-02 00:00:00.000000 +00:00'))
                AND "cctx_status"."created_timestamp" >= (extract(epoch from '2025-01-01 00:00:00.000000 +00:00'))
            GROUP BY "date"
        "#;
        assert_eq!(normalize_sql(expected), normalize_sql(&actual.to_string()))
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_zetachain_cross_chain_txns() {
        simple_test_chart_with_zetachain_cctx::<NewZetachainCrossChainTxns>(
            "update_new_zetachain_cross_chain_txns",
            vec![("2022-11-09", "1"), ("2022-11-10", "2")],
        )
        .await;
    }
}
