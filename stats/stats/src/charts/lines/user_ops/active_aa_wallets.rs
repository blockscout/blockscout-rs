//! Active account_abstraction_wallets on each day.

use std::ops::Range;

use crate::{
    charts::db_interaction::read::QueryAllBlockTimestampRange,
    data_source::{
        kinds::{
            local_db::{
                parameters::update::batching::parameters::Batch30Days, DirectVecLocalDbChartSource,
            },
            remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
        },
        types::BlockscoutMigrations,
    },
    ChartProperties, Named,
};

use blockscout_db::entity::user_operations;
use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use migration::IntoColumnRef;
use sea_orm::Statement;

use super::active_bundlers::count_distinct_in_user_ops;

pub struct ActiveAccountAbstractionWalletsStatement;

impl StatementFromRange for ActiveAccountAbstractionWalletsStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        _completed_migrations: &BlockscoutMigrations,
    ) -> Statement {
        count_distinct_in_user_ops(user_operations::Column::Sender.into_column_ref(), range)
    }
}

pub type ActiveAccountAbstractionWalletsRemote = RemoteDatabaseSource<
    PullAllWithAndSort<
        ActiveAccountAbstractionWalletsStatement,
        NaiveDate,
        String,
        QueryAllBlockTimestampRange,
    >,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "activeAccountAbstractionWallets".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

pub type ActiveAccountAbstractionWallets =
    DirectVecLocalDbChartSource<ActiveAccountAbstractionWalletsRemote, Batch30Days, Properties>;

#[cfg(test)]
mod tests {
    use crate::tests::simple_test::simple_test_chart;

    use super::ActiveAccountAbstractionWallets;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_active_account_abstraction_wallets() {
        simple_test_chart::<ActiveAccountAbstractionWallets>(
            "update_active_account_abstraction_wallets",
            vec![
                ("2022-11-09", "1"),
                ("2022-11-10", "1"),
                ("2022-11-11", "1"),
                ("2022-11-12", "1"),
                ("2022-12-01", "1"),
                ("2023-02-01", "1"),
            ],
        )
        .await;
    }
}
