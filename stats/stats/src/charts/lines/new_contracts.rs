use std::ops::RangeInclusive;

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
use sea_orm::{prelude::DateTimeUtc, DbBackend, Statement};

pub struct NewContractsRemote;

impl RemoteSource for NewContractsRemote {
    type Point = DateValueString;

    fn get_query(range: Option<RangeInclusive<DateTimeUtc>>) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"SELECT day AS date, COUNT(*)::text AS value
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

pub struct NewAccountsInner;

impl Named for NewAccountsInner {
    const NAME: &'static str = "newContracts";
}

impl Chart for NewAccountsInner {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

impl CloneChart for NewAccountsInner {
    type Dependency = RemoteSourceWrapper<NewContractsRemote>;
}

pub type NewContracts = CloneChartWrapper<NewAccountsInner>;

pub struct NewContractsIntInner;

impl ParseAdapter for NewContractsIntInner {
    type InnerSource = NewContracts;
    type ParseInto = DateValueInt;
}

pub type NewContractsInt = ParseAdapterWrapper<NewContractsIntInner>;

#[cfg(test)]
mod tests {
    use super::NewContracts;
    use crate::tests::simple_test::simple_test_chart;

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
}
