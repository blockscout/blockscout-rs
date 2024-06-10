use crate::data_source::kinds::updateable_chart::clone::point::ClonePointChartWrapper;

mod _inner {
    use crate::{
        data_source::kinds::{
            remote_db::{PullOne, RemoteDatabaseSource, StatementForOne},
            updateable_chart::clone::point::ClonePointChart,
        },
        Chart, DateValueString, Named,
    };
    use entity::sea_orm_active_enums::ChartType;
    use sea_orm::{DbBackend, Statement};

    pub struct TotalTokensStatement;

    impl StatementForOne for TotalTokensStatement {
        fn get_statement() -> Statement {
            Statement::from_string(
                DbBackend::Postgres,
                r#"
                    SELECT 
                        (
                            SELECT count(*)::text
                                FROM tokens
                        ) AS "value",
                        (
                            SELECT max(timestamp)::date as "date" 
                                FROM blocks
                                WHERE blocks.consensus = true
                        ) AS "date"
                "#,
            )
        }
    }

    pub type TotalTokensRemote =
        RemoteDatabaseSource<PullOne<TotalTokensStatement, DateValueString>>;

    pub struct TotalTokensInner;

    impl Named for TotalTokensInner {
        const NAME: &'static str = "totalTokens";
    }

    impl Chart for TotalTokensInner {
        fn chart_type() -> ChartType {
            ChartType::Counter
        }
    }

    impl ClonePointChart for TotalTokensInner {
        type Dependency = TotalTokensRemote;
    }
}

pub type TotalTokens = ClonePointChartWrapper<_inner::TotalTokensInner>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_tokens() {
        simple_test_counter::<TotalTokens>("update_total_tokens", "4", None).await;
    }
}
