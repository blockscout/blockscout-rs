use std::ops::Range;

use crate::data_source::{
    kinds::remote_db::{PullOne24hCached, RemoteDatabaseSource, StatementFromRange},
    types::BlockscoutMigrations,
};
use blockscout_db::entity::{blocks, transactions};
use chrono::{DateTime, Utc};
use migration::{Alias, Func};
use sea_orm::{DbBackend, FromQueryResult, IntoSimpleExpr, QuerySelect, QueryTrait, Statement};

pub mod average_txn_fee_24h;
pub mod new_txns_24h;
pub mod txns_fee_24h;

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
