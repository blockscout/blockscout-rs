use crate::data_source::kinds::{
    adapter::ParseAdapterWrapper, updateable_chart::clone::CloneChartWrapper,
};

/// Items in this module are not intended to be used outside. They are only public
/// since the actual public type is just an alias (to wrapper).
///
/// I.e. use [`super`]'s types.
pub mod _inner {
    use crate::{
        charts::db_interaction::types::DateValueInt,
        data_source::kinds::{
            adapter::ParseAdapter,
            remote::{RemoteSource, RemoteSourceWrapper},
            updateable_chart::clone::CloneChart,
        },
        utils::sql_with_range_filter_opt,
        Chart, DateValueString, Named,
    };
    use entity::sea_orm_active_enums::ChartType;
    use sea_orm::{prelude::*, DbBackend, Statement};

    use super::NewTxns;

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

    pub struct NewTxnsIntInner;

    impl ParseAdapter for NewTxnsIntInner {
        type InnerSource = NewTxns;
        type ParseInto = DateValueInt;
    }
}

pub type NewTxns = CloneChartWrapper<_inner::NewTxnsInner>;
pub type NewTxnsInt = ParseAdapterWrapper<_inner::NewTxnsIntInner>;

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
