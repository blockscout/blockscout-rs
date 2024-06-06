use crate::data_source::kinds::updateable_chart::clone::point::ClonePointChartWrapper;

/// Items in this module are not intended to be used outside. They are only public
/// since the actual public type is just an alias (to wrapper).
///
/// I.e. use [`super`]'s types.
pub mod _inner {
    use crate::{
        data_source::kinds::{
            remote::point::{RemotePointSource, RemotePointSourceWrapper},
            updateable_chart::clone::point::ClonePointChart,
        },
        Chart, DateValueString, Named,
    };
    use entity::sea_orm_active_enums::ChartType;
    use sea_orm::{DbBackend, Statement};

    pub struct TotalTokensRemote;

    impl RemotePointSource for TotalTokensRemote {
        type Point = DateValueString;
        fn get_query() -> Statement {
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
        type Dependency = RemotePointSourceWrapper<TotalTokensRemote>;
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
