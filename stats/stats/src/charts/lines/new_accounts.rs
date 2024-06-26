use std::ops::RangeInclusive;

use crate::{
    charts::db_interaction::types::DateValueInt,
    data_source::{
        kinds::{
            data_manipulation::map::MapParseTo,
            local_db::{
                parameters::{
                    update::batching::{
                        parameters::{BatchMax, PassVecStep},
                        BatchUpdate,
                    },
                    DefaultCreate, DefaultQueryVec,
                },
                LocalDbChartSource,
            },
            remote_db::{QueryBehaviour, RemoteDatabaseSource, StatementFromRange},
        },
        UpdateContext,
    },
    missing_date::trim_out_of_range_sorted,
    utils::sql_with_range_filter_opt,
    ChartProperties, DateValueString, Named, UpdateError,
};

use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, FromQueryResult, Statement};

pub struct NewAccountsStatement;

impl StatementFromRange for NewAccountsStatement {
    fn get_statement(range: Option<RangeInclusive<DateTimeUtc>>) -> Statement {
        // `MIN_UTC` does not fit into postgres' timestamp. Unix epoch start should be enough
        let min_timestamp = DateTimeUtc::UNIX_EPOCH;
        // All transactions from the beginning must be considered to calculate new accounts correctly.
        // E.g. if account was first active both before `range.start()` and within the range,
        // we don't want to count it within the range (as it's not a *new* account).
        let range = range.map(|r| (min_timestamp..=r.into_inner().1));
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT
                    first_tx.date as date,
                    count(*)::TEXT as value
                FROM (
                    SELECT DISTINCT ON (t.from_address_hash)
                        b.timestamp::date as date
                    FROM transactions  t
                    JOIN blocks        b ON t.block_hash = b.hash
                    WHERE
                        b.timestamp != to_timestamp(0) AND
                        b.consensus = true {filter}
                    ORDER BY t.from_address_hash, b.timestamp
                ) first_tx
                GROUP BY first_tx.date;
            "#,
            [],
            "b.timestamp",
            range
        )
    }
}

pub struct NewAccountsQueryBehaviour;

impl QueryBehaviour for NewAccountsQueryBehaviour {
    type Output = Vec<DateValueString>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: Option<RangeInclusive<DateTimeUtc>>,
    ) -> Result<Vec<DateValueString>, UpdateError> {
        let query = NewAccountsStatement::get_statement(range.clone());
        let mut data = DateValueString::find_by_statement(query)
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

pub struct Properties;

impl Named for Properties {
    const NAME: &'static str = "newAccounts";
}

impl ChartProperties for Properties {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

pub type NewAccounts = LocalDbChartSource<
    NewAccountsRemote,
    (),
    DefaultCreate<Properties>,
    BatchUpdate<
        NewAccountsRemote,
        (),
        PassVecStep,
        // see `NewAccountsRemote` docs
        BatchMax,
        Properties,
    >,
    DefaultQueryVec<Properties>,
    Properties,
>;

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
