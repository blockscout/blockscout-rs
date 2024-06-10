use crate::{
    charts::db_interaction::types::DateValueInt,
    data_source::kinds::{map::parse::MapParseTo, updateable_chart::clone::CloneChartWrapper},
};

mod _inner {
    use std::ops::RangeInclusive;

    use crate::{
        data_source::kinds::{
            remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
            updateable_chart::clone::CloneChart,
        },
        utils::sql_with_range_filter_opt,
        Chart, DateValueString, Named,
    };
    use entity::sea_orm_active_enums::ChartType;
    use sea_orm::{prelude::*, DbBackend, Statement};

    pub struct NewTxnsStatement;

    impl StatementFromRange for NewTxnsStatement {
        fn get_statement(range: Option<RangeInclusive<DateTimeUtc>>) -> Statement {
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

    pub struct NewTxnsInner;

    impl Named for NewTxnsInner {
        const NAME: &'static str = "newTxns";
    }

    impl Chart for NewTxnsInner {
        fn chart_type() -> ChartType {
            ChartType::Line
        }
    }

    impl CloneChart for NewTxnsInner {
        type Dependency = NewTxnsRemote;
    }
}

pub type NewTxns = CloneChartWrapper<_inner::NewTxnsInner>;
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
