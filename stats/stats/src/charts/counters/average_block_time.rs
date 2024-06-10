use crate::data_source::kinds::updateable_chart::clone::point::ClonePointChartWrapper;

mod _inner {
    use crate::{
        charts::db_interaction::types::DateValueDouble,
        data_source::kinds::{
            map::to_string::MapToString,
            remote_db::{PullOne, RemoteDatabaseSource, StatementForOne},
            updateable_chart::clone::point::ClonePointChart,
        },
        Chart, Named,
    };
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
        RemoteDatabaseSource<PullOne<AverageBlockTimeStatement, DateValueDouble>>;

    pub type AverageBlockTimeRemoteString = MapToString<AverageBlockTimeRemote>;

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
        type Dependency = AverageBlockTimeRemoteString;
    }
}

pub type AverageBlockTime = ClonePointChartWrapper<_inner::AverageBlockTimeInner>;

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
