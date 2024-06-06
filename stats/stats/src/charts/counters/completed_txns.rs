use crate::data_source::kinds::updateable_chart::clone::point::ClonePointChartWrapper;

mod _inner {
    use crate::{
        data_source::kinds::{
            remote::point::{RemotePointSource, RemotePointSourceWrapper},
            updateable_chart::clone::point::ClonePointChart,
        },
        Chart, DateValueString, Named,
    };
    use entity::sea_orm_active_enums::ChartType;
    use sea_orm::{DbBackend, Statement};

    pub struct CompletedTxnsRemote;

    impl RemotePointSource for CompletedTxnsRemote {
        type Point = DateValueString;
        fn get_query() -> Statement {
            Statement::from_string(
                DbBackend::Postgres,
                r#"
                    SELECT
                        (all_success - all_success_dropped)::TEXT AS value,
                        last_block_date AS date 
                    FROM (
                        SELECT (
                            SELECT COUNT(*) AS all_success
                            FROM transactions t
                            WHERE t.status = 1
                        ), (
                            SELECT COUNT(*) as all_success_dropped
                            FROM transactions t
                            JOIN blocks b ON t.block_hash = b.hash
                            WHERE t.status = 1 AND b.consensus = false
                        ), (
                            SELECT MAX(b.timestamp)::DATE AS last_block_date
                            FROM blocks b
                            WHERE b.consensus = true
                        )
                    ) AS sub
                "#,
            )
        }
    }

    pub struct CompletedTxnsInner;

    impl Named for CompletedTxnsInner {
        const NAME: &'static str = "completedTxns";
    }

    impl Chart for CompletedTxnsInner {
        fn chart_type() -> ChartType {
            ChartType::Counter
        }
    }

    impl ClonePointChart for CompletedTxnsInner {
        type Dependency = RemotePointSourceWrapper<CompletedTxnsRemote>;
    }
}

pub type CompletedTxns = ClonePointChartWrapper<_inner::CompletedTxnsInner>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_completed_txns() {
        simple_test_counter::<CompletedTxns>("update_completed_txns", "46", None).await;
    }
}
