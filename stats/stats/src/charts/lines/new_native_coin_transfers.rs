use std::ops::RangeInclusive;

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

pub struct NewNativeCoinTransfersStatement;

impl StatementFromRange for NewNativeCoinTransfersStatement {
    fn get_statement(range: Option<RangeInclusive<DateTimeUtc>>) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                    SELECT 
                        DATE(b.timestamp) as date,
                        COUNT(*)::TEXT as value
                    FROM transactions t
                    JOIN blocks       b ON t.block_hash = b.hash
                    WHERE
                        b.timestamp != to_timestamp(0) AND
                        b.consensus = true AND
                        LENGTH(t.input) = 0 AND
                        t.value >= 0 {filter}
                    GROUP BY date
                "#,
            [],
            "b.timestamp",
            range
        )
    }
}

pub type NewNativeCoinTransfersRemote =
    RemoteDatabaseSource<PullAllWithAndSort<NewNativeCoinTransfersStatement, DateValueString>>;

pub struct NewNativeCoinTransfersProperties;

impl Named for NewNativeCoinTransfersProperties {
    const NAME: &'static str = "newNativeCoinTransfers";
}

impl ChartProperties for NewNativeCoinTransfersProperties {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

pub type NewNativeCoinTransfers =
    DirectVecLocalDbChartSource<NewNativeCoinTransfersRemote, NewNativeCoinTransfersProperties>;

pub type NewNativeCoinTransfersInt = MapParseTo<NewNativeCoinTransfers, DateValueInt>;

#[cfg(test)]
mod tests {
    use super::NewNativeCoinTransfers;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_native_coins_transfers() {
        simple_test_chart::<NewNativeCoinTransfers>(
            "update_native_coins_transfers",
            vec![
                ("2022-11-09", "2"),
                ("2022-11-10", "4"),
                ("2022-11-11", "4"),
                ("2022-11-12", "2"),
                ("2022-12-01", "2"),
                ("2023-02-01", "2"),
                ("2023-03-01", "1"),
            ],
        )
        .await;
    }
}
