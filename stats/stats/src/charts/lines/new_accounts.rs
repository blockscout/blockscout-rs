use crate::{
    charts::db_interaction::types::DateValueInt,
    data_source::kinds::{adapter::parse::MapParseTo, updateable_chart::clone::CloneChartWrapper},
};

mod _inner {
    use std::ops::RangeInclusive;

    use crate::{
        charts::db_interaction::types::DateValueInt,
        data_source::{
            kinds::{
                adapter::to_string::MapToString,
                remote_db::{QueryBehaviour, RemoteDatabaseSource, StatementFromRange},
                updateable_chart::clone::CloneChart,
            },
            UpdateContext,
        },
        missing_date::trim_out_of_range_sorted,
        Chart, Named, UpdateError,
    };
    use chrono::Duration;
    use entity::sea_orm_active_enums::ChartType;
    use sea_orm::{prelude::*, DbBackend, FromQueryResult, Statement};

    pub struct NewAccountsStatement;

    impl StatementFromRange for NewAccountsStatement {
        fn get_statement(range: Option<RangeInclusive<DateTimeUtc>>) -> Statement {
            // we want to consider the time at range end; thus optional
            // filter
            if let Some(range) = range {
                Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    r#"
                        SELECT
                            first_tx.date as date,
                            count(*) as value
                        FROM (
                            SELECT DISTINCT ON (t.from_address_hash)
                                b.timestamp::date as date
                            FROM transactions  t
                            JOIN blocks        b ON t.block_hash = b.hash
                            WHERE
                                b.timestamp != to_timestamp(0) AND
                                b.consensus = true AND
                                b.timestamp <= $1
                            ORDER BY t.from_address_hash, b.timestamp
                        ) first_tx
                        GROUP BY first_tx.date;
                    "#,
                    vec![(*range.end()).into()],
                )
            } else {
                Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    r#"
                        SELECT
                            first_tx.date as date,
                            count(*) as value
                        FROM (
                            SELECT DISTINCT ON (t.from_address_hash)
                                b.timestamp::date as date
                            FROM transactions  t
                            JOIN blocks        b ON t.block_hash = b.hash
                            WHERE
                                b.timestamp != to_timestamp(0) AND
                                b.consensus = true
                            ORDER BY t.from_address_hash, b.timestamp
                        ) first_tx
                        GROUP BY first_tx.date;
                    "#,
                    vec![],
                )
            }
        }
    }

    pub struct NewAccountsQueryBehaviour;

    impl QueryBehaviour for NewAccountsQueryBehaviour {
        type Output = Vec<DateValueInt>;

        async fn query_data(
            cx: &UpdateContext<'_>,
            range: Option<RangeInclusive<DateTimeUtc>>,
        ) -> Result<Vec<DateValueInt>, UpdateError> {
            let query = NewAccountsStatement::get_statement(range.clone());
            let mut data = DateValueInt::find_by_statement(query)
                .all(cx.blockscout)
                .await
                .map_err(UpdateError::BlockscoutDB)?;
            // make sure that it's sorted
            data.sort_by_key(|d| d.date);
            if let Some(range) = range {
                let range = range.start().date_naive()..=range.end().date_naive();
                trim_out_of_range_sorted(&mut data, range);
            }
            Ok(data)
        }
    }

    /// Note:  The intended strategy is to update whole range at once, even
    /// though the implementation allows batching. The batching was done
    /// to simplify interface of the data source.
    ///
    /// Thus, use max batch size in the dependant data sources.
    pub type NewAccountsRemote = RemoteDatabaseSource<NewAccountsQueryBehaviour>;

    pub type NewAccountsRemoteString = MapToString<NewAccountsRemote>;

    pub struct NewAccountsInner;

    impl Named for NewAccountsInner {
        const NAME: &'static str = "newAccounts";
    }

    impl Chart for NewAccountsInner {
        fn chart_type() -> ChartType {
            ChartType::Line
        }
    }

    impl CloneChart for NewAccountsInner {
        type Dependency = NewAccountsRemoteString;

        fn batch_size() -> Duration {
            // see `NewAccountsRemote` docs
            Duration::max_value()
        }
    }
}

pub type NewAccounts = CloneChartWrapper<_inner::NewAccountsInner>;
pub type NewAccountsInt = MapParseTo<NewAccounts, DateValueInt>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::{ranged_test_chart, simple_test_chart};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_accounts() {
        simple_test_chart::<NewAccounts>(
            "update_new_accounts",
            vec![
                ("2022-11-09", "1"),
                ("2022-11-10", "3"),
                ("2022-11-11", "4"),
                ("2023-03-01", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn ranged_update_new_accounts() {
        ranged_test_chart::<NewAccounts>(
            "ranged_update_new_accounts",
            vec![
                ("2022-11-09", "1"),
                ("2022-11-10", "3"),
                // only half of the tx's should be detected; others were created
                // later than update time
                ("2022-11-11", "2"),
            ],
            "2022-11-09".parse().unwrap(),
            "2022-11-11".parse().unwrap(),
            Some("2022-11-11T14:00:00".parse().unwrap()),
        )
        .await;
    }
}
