use std::ops::Range;

use crate::{
    data_source::{
        kinds::{
            data_manipulation::map::MapToString,
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

/// `AverageTxnFeeStatement` but without `group by`
pub struct AverageTxnFeeUngroupedStatement;

impl StatementFromRange for AverageTxnFeeUngroupedStatement {
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
                        (AVG(
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
                        (AVG(
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
    value: f64,
}

pub struct AverageTxnFee24hQuery;

impl RemoteQueryBehaviour for AverageTxnFee24hQuery {
    type Output = TimespanValue<NaiveDate, f64>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Self::Output, ChartError> {
        let update_time = cx.time;
        let range_24h = update_time
            .checked_sub_signed(TimeDelta::hours(24))
            .unwrap_or(DateTime::<Utc>::MIN_UTC)..=update_time;
        let query = AverageTxnFeeUngroupedStatement::get_statement(
            Some(inclusive_range_to_exclusive(range_24h)),
            &cx.blockscout_applied_migrations,
        );

        let data = Value::find_by_statement(query)
            .one(cx.blockscout.connection.as_ref())
            .await
            .map_err(ChartError::BlockscoutDB)?
            // no transactions for yesterday
            .unwrap_or_else(|| Value { value: 0.0 });

        Ok(TimespanValue {
            timespan: update_time.date_naive(),
            value: data.value,
        })
    }
}

pub type AverageTxnFee24hRemote = RemoteDatabaseSource<AverageTxnFee24hQuery>;

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

pub type AverageTxnFee24h =
    DirectPointLocalDbChartSource<MapToString<AverageTxnFee24hRemote>, Properties>;

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
}
