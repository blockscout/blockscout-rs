use std::ops::Range;

use crate::{
    data_source::{
        kinds::{
            local_db::DirectPointLocalDbChartSource,
            remote_db::{
                PullOne24h, RemoteDatabaseSource, StatementFromRange,
            },
        },
        types::BlockscoutMigrations,
    },
    utils::sql_with_range_filter_opt, ChartProperties, MissingDatePolicy, Named,
};
use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

/// `NewTxnsStatement` but without `group by`
pub struct NewTxnsUngroupedStatement;

impl StatementFromRange for NewTxnsUngroupedStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        completed_migrations: &BlockscoutMigrations,
    ) -> Statement {
        // do not filter by `!= to_timestamp(0)` because
        // 1. it allows to use index `transactions_block_consensus_index`
        // 2. there is no reason not to count genesis transactions
        if completed_migrations.denormalization {
            sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                    SELECT
                        COUNT(*)::TEXT as value
                    FROM transactions t
                    WHERE
                        t.block_consensus = true {filter};
                "#,
                [],
                "t.block_timestamp",
                range
            )
        } else {
            sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                    SELECT
                        COUNT(*)::TEXT as value
                    FROM transactions t
                    JOIN blocks       b ON t.block_hash = b.hash
                    WHERE
                        b.consensus = true {filter};
                "#,
                [],
                "b.timestamp",
                range
            )
        }
    }
}

pub type NewTxns24hRemote = RemoteDatabaseSource<PullOne24h<NewTxnsUngroupedStatement, String>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newTxns24h".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Counter
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
}

pub type NewTxns24h = DirectPointLocalDbChartSource<NewTxns24hRemote, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_txns_24h_1() {
        simple_test_counter::<NewTxns24h>("update_new_txns_24h_1", "1", None).await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_txns_24h_2() {
        simple_test_counter::<NewTxns24h>(
            "update_new_txns_24h_2",
            // block at `2022-11-11T00:00:00` is not counted because sql is not that precise :/
            "12",
            Some(dt("2022-11-11T00:00:00")),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_txns_24h_3() {
        simple_test_counter::<NewTxns24h>(
            "update_new_txns_24h_3",
            "0",
            Some(dt("2024-11-11T00:00:00")),
        )
        .await;
    }
}
