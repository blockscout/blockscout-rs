use std::ops::Range;

use crate::{
    data_source::{
        kinds::{
            local_db::DirectPointLocalDbChartSource,
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour, StatementFromRange},
        },
        types::BlockscoutMigrations,
        UpdateContext,
    },
    range::{inclusive_range_to_exclusive, UniversalRange},
    types::TimespanValue,
    utils::sql_with_range_filter_opt,
    ChartError, ChartProperties, MissingDatePolicy, Named,
};
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, FromQueryResult, Statement};

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

#[derive(FromQueryResult)]
struct Value {
    value: String,
}

pub struct NewTxns24hQuery;

impl RemoteQueryBehaviour for NewTxns24hQuery {
    type Output = TimespanValue<NaiveDate, String>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Self::Output, ChartError> {
        let update_time = cx.time;
        let range_24h = update_time
            .checked_sub_signed(TimeDelta::hours(24))
            .unwrap_or(DateTime::<Utc>::MIN_UTC)..=update_time;
        let query = NewTxnsUngroupedStatement::get_statement(
            Some(inclusive_range_to_exclusive(range_24h)),
            &cx.blockscout_applied_migrations,
        );
        let data = Value::find_by_statement(query)
            .one(cx.blockscout.connection.as_ref())
            .await
            .map_err(ChartError::BlockscoutDB)?
            // no transactions for yesterday
            .unwrap_or_else(|| Value {
                value: "0".to_string(),
            });
        Ok(TimespanValue {
            timespan: update_time.date_naive(),
            value: data.value,
        })
    }
}

pub type NewTxns24hRemote = RemoteDatabaseSource<NewTxns24hQuery>;

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
