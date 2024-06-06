//! Active accounts on each day.

use crate::data_source::kinds::updateable_chart::clone::CloneChartWrapper;

mod _inner {
    use crate::{
        data_source::kinds::{
            remote::{RemoteSource, RemoteSourceWrapper},
            updateable_chart::clone::CloneChart,
        },
        utils::sql_with_range_filter_opt,
        Chart, DateValueString, Named,
    };
    use entity::sea_orm_active_enums::ChartType;
    use sea_orm::{prelude::*, DbBackend, Statement};

    pub struct ActiveAccountsRemote;

    impl RemoteSource for ActiveAccountsRemote {
        type Point = DateValueString;

        fn get_query(range: Option<std::ops::RangeInclusive<DateTimeUtc>>) -> Statement {
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

    pub struct ActiveAccountsInner;

    impl Named for ActiveAccountsInner {
        const NAME: &'static str = "activeAccounts";
    }

    impl Chart for ActiveAccountsInner {
        fn chart_type() -> ChartType {
            ChartType::Line
        }
    }

    impl CloneChart for ActiveAccountsInner {
        type Dependency = RemoteSourceWrapper<ActiveAccountsRemote>;
    }
}

pub type ActiveAccounts = CloneChartWrapper<_inner::ActiveAccountsInner>;

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
