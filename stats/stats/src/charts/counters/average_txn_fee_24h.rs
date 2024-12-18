use std::ops::Range;

use crate::{
    data_source::{
        kinds::{
            data_manipulation::map::{MapToString, UnwrapOr},
            local_db::DirectPointLocalDbChartSource,
            remote_db::{PullOne24h, RemoteDatabaseSource, StatementFromRange},
        },
        types::BlockscoutMigrations,
    },
    gettable_const,
    utils::{produce_filter_and_values, sql_with_range_filter_opt},
    ChartProperties, MissingDatePolicy, Named,
};
use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

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

pub type AverageTxnFee24hRemote =
    RemoteDatabaseSource<PullOne24h<AverageTxnFeeUngroupedStatement, Option<f64>>>;

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

pub type AverageTxnFee24h =
    DirectPointLocalDbChartSource<MapToString<UnwrapOr<AverageTxnFee24hRemote, Zero>>, Properties>;

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
