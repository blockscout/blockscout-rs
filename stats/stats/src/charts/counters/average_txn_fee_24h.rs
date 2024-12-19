use std::ops::Range;

use crate::{
    data_source::{
        kinds::{
            data_manipulation::map::{Map, MapFunction, MapToString, UnwrapOr},
            local_db::DirectPointLocalDbChartSource,
            remote_db::{PullOne24hCached, RemoteDatabaseSource, StatementFromRange},
        },
        types::BlockscoutMigrations,
    },
    gettable_const,
    types::TimespanValue,
    ChartProperties, MissingDatePolicy, Named,
};
use blockscout_db::entity::{blocks, transactions};
use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use migration::{Alias, Func};
use sea_orm::{DbBackend, FromQueryResult, IntoSimpleExpr, QuerySelect, QueryTrait, Statement};

const ETHER: i64 = i64::pow(10, 18);

pub struct TxnsStatsStatement;

impl StatementFromRange for TxnsStatsStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        completed_migrations: &BlockscoutMigrations,
    ) -> Statement {
        use sea_orm::prelude::*;

        let fee_query = Expr::cust_with_exprs(
            "COALESCE($1, $2 + LEAST($3, $4))",
            [
                transactions::Column::GasPrice.into_simple_expr(),
                blocks::Column::BaseFeePerGas.into_simple_expr(),
                transactions::Column::MaxPriorityFeePerGas.into_simple_expr(),
                transactions::Column::MaxFeePerGas
                    .into_simple_expr()
                    .sub(blocks::Column::BaseFeePerGas.into_simple_expr()),
            ],
        )
        .mul(transactions::Column::GasUsed.into_simple_expr());

        let count_query = Func::count(transactions::Column::Hash.into_simple_expr());
        let sum_query = Expr::expr(Func::sum(fee_query.clone()))
            .div(ETHER)
            .cast_as(Alias::new("FLOAT"));
        let avg_query = Expr::expr(Func::avg(fee_query))
            .div(ETHER)
            .cast_as(Alias::new("FLOAT"));

        let base_query = blocks::Entity::find().inner_join(transactions::Entity);
        let mut query = base_query
            .select_only()
            .expr_as(count_query, "count")
            .expr_as(sum_query, "fee_sum")
            .expr_as(avg_query, "fee_average")
            .to_owned();

        if let Some(r) = range {
            if completed_migrations.denormalization {
                query = query
                    .filter(transactions::Column::BlockTimestamp.lt(r.end))
                    .filter(transactions::Column::BlockTimestamp.gte(r.start))
            }
            query = query
                .filter(blocks::Column::Timestamp.lt(r.end))
                .filter(blocks::Column::Timestamp.gte(r.start));
        }

        query.build(DbBackend::Postgres)
    }
}

#[derive(Debug, Clone, FromQueryResult)]
pub struct TxnsStatsValue {
    pub count: i64,
    pub fee_average: Option<f64>,
    pub fee_sum: Option<f64>,
}

pub type Txns24hStats = RemoteDatabaseSource<PullOne24hCached<TxnsStatsStatement, TxnsStatsValue>>;

pub struct ExtractAverage;

impl MapFunction<TimespanValue<NaiveDate, TxnsStatsValue>> for ExtractAverage {
    type Output = TimespanValue<NaiveDate, Option<f64>>;

    fn function(
        inner_data: TimespanValue<NaiveDate, TxnsStatsValue>,
    ) -> Result<Self::Output, crate::ChartError> {
        Ok(TimespanValue {
            timespan: inner_data.timespan,
            value: inner_data.value.fee_average,
        })
    }
}

pub type AverageTxnFee24hExtracted = Map<Txns24hStats, ExtractAverage>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "averageTxnFee24h".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Counter
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillZero
    }
}

gettable_const!(Zero: f64 = 0.0);

pub type AverageTxnFee24h = DirectPointLocalDbChartSource<
    MapToString<UnwrapOr<AverageTxnFee24hExtracted, Zero>>,
    Properties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_txns_fee_1() {
        simple_test_counter::<AverageTxnFee24h>(
            "update_average_txns_fee_1",
            "0.000023592592569",
            None,
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_txns_fee_2() {
        simple_test_counter::<AverageTxnFee24h>(
            "update_average_txns_fee_2",
            "0.00006938997814411765",
            Some(dt("2022-11-11T16:00:00")),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_txns_fee_3() {
        simple_test_counter::<AverageTxnFee24h>(
            "update_average_txns_fee_3",
            "0",
            Some(dt("2024-10-10T00:00:00")),
        )
        .await;
    }
}
