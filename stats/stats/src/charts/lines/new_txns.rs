use std::ops::Range;

use crate::{
    charts::db_interaction::types::DateValueInt,
    data_source::kinds::{
        data_manipulation::map::MapParseTo,
        local_db::DirectVecLocalDbChartSource,
        remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
    },
    utils::sql_with_range_filter_opt,
    ChartProperties, DateValueString, Named,
};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

pub struct NewTxnsStatement;

impl StatementFromRange for NewTxnsStatement {
    fn get_statement(range: Option<Range<DateTimeUtc>>) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT
                    date(b.timestamp) as date,
                    COUNT(*)::TEXT as value
                FROM transactions t
                JOIN blocks       b ON t.block_hash = b.hash
                WHERE
                    b.timestamp != to_timestamp(0) AND
                    b.consensus = true {filter}
                GROUP BY date;
            "#,
            [],
            "b.timestamp",
            range
        )
    }
}

pub type NewTxnsRemote =
    RemoteDatabaseSource<PullAllWithAndSort<NewTxnsStatement, DateValueString>>;

pub struct NewTxnsProperties;

impl Named for NewTxnsProperties {
    const NAME: &'static str = "newTxns";
}

impl ChartProperties for NewTxnsProperties {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

pub type NewTxns = DirectVecLocalDbChartSource<NewTxnsRemote, NewTxnsProperties>;
pub type NewTxnsInt = MapParseTo<NewTxns, DateValueInt>;

#[cfg(test)]
mod tests {
    use super::NewTxns;
    use crate::tests::simple_test::{ranged_test_chart, simple_test_chart};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_txns() {
        simple_test_chart::<NewTxns>(
            "update_new_txns",
            vec![
                ("2022-11-09", "5"),
                ("2022-11-10", "12"),
                ("2022-11-11", "14"),
                ("2022-11-12", "5"),
                ("2022-12-01", "5"),
                ("2023-01-01", "1"),
                ("2023-02-01", "4"),
                ("2023-03-01", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn ranged_update_new_txns() {
        ranged_test_chart::<NewTxns>(
            "ranged_update_new_txns",
            vec![
                ("2022-11-09", "5"),
                ("2022-11-10", "12"),
                ("2022-11-11", "14"),
                ("2022-11-12", "5"),
                ("2022-12-01", "5"),
            ],
            "2022-11-08".parse().unwrap(),
            "2022-12-01".parse().unwrap(),
            None,
        )
        .await;
    }
}
