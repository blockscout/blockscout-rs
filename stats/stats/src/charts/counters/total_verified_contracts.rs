use crate::{
    data_source::{
        kinds::{
            data_manipulation::map::MapToString,
            local_db::DirectPointLocalDbChartSource,
            remote_db::{PullOneValue, RemoteDatabaseSource, StatementFromUpdateTime},
        },
        types::BlockscoutMigrations,
    },
    ChartProperties, MissingDatePolicy, Named,
};

use blockscout_db::entity::smart_contracts;
use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{
    sea_query::{Asterisk, Func, IntoColumnRef},
    ColumnTrait, DbBackend, EntityTrait, QueryFilter, QuerySelect, QueryTrait, Statement,
};

pub struct TotalVerifiedContractsStatement;

impl StatementFromUpdateTime for TotalVerifiedContractsStatement {
    fn get_statement(
        update_time: DateTime<Utc>,
        _completed_migrations: &BlockscoutMigrations,
    ) -> Statement {
        smart_contracts::Entity::find()
            .select_only()
            .filter(smart_contracts::Column::InsertedAt.lte(update_time))
            .expr_as(Func::count(Asterisk.into_column_ref()), "value")
            .build(DbBackend::Postgres)
    }
}

pub type TotalVerifiedContractsRemote =
    RemoteDatabaseSource<PullOneValue<TotalVerifiedContractsStatement, NaiveDate, i64>>;

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
