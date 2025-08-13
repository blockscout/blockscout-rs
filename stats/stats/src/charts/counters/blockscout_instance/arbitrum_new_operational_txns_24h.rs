use crate::chart_prelude::*;
use blockscout_db::entity::blocks;

use super::{CalculateOperationalTxns, NewTxns24hInt};

pub struct NewBlocks24hStatement;
impl_db_choice!(NewBlocks24hStatement, UseBlockscoutDB);

impl StatementFromUpdateTime for NewBlocks24hStatement {
    fn get_statement(
        update_time: DateTime<Utc>,
        _completed_migrations: &IndexerMigrations,
    ) -> Statement {
        blocks::Entity::find()
            .select_only()
            .filter(blocks::Column::Timestamp.ne(DateTime::UNIX_EPOCH))
            .filter(blocks::Column::Consensus.eq(true))
            .filter(interval_24h_filter(
                blocks::Column::Timestamp.into_simple_expr(),
                update_time,
            ))
            .expr_as(Func::count(Asterisk.into_column_ref()), "value")
            .build(DbBackend::Postgres)
    }
}

pub type NewBlocks24hInt =
    RemoteDatabaseSource<PullOneNowValue<NewBlocks24hStatement, NaiveDate, i64>>;

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
        IndexingStatus::LEAST_RESTRICTIVE
    }
}

pub type ArbitrumNewOperationalTxns24h = DirectPointLocalDbChartSource<
    Map<(NewBlocks24hInt, NewTxns24hInt), CalculateOperationalTxns<Properties>>,
    Properties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_arbitrum_new_operational_txns_24h() {
        simple_test_counter::<ArbitrumNewOperationalTxns24h>(
            "update_arbitrum_new_operational_txns_24h",
            "10",
            Some(dt("2022-11-11T00:00:00")),
        )
        .await;
    }
}
