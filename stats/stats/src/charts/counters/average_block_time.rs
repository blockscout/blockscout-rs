use crate::{
    data_source::kinds::{
        data_manipulation::map::MapToString,
        local_db::DirectPointLocalDbChartSource,
        remote_db::{PullOne, RemoteDatabaseSource, StatementForOne},
    },
    ChartProperties, MissingDatePolicy, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

pub struct AverageBlockTimeStatement;

impl StatementForOne for AverageBlockTimeStatement {
    fn get_statement() -> Statement {
        Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
                SELECT
                    max(timestamp)::date as date, 
                    (CASE WHEN avg(diff) IS NULL THEN 0 ELSE avg(diff) END)::float as value
                FROM
                (
                    SELECT
                        timestamp,
                        EXTRACT(
                            EPOCH FROM timestamp - lag(timestamp) OVER (ORDER BY timestamp)
                        ) as diff
                    FROM blocks b
                    WHERE b.timestamp != to_timestamp(0) AND consensus = true
                ) t
            "#,
            vec![],
        )
    }
}

pub type AverageBlockTimeRemote =
    RemoteDatabaseSource<PullOne<AverageBlockTimeStatement, NaiveDate, f64>>;

pub type AverageBlockTimeRemoteString = MapToString<AverageBlockTimeRemote>;

pub struct AverageBlockTimeProperties;

impl Named for AverageBlockTimeProperties {
    fn name() -> String {
                "averageBlockTime".into()
            }
}

impl ChartProperties for AverageBlockTimeProperties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Counter
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
}

pub type AverageBlockTime =
    DirectPointLocalDbChartSource<AverageBlockTimeRemoteString, AverageBlockTimeProperties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_block_time() {
        simple_test_counter::<AverageBlockTime>(
            "update_average_block_time",
            "802200.0833333334",
            None,
        )
        .await;
    }
}
