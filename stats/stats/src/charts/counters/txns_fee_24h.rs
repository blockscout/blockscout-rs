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
    utils::{produce_filter_and_values, sql_with_range_filter_opt},
    ChartError, ChartProperties, MissingDatePolicy, Named,
};
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, FromQueryResult, Statement};

const ETHER: i64 = i64::pow(10, 18);

pub struct TxnsFeeUngroupedStatement;

impl StatementFromRange for TxnsFeeUngroupedStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        completed_migrations: &BlockscoutMigrations,
    ) -> Statement {
        if completed_migrations.denormalization {
            // TODO: consider supporting such case in macro ?
            let mut args = vec![ETHER.into()];
            let (tx_filter, new_args) =
                produce_filter_and_values(range.clone(), "t.block_timestamp", args.len() + 1);
            args.extend(new_args);
            let (block_filter, new_args) =
                produce_filter_and_values(range.clone(), "b.timestamp", args.len() + 1);
            args.extend(new_args);
            let sql = format!(
                r#"
                    SELECT
                        (SUM(
                            t_filtered.gas_used *
                            COALESCE(
                                t_filtered.gas_price,
                                b.base_fee_per_gas + LEAST(
                                    t_filtered.max_priority_fee_per_gas,
                                    t_filtered.max_fee_per_gas - b.base_fee_per_gas
                                )
                            )
                        ) / $1)::FLOAT as value
                    FROM (
                        SELECT * from transactions t
                        WHERE
                            t.block_consensus = true AND
                            t.block_timestamp != to_timestamp(0) {tx_filter}
                    ) as t_filtered
                    JOIN blocks       b ON t_filtered.block_hash = b.hash
                    WHERE
                        b.timestamp != to_timestamp(0) AND
                        b.consensus = true {block_filter}
                "#,
            );
            Statement::from_sql_and_values(DbBackend::Postgres, sql, args)
        } else {
            sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                    SELECT
                        (SUM(
                            t.gas_used *
                            COALESCE(
                                t.gas_price,
                                b.base_fee_per_gas + LEAST(
                                    t.max_priority_fee_per_gas,
                                    t.max_fee_per_gas - b.base_fee_per_gas
                                )
                            )
                        ) / $1)::FLOAT as value
                    FROM transactions t
                    JOIN blocks       b ON t.block_hash = b.hash
                    WHERE
                        b.timestamp != to_timestamp(0) AND
                        b.consensus = true {filter}
                "#,
                [ETHER.into()],
                "b.timestamp",
                range
            )
        }
    }
}

#[derive(FromQueryResult)]
struct Value {
    // if there are no transactions/blocks - the row is still returned
    // but with null value
    value: Option<f64>,
}

pub struct TxnsFee24hQuery;

impl RemoteQueryBehaviour for TxnsFee24hQuery {
    type Output = TimespanValue<NaiveDate, String>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Self::Output, ChartError> {
        let update_time = cx.time;
        let range_24h = update_time
            .checked_sub_signed(TimeDelta::hours(24))
            .unwrap_or(DateTime::<Utc>::MIN_UTC)..=update_time;
        let query = TxnsFeeUngroupedStatement::get_statement(
            Some(inclusive_range_to_exclusive(range_24h)),
            &cx.blockscout_applied_migrations,
        );
        let value = Value::find_by_statement(query)
            .one(cx.blockscout.connection.as_ref())
            .await
            .map_err(ChartError::BlockscoutDB)?
            .map(|v| v.value)
            .flatten()
            // no transactions for yesterday
            .unwrap_or_else(|| 0.0);
        Ok(TimespanValue {
            timespan: update_time.date_naive(),
            value: value.to_string(),
        })
    }
}

pub type TxnsFee24hRemote = RemoteDatabaseSource<TxnsFee24hQuery>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "txnsFee24h".into()
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

pub type TxnsFee24h = DirectPointLocalDbChartSource<TxnsFee24hRemote, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_fee_24h_1() {
        simple_test_counter::<TxnsFee24h>("update_txns_fee_24h_1", "0.000023592592569", None).await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_fee_24h_2() {
        simple_test_counter::<TxnsFee24h>(
            "update_txns_fee_24h_2",
            "0.000495444443949",
            Some(dt("2022-11-11T00:00:00")),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_fee_24h_3() {
        simple_test_counter::<TxnsFee24h>(
            "update_txns_fee_24h_3",
            "0",
            Some(dt("2024-11-11T00:00:00")),
        )
        .await;
    }
}
