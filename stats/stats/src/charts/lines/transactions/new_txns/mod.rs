use std::{collections::HashSet, ops::Range};

use blockscout_db::entity::{blocks, transactions};
use chrono::{DateTime, Utc};
use migration::{Alias, ExprTrait, IntoIden};
use sea_orm::{
    sea_query, ColumnTrait, DatabaseBackend, DbBackend, EntityName, EntityTrait, IntoIdentity,
    IntoSimpleExpr, QueryFilter, QueryOrder, QuerySelect, QueryTrait, Statement,
};
use sea_query::{Expr, Func};

use crate::{
    charts::db_interaction::utils::datetime_range_filter,
    data_source::{
        kinds::remote_db::{PullAllWithAndSortCached, RemoteDatabaseSource, StatementFromRange},
        types::BlockscoutMigrations,
    },
    types::new_txns::NewTxnsCombinedPoint,
    utils::sql_with_range_filter_opt,
    ChartKey, Named, QueryAllBlockTimestampRange,
};

pub mod all_new_txns;
pub mod op_stack_operational;
pub use all_new_txns::{
    NewTxns, NewTxnsInt, NewTxnsMonthly, NewTxnsMonthlyInt, NewTxnsWeekly, NewTxnsYearly,
};

fn get_statement_experimental(
    range: Option<Range<DateTime<Utc>>>,
    completed_migrations: &BlockscoutMigrations,
    enabled_group_charts_recursive: &HashSet<String>,
) -> Statement {
    let use_op_stack_filter: bool = enabled_group_charts_recursive
        .contains(&op_stack_operational::OpStackNewOperationalTxns::name());
    let denormalized = completed_migrations.denormalization;

    let date_intermediate_col = "date".into_identity();
    let mut query = transactions::Entity::find().select_only();
    if !denormalized {
        query = query.join(
            sea_orm::JoinType::InnerJoin,
            transactions::Entity::belongs_to(blocks::Entity)
                .from(transactions::Column::BlockHash)
                .to(blocks::Column::Hash)
                .into(),
        );
    }
    let date_expr = if denormalized {
        transactions::Column::BlockTimestamp
            .into_simple_expr()
            .cast_as(Alias::new("date"))
    } else {
        blocks::Column::Timestamp
            .into_simple_expr()
            .cast_as(Alias::new("date"))
    };
    query = query.expr_as(date_expr, date_intermediate_col.clone());
    let op_stack_expr = if use_op_stack_filter {
        Expr::cust(format!(
            "COUNT(*) FILTER (WHERE {})",
            op_stack_operational::transactions_filter(transactions::Entity.table_name())
        ))
    } else {
        Expr::value(0)
    };
    query = query
        .expr_as(
            Func::count(Expr::col(sea_query::Asterisk)).cast_as(Alias::new("text")),
            "all_transactions",
        )
        .expr_as(
            op_stack_expr.cast_as(Alias::new("text")),
            "op_stack_operational_transactions",
        );

    let consensus_filter_expr = if denormalized {
        transactions::Column::BlockConsensus.eq(true)
    } else {
        blocks::Column::Consensus.eq(true)
    };

    query = query
        .filter(consensus_filter_expr)
        .group_by(Expr::col(date_intermediate_col.clone().into_iden()))
        .order_by(
            Expr::col(date_intermediate_col.into_iden()),
            sea_orm::Order::Asc,
        );

    if let Some(range) = range {
        if denormalized {
            query = datetime_range_filter(query, transactions::Column::BlockTimestamp, &range);
        } else {
            query = datetime_range_filter(query, blocks::Column::Timestamp, &range);
        }
    }

    query.build(DatabaseBackend::Postgres)
}

pub struct NewTxnsCombinedStatement;

impl StatementFromRange for NewTxnsCombinedStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        completed_migrations: &BlockscoutMigrations,
        _: &HashSet<ChartKey>,
    ) -> Statement {
        // do not filter by `!= to_timestamp(0)` because
        // 1. it allows to use index `transactions_block_consensus_index`
        // 2. there is no reason not to count genesis transactions
        if completed_migrations.denormalization {
            sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                    SELECT
                        date(t.block_timestamp) as date,
                        COUNT(*)::TEXT as all_transactions,
                        (COUNT(*) FILTER (WHERE {op_stack_filter}))::TEXT as op_stack_operational_transactions
                    FROM transactions t
                    WHERE
                        t.block_consensus = true {filter}
                    GROUP BY date;
                "#,
                [],
                "t.block_timestamp",
                range,
                op_stack_filter = op_stack_operational::transactions_filter("t")
            )
        } else {
            sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                    SELECT
                        date(b.timestamp) as date,
                        COUNT(*)::TEXT as all_transactions,
                        (COUNT(*) FILTER (WHERE {op_stack_filter}))::TEXT as op_stack_operational_transactions
                    FROM transactions t
                    JOIN blocks       b ON t.block_hash = b.hash
                    WHERE
                        b.consensus = true {filter}
                    GROUP BY date;
                "#,
                [],
                "b.timestamp",
                range,
                op_stack_filter = op_stack_operational::transactions_filter("t")
            )
        }
    }
}

pub type NewTxnsCombinedRemote = RemoteDatabaseSource<
    PullAllWithAndSortCached<
        NewTxnsCombinedStatement,
        NewTxnsCombinedPoint,
        QueryAllBlockTimestampRange,
    >,
>;

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use pretty_assertions::assert_eq;

    use crate::{
        data_source::types::BlockscoutMigrations,
        tests::{normalize_sql, point_construction::dt},
        Named,
    };

    use super::{get_statement_experimental, op_stack_operational};

    #[test]
    fn statements_are_correct() {
        let test_range = dt("2025-01-01T00:00:00").and_utc()..dt("2025-01-02T00:00:00").and_utc();
        let from_hash = op_stack_operational::ATTRIBUTES_DEPOSITED_FROM_HASH;
        let to_hash = op_stack_operational::ATTRIBUTES_DEPOSITED_TO_HASH;
        let enabled_with_op_stack =
            HashSet::from([op_stack_operational::OpStackNewOperationalTxns::name()]);

        let denormalized_with_op_stack = get_statement_experimental(
            Some(test_range.clone()),
            &BlockscoutMigrations::latest(),
            &enabled_with_op_stack,
        );
        let expected = format!(
            r#"
                SELECT
                    CAST("transactions"."block_timestamp" AS date) AS "date",
                    CAST(COUNT(*) AS text) AS "all_transactions",
                    CAST((COUNT(*) FILTER (WHERE NOT (
                        transactions.from_address_hash = decode('{from_hash}', 'hex') AND
                        transactions.to_address_hash = decode('{to_hash}', 'hex')
                    ))) AS text) AS "op_stack_operational_transactions"
                FROM "transactions"
                WHERE
                    "transactions"."block_consensus" = TRUE AND
                    "transactions"."block_timestamp" < '2025-01-02 00:00:00 +00:00' AND
                    "transactions"."block_timestamp" >= '2025-01-01 00:00:00 +00:00'
                GROUP BY "date"
                ORDER BY "date" ASC
            "#,
        );
        assert_eq!(
            normalize_sql(&expected),
            normalize_sql(&denormalized_with_op_stack.to_string())
        );

        let not_denormalized_with_op_stack = get_statement_experimental(
            Some(test_range.clone()),
            &BlockscoutMigrations::empty(),
            &enabled_with_op_stack,
        );

        let expected = format!(
            r#"
                SELECT
                    CAST("blocks"."timestamp" AS date) AS "date",
                    CAST(COUNT(*) AS text) AS "all_transactions",
                    CAST((COUNT(*) FILTER (WHERE NOT (
                        transactions.from_address_hash = decode('{from_hash}', 'hex') AND
                        transactions.to_address_hash = decode('{to_hash}', 'hex')
                    ))) AS text) AS "op_stack_operational_transactions"
                FROM "transactions"
                INNER JOIN "blocks" ON "transactions"."block_hash" = "blocks"."hash"
                WHERE
                    "blocks"."consensus" = TRUE AND
                    "blocks"."timestamp" < '2025-01-02 00:00:00 +00:00' AND
                    "blocks"."timestamp" >= '2025-01-01 00:00:00 +00:00'
                GROUP BY "date"
                ORDER BY "date" ASC
            "#,
        );
        assert_eq!(
            normalize_sql(&expected),
            normalize_sql(&not_denormalized_with_op_stack.to_string())
        );

        let denormalized_without_op_stack = get_statement_experimental(
            Some(test_range),
            &BlockscoutMigrations::latest(),
            &HashSet::new(),
        );
        let expected = format!(
            r#"
                SELECT
                    CAST("transactions"."block_timestamp" AS date) AS "date",
                    CAST(COUNT(*) AS text) AS "all_transactions",
                    CAST(0 AS text) AS "op_stack_operational_transactions"
                FROM "transactions"
                WHERE
                    "transactions"."block_consensus" = TRUE AND
                    "transactions"."block_timestamp" < '2025-01-02 00:00:00 +00:00' AND
                    "transactions"."block_timestamp" >= '2025-01-01 00:00:00 +00:00'
                GROUP BY "date"
                ORDER BY "date" ASC
            "#,
        );
        assert_eq!(
            normalize_sql(&expected),
            normalize_sql(&denormalized_without_op_stack.to_string())
        );
    }
}
