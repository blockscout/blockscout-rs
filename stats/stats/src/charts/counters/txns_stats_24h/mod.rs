use std::{collections::HashSet, ops::Range};

use crate::{
    charts::db_interaction::utils::datetime_range_filter,
    data_source::{
        kinds::remote_db::{PullOne24hCached, RemoteDatabaseSource, StatementFromRange},
        types::BlockscoutMigrations,
    },
    lines::op_stack_operational_transactions_filter,
    ChartKey,
};
use blockscout_db::entity::{blocks, transactions};
use chrono::{DateTime, Utc};
use migration::{Alias, Func};
use sea_orm::{DbBackend, FromQueryResult, IntoSimpleExpr, QuerySelect, QueryTrait, Statement};

pub mod average_txn_fee_24h;
pub mod new_txns_24h;
pub mod op_stack_new_operational_txns_24h;
pub mod txns_fee_24h;

const ETHER: i64 = i64::pow(10, 18);

pub struct TxnsStatsStatement;

impl StatementFromRange for TxnsStatsStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        completed_migrations: &BlockscoutMigrations,
        _enabled_update_charts_recursive: &HashSet<ChartKey>,
    ) -> Statement {
        use sea_orm::prelude::*;

        // see `statement_is_correct` for resulting SQL statement
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
        let count_op_stack_query = Expr::cust(format!(
            r#"COUNT("transactions"."hash") FILTER (WHERE {})"#,
            op_stack_operational_transactions_filter("transactions")
        ));
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
            .expr_as(count_op_stack_query, "count_op_stack_operational")
            .expr_as(sum_query, "fee_sum")
            .expr_as(avg_query, "fee_average")
            .to_owned();

        if let Some(r) = range {
            if completed_migrations.denormalization {
                query = datetime_range_filter(query, transactions::Column::BlockTimestamp, &r);
            }
            query = datetime_range_filter(query, blocks::Column::Timestamp, &r);
        }

        query.build(DbBackend::Postgres)
    }
}

#[derive(Debug, Clone, FromQueryResult)]
pub struct TxnsStatsValue {
    pub count: i64,
    pub count_op_stack_operational: i64,
    pub fee_average: Option<f64>,
    pub fee_sum: Option<f64>,
}

pub type Txns24hStats = RemoteDatabaseSource<PullOne24hCached<TxnsStatsStatement, TxnsStatsValue>>;

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use pretty_assertions::assert_eq;

    use crate::{
        data_source::{kinds::remote_db::StatementFromRange, types::BlockscoutMigrations},
        tests::{normalize_sql, point_construction::dt},
    };

    use super::TxnsStatsStatement;

    #[test]
    fn statement_is_correct() {
        // mostly a test for easier comprehension of `TxnsStatsStatement`
        let actual = TxnsStatsStatement::get_statement(
            Some(dt("2025-01-01T00:00:00").and_utc()..dt("2025-01-02T00:00:00").and_utc()),
            &BlockscoutMigrations::latest(),
            &HashSet::new(),
        );

        let expected = r#"
            SELECT COUNT("transactions"."hash") AS "count",
                COUNT("transactions"."hash") FILTER (WHERE NOT (
                    transactions.from_address_hash = decode('deaddeaddeaddeaddeaddeaddeaddeaddead0001', 'hex') AND
                    transactions.to_address_hash = decode('4200000000000000000000000000000000000015', 'hex')
                )) AS "count_op_stack_operational",
                CAST((SUM((COALESCE("transactions"."gas_price", "blocks"."base_fee_per_gas" + LEAST("transactions"."max_priority_fee_per_gas", "transactions"."max_fee_per_gas" - "blocks"."base_fee_per_gas"))) * "transactions"."gas_used") / 1000000000000000000) AS FLOAT) AS "fee_sum",
                CAST((AVG((COALESCE("transactions"."gas_price", "blocks"."base_fee_per_gas" + LEAST("transactions"."max_priority_fee_per_gas", "transactions"."max_fee_per_gas" - "blocks"."base_fee_per_gas"))) * "transactions"."gas_used") / 1000000000000000000) AS FLOAT) AS "fee_average"
            FROM "blocks"
            INNER JOIN "transactions" ON "blocks"."hash" = "transactions"."block_hash"
            WHERE "transactions"."block_timestamp" < '2025-01-02 00:00:00 +00:00'
                AND "transactions"."block_timestamp" >= '2025-01-01 00:00:00 +00:00'
                AND "blocks"."timestamp" < '2025-01-02 00:00:00 +00:00'
                AND "blocks"."timestamp" >= '2025-01-01 00:00:00 +00:00'
        "#;
        assert_eq!(normalize_sql(expected), normalize_sql(&actual.to_string()))
    }
}
