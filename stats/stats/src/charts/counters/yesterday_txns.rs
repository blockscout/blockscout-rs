use crate::{
    data_source::{
        kinds::{
            local_db::DirectPointLocalDbChartSource,
            remote_db::{PullOne, RemoteDatabaseSource, StatementForOne},
        },
        types::BlockscoutMigrations,
    },
    ChartProperties, MissingDatePolicy, Named,
};
use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

pub struct YesterdayTxnsStatement;

impl StatementForOne for YesterdayTxnsStatement {
    fn get_statement(_: &BlockscoutMigrations) -> Statement {
        Statement::from_string(
            DbBackend::Postgres,
            r#"
                SELECT
                    date, value
                FROM ( 
                    SELECT (
                        SELECT COUNT(*)::TEXT as value FROM addresses
                    ), (
                        SELECT MAX(b.timestamp)::DATE AS date
                        FROM blocks b
                        WHERE b.consensus = true
                    )
                ) as sub
            "#,
        )
    }
}

pub type YesterdayTxnsRemote =
    RemoteDatabaseSource<PullOne<YesterdayTxnsStatement, NaiveDate, String>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "yesterdayTxns".into()
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

pub type YesterdayTxns = DirectPointLocalDbChartSource<YesterdayTxnsRemote, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_yesterday_txns() {
        simple_test_counter::<YesterdayTxns>("update_yesterday_txns", "33", None).await;
    }
}