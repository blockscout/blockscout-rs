use crate::{
    charts::db_interaction::types::DateValueDouble,
    data_source::kinds::{
        adapter::point::{ToStringPointAdapter, ToStringPointAdapterWrapper},
        remote::point::{RemotePointSource, RemotePointSourceWrapper},
        updateable_chart::clone::point::{ClonePointChart, ClonePointChartWrapper},
    },
    Chart, Named,
};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

/// Use of batching does not make sense; set the max/no range
pub struct AverageBlockTimeRemote;

impl RemotePointSource for AverageBlockTimeRemote {
    type Point = DateValueDouble;
    fn get_query() -> Statement {
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

pub struct AverageBlockTimeRemoteString;

impl ToStringPointAdapter for AverageBlockTimeRemoteString {
    type InnerSource = RemotePointSourceWrapper<AverageBlockTimeRemote>;
}

pub struct AverageBlockTimeInner;

impl Named for AverageBlockTimeInner {
    const NAME: &'static str = "averageBlockTime";
}

impl Chart for AverageBlockTimeInner {
    fn chart_type() -> ChartType {
        ChartType::Counter
    }
}

impl ClonePointChart for AverageBlockTimeInner {
    type Dependency = ToStringPointAdapterWrapper<AverageBlockTimeRemoteString>;
}

pub type AverageBlockTime = ClonePointChartWrapper<AverageBlockTimeInner>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_block_time() {
        simple_test_counter::<AverageBlockTime>("update_average_block_time", "802200.0833333334")
            .await;
    }
}
