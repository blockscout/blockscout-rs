use crate::{
    charts::db_interaction::types::DateValueInt,
    data_source::kinds::{
        adapter::{ParseAdapter, ParseAdapterWrapper},
        remote::{RemoteSource, RemoteSourceWrapper},
        updateable_chart::clone::{CloneChart, CloneChartWrapper},
    },
    utils::sql_with_range_filter_opt,
    Chart, DateValueString, Named,
};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

pub struct NewTxnsRemote;

impl RemoteSource for NewTxnsRemote {
    type Point = DateValueString;

    fn get_query(range: Option<std::ops::RangeInclusive<DateTimeUtc>>) -> Statement {
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
    type Dependency = RemoteSourceWrapper<NewTxnsRemote>;
}

pub type NewTxns = CloneChartWrapper<NewTxnsInner>;

pub struct NewTxnsIntInner;

impl ParseAdapter for NewTxnsIntInner {
    type InnerSource = NewTxns;
    type ParseInto = DateValueInt;
}

pub type NewTxnsInt = ParseAdapterWrapper<NewTxnsIntInner>;

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
                ("2022-11-08", "0"),
                ("2022-11-09", "5"),
                ("2022-11-10", "12"),
                ("2022-11-11", "14"),
                ("2022-11-12", "5"),
                ("2022-11-13", "0"),
                ("2022-11-14", "0"),
                ("2022-11-15", "0"),
                ("2022-11-16", "0"),
                ("2022-11-17", "0"),
                ("2022-11-18", "0"),
                ("2022-11-19", "0"),
                ("2022-11-20", "0"),
                ("2022-11-21", "0"),
                ("2022-11-22", "0"),
                ("2022-11-23", "0"),
                ("2022-11-24", "0"),
                ("2022-11-25", "0"),
                ("2022-11-26", "0"),
                ("2022-11-27", "0"),
                ("2022-11-28", "0"),
                ("2022-11-29", "0"),
                ("2022-11-30", "0"),
                ("2022-12-01", "5"),
            ],
            "2022-11-08".parse().unwrap(),
            "2022-12-01".parse().unwrap(),
            None,
        )
        .await;
    }
}
