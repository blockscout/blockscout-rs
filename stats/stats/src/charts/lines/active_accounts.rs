//! Active accounts on each day.

use std::ops::Range;

use crate::{
    data_source::kinds::{
        local_db::DirectVecLocalDbChartSource,
        remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
    },
    utils::sql_with_range_filter_opt,
    ChartProperties, DateValueString, Named,
};

use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

pub struct ActiveAccountsStatement;

impl StatementFromRange for ActiveAccountsStatement {
    fn get_statement(range: Option<Range<DateTimeUtc>>) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT
                    DATE(blocks.timestamp) as date,
                    COUNT(DISTINCT from_address_hash)::TEXT as value
                FROM transactions
                JOIN blocks on transactions.block_hash = blocks.hash
                WHERE
                    blocks.timestamp != to_timestamp(0) AND
                    blocks.consensus = true {filter}
                GROUP BY date(blocks.timestamp);
            "#,
            [],
            "blocks.timestamp",
            range
        )
    }
}

pub type ActiveAccountsRemote =
    RemoteDatabaseSource<PullAllWithAndSort<ActiveAccountsStatement, DateValueString>>;

pub struct ActiveAccountsProperties;

impl Named for ActiveAccountsProperties {
    const NAME: &'static str = "activeAccounts";
}

impl ChartProperties for ActiveAccountsProperties {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

pub type ActiveAccounts =
    DirectVecLocalDbChartSource<ActiveAccountsRemote, ActiveAccountsProperties>;

#[cfg(test)]
mod tests {
    use crate::tests::simple_test::simple_test_chart;

    use super::ActiveAccounts;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_active_accounts() {
        simple_test_chart::<ActiveAccounts>(
            "update_active_accounts",
            vec![
                ("2022-11-09", "1"),
                ("2022-11-10", "3"),
                ("2022-11-11", "4"),
                ("2022-11-12", "1"),
                ("2022-12-01", "1"),
                ("2023-01-01", "1"),
                ("2023-02-01", "1"),
                ("2023-03-01", "1"),
            ],
        )
        .await;
    }
}
