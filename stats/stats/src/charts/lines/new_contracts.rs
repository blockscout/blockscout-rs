use std::ops::Range;

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
use sea_orm::{prelude::DateTimeUtc, DbBackend, Statement};

pub struct NewContractsStatement;

impl StatementFromRange for NewContractsStatement {
    fn get_statement(range: Option<Range<DateTimeUtc>>) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT day AS date, COUNT(*)::text AS value
                FROM (
                    SELECT 
                        DISTINCT ON (txns_plus_internal_txns.hash)
                        txns_plus_internal_txns.day
                    FROM (
                        SELECT
                            t.created_contract_address_hash AS hash,
                            b.timestamp::date AS day
                        FROM transactions t
                            JOIN blocks b ON b.hash = t.block_hash
                        WHERE
                            t.created_contract_address_hash NOTNULL AND
                            b.consensus = TRUE AND
                            b.timestamp != to_timestamp(0) {filter}
                        UNION
                        SELECT
                            it.created_contract_address_hash AS hash,
                            b.timestamp::date AS day
                        FROM internal_transactions it
                            JOIN blocks b ON b.hash = it.block_hash
                        WHERE
                            it.created_contract_address_hash NOTNULL AND
                            b.consensus = TRUE AND
                            b.timestamp != to_timestamp(0) {filter}
                    ) txns_plus_internal_txns
                ) sub
                GROUP BY sub.day;
            "#,
            [],
            "b.timestamp",
            range,
        )
    }
}

pub type NewContractsRemote =
    RemoteDatabaseSource<PullAllWithAndSort<NewContractsStatement, DateValueString>>;

pub struct NewContractsProperties;

impl Named for NewContractsProperties {
    const NAME: &'static str = "newContracts";
}

impl ChartProperties for NewContractsProperties {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

pub type NewContracts = DirectVecLocalDbChartSource<NewContractsRemote, NewContractsProperties>;
pub type NewContractsInt = MapParseTo<NewContracts, DateValueInt>;

#[cfg(test)]
mod tests {
    use super::NewContracts;
    use crate::tests::{
        point_construction::{d, dt},
        simple_test::{ranged_test_chart, simple_test_chart},
    };

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_contracts() {
        simple_test_chart::<NewContracts>(
            "update_new_contracts",
            vec![
                ("2022-11-09", "3"),
                ("2022-11-10", "6"),
                ("2022-11-11", "8"),
                ("2022-11-12", "2"),
                ("2022-12-01", "2"),
                ("2023-01-01", "1"),
                ("2023-02-01", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn ranged_update_new_contracts() {
        ranged_test_chart::<NewContracts>(
            "ranged_update_new_contracts",
            vec![
                ("2022-11-11", "8"),
                ("2022-11-12", "2"),
                ("2022-12-01", "2"),
            ],
            d("2022-11-11"),
            d("2022-12-01"),
            Some(dt("2022-12-01T12:00:00")),
        )
        .await;
    }
}
