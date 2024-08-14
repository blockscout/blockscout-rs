use crate::{
    data_source::kinds::{
        local_db::DirectPointLocalDbChartSource,
        remote_db::{PullOne, RemoteDatabaseSource, StatementForOne},
    },
    ChartProperties, MissingDatePolicy, Named,
};

use chrono::NaiveDate;
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

pub type TotalTokensRemote = RemoteDatabaseSource<PullOne<TotalTokensStatement, NaiveDate, String>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalTokens".into()
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

pub type TotalTokens = DirectPointLocalDbChartSource<TotalTokensRemote, Properties>;

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
