use std::ops::Range;

use crate::{
    data_source::{
        kinds::{
            data_manipulation::map::{Map, MapParseTo, StripWrapper},
            local_db::DirectPointLocalDbChartSource,
            remote_db::{PullOne24hCached, RemoteDatabaseSource, StatementFromRange},
        },
        types::{BlockscoutMigrations, WrappedValue},
    },
    utils::sql_with_range_filter_opt,
    ChartProperties, IndexingStatus, MissingDatePolicy, Named,
};
use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

use super::{CalculateOperationalTxns, NewTxns24hInt};

pub struct NewBlocks24hStatement;

impl StatementFromRange for NewBlocks24hStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        _completed_migrations: &BlockscoutMigrations,
    ) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT
                    COUNT(*)::TEXT as value
                FROM public.blocks
                WHERE
                    blocks.timestamp != to_timestamp(0) AND
                    consensus = true {filter};
            "#,
            [],
            "blocks.timestamp",
            range
        )
    }
}

// caching is not needed but I don't want to make another type just for this
//
// btw the caching should solve the problem with not storing `NewBlocks24h` in local
// db while not introducing any new unnecessary entries to the db. so it should be safe
// to use this in other places as well (in terms of efficiency)
pub type NewBlocks24h =
    RemoteDatabaseSource<PullOne24hCached<NewBlocks24hStatement, WrappedValue<String>>>;

pub type NewBlocks24hInt = MapParseTo<StripWrapper<NewBlocks24h>, i64>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newOperationalTxns24h".into()
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
    fn indexing_status_requirement() -> IndexingStatus {
        IndexingStatus::NoneIndexed
    }
}

pub type NewOperationalTxns24h = DirectPointLocalDbChartSource<
    Map<(NewBlocks24hInt, NewTxns24hInt), CalculateOperationalTxns<Properties>>,
    Properties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_operational_txns_24h() {
        simple_test_counter::<NewOperationalTxns24h>(
            "update_new_operational_txns_24h",
            // block at `2022-11-11T00:00:00` is not counted because
            // the relation is 'less than' in query
            "9",
            Some(dt("2022-11-11T00:00:00")),
        )
        .await;
    }
}
