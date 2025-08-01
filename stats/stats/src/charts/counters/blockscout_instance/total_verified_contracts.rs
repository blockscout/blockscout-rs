use crate::{
    ChartProperties, IndexingStatus, MissingDatePolicy, Named,
    data_source::{
        kinds::{
            data_manipulation::map::MapToString,
            local_db::DirectPointLocalDbChartSource,
            remote_db::{PullOneNowValue, RemoteDatabaseSource, StatementFromUpdateTime},
        },
        types::IndexerMigrations,
    },
    indexing_status::{BlockscoutIndexingStatus, IndexingStatusTrait, UserOpsIndexingStatus},
};

use blockscout_db::entity::smart_contracts;
use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{
    ColumnTrait, DbBackend, EntityTrait, QueryFilter, QuerySelect, QueryTrait, Statement,
    sea_query::{Asterisk, Func, IntoColumnRef},
};

pub struct TotalVerifiedContractsStatement;

impl StatementFromUpdateTime for TotalVerifiedContractsStatement {
    fn get_statement(
        update_time: DateTime<Utc>,
        _completed_migrations: &IndexerMigrations,
    ) -> Statement {
        smart_contracts::Entity::find()
            .select_only()
            .filter(smart_contracts::Column::InsertedAt.lte(update_time))
            .expr_as(Func::count(Asterisk.into_column_ref()), "value")
            .build(DbBackend::Postgres)
    }
}

pub type TotalVerifiedContractsRemote =
    RemoteDatabaseSource<PullOneNowValue<TotalVerifiedContractsStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalVerifiedContracts".into()
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
        IndexingStatus {
            blockscout: BlockscoutIndexingStatus::NoneIndexed,
            user_ops: UserOpsIndexingStatus::LEAST_RESTRICTIVE,
        }
    }
}

pub type TotalVerifiedContracts =
    DirectPointLocalDbChartSource<MapToString<TotalVerifiedContractsRemote>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_verified_contracts() {
        simple_test_counter::<TotalVerifiedContracts>("update_total_verified_contracts", "3", None)
            .await;
    }
}
