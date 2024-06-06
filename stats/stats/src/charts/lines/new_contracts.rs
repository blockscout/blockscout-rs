use crate::data_source::kinds::{
    adapter::ParseAdapterWrapper, updateable_chart::clone::CloneChartWrapper,
};

/// Items in this module are not intended to be used outside. They are only public
/// since the actual public type is just an alias (to wrapper).
///
/// I.e. use [`super`]'s types.
pub mod _inner {
    use std::ops::RangeInclusive;

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
    use sea_orm::{prelude::DateTimeUtc, DbBackend, Statement};

    use super::NewContracts;

    pub struct NewContractsRemote;

    impl RemoteSource for NewContractsRemote {
        type Point = DateValueString;

        fn get_query(range: Option<RangeInclusive<DateTimeUtc>>) -> Statement {
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

    pub struct NewContractsInner;

    impl Named for NewContractsInner {
        const NAME: &'static str = "newContracts";
    }

    impl Chart for NewContractsInner {
        fn chart_type() -> ChartType {
            ChartType::Line
        }
    }

    impl CloneChart for NewContractsInner {
        type Dependency = RemoteSourceWrapper<NewContractsRemote>;
    }

    pub struct NewContractsIntInner;

    impl ParseAdapter for NewContractsIntInner {
        type InnerSource = NewContracts;
        type ParseInto = DateValueInt;
    }
}

pub type NewContracts = CloneChartWrapper<_inner::NewContractsInner>;
pub type NewContractsInt = ParseAdapterWrapper<_inner::NewContractsIntInner>;

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
