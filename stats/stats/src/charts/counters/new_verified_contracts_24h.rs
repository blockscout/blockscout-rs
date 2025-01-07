use crate::{
    charts::db_interaction::utils::interval_24h_filter,
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
use migration::{Asterisk, Func, IntoColumnRef};
use sea_orm::{prelude::*, DbBackend, IntoSimpleExpr, QuerySelect, QueryTrait};

pub struct NewVerifiedContracts24hStatement;

impl StatementFromUpdateTime for NewVerifiedContracts24hStatement {
    fn get_statement(
        update_time: DateTime<Utc>,
        _completed_migrations: &BlockscoutMigrations,
    ) -> sea_orm::Statement {
        smart_contracts::Entity::find()
            .select_only()
            .filter(interval_24h_filter(
                smart_contracts::Column::InsertedAt.into_simple_expr(),
                update_time,
            ))
            .expr_as(Func::count(Asterisk.into_column_ref()), "value")
            .build(DbBackend::Postgres)
    }
}

pub type NewVerifiedContracts24hRemote =
    RemoteDatabaseSource<PullOneValue<NewVerifiedContracts24hStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newVerifiedContracts24h".into()
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

pub type NewVerifiedContracts24h =
    DirectPointLocalDbChartSource<MapToString<NewVerifiedContracts24hRemote>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_verified_contracts_24h() {
        simple_test_counter::<NewVerifiedContracts24h>(
            "update_new_verified_contracts_24h",
            "1",
            Some(dt("2022-11-16T6:30:00")),
        )
        .await;
    }
}
