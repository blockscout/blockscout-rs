use std::ops::Range;

use chrono::{DateTime, Utc};
use sea_orm::{DbBackend, Statement};

use crate::{
    data_source::{
        kinds::remote_db::{PullAllWithAndSortCached, RemoteDatabaseSource, StatementFromRange},
        types::BlockscoutMigrations,
    },
    types::new_txns::NewTxnsCombinedPoint,
    utils::sql_with_range_filter_opt,
    QueryAllBlockTimestampRange,
};

pub mod all_new_txns;
pub mod op_stack_operational;
pub use all_new_txns::{
    NewTxns, NewTxnsInt, NewTxnsMonthly, NewTxnsMonthlyInt, NewTxnsWeekly, NewTxnsYearly,
};

pub struct NewTxnsCombinedStatement;

impl StatementFromRange for NewTxnsCombinedStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        completed_migrations: &BlockscoutMigrations,
    ) -> Statement {
        // format!("{}", "a", ());
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
