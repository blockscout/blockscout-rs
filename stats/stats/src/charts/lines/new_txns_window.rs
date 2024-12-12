//! New transactions for the last N days (usually 30).
//!
//! Allowed to work on a non-indexed networks, as it
//! recalculates whole N day window/range each time.
//!
//! Does not include last day, even as incomplete day.

use crate::{
    data_source::{
        kinds::{
            local_db::{
                parameters::{
                    update::batching::{
                        parameters::{BatchMaxDays, ClearAllAndPassStep},
                        BatchUpdate,
                    },
                    DefaultCreate, DefaultQueryVec,
                },
                LocalDbChartSource,
            },
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour, StatementFromRange},
        },
        types::BlockscoutMigrations,
        UpdateContext,
    },
    range::UniversalRange,
    types::{Timespan, TimespanDuration, TimespanValue},
    utils::day_start,
    ChartError, ChartProperties, Named,
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{FromQueryResult, Statement};

use super::NewTxnsStatement;

pub const WINDOW: u64 = 30;

fn new_txns_window_statement(
    update_day: NaiveDate,
    completed_migrations: &BlockscoutMigrations,
) -> Statement {
    // `update_day` is not included because the data would
    // be incomplete.
    let window =
        day_start(&update_day.saturating_sub(TimespanDuration::from_timespan_repeats(WINDOW)))
            ..day_start(&update_day);
    NewTxnsStatement::get_statement(Some(window), completed_migrations)
}

pub struct NewTxnsWindowQuery;

impl RemoteQueryBehaviour for NewTxnsWindowQuery {
    type Output = Vec<TimespanValue<NaiveDate, String>>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Vec<TimespanValue<NaiveDate, String>>, ChartError> {
        let update_day = cx.time.date_naive();
        let query = new_txns_window_statement(update_day, &cx.blockscout_applied_migrations);
        let mut data = TimespanValue::<NaiveDate, String>::find_by_statement(query)
            .all(cx.blockscout.connection.as_ref())
            .await
            .map_err(ChartError::BlockscoutDB)?;
        // linear time for sorted sequences
        data.sort_unstable_by(|a, b| a.timespan.cmp(&b.timespan));
        Ok(data)
    }
}

pub type NewTxnsWindowRemote = RemoteDatabaseSource<NewTxnsWindowQuery>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newTxnsWindow".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

pub type NewTxnsWindow = LocalDbChartSource<
    NewTxnsWindowRemote,
    (),
    DefaultCreate<Properties>,
    BatchUpdate<
        NewTxnsWindowRemote,
        (),
        ClearAllAndPassStep,
        BatchMaxDays,
        DefaultQueryVec<Properties>,
        Properties,
    >,
    DefaultQueryVec<Properties>,
    Properties,
>;

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        data_source::{DataSource, UpdateParameters},
        query_dispatch::QuerySerialized,
        tests::{
            mock_blockscout::{fill_mock_blockscout_data, imitate_reindex},
            point_construction::dt,
            simple_test::{chart_output_to_expected, map_str_tuple_to_owned, prepare_chart_test},
        },
        utils::MarkedDbConnection,
    };

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_window_clears_and_overwrites() {
        let (init_time, db, blockscout) =
            prepare_chart_test::<NewTxnsWindow>("update_txns_window_clears_and_overwrites", None)
                .await;
        {
            let current_date = init_time.date_naive();
            fill_mock_blockscout_data(&blockscout, current_date).await;
        }
        let current_time = dt("2022-12-01T00:00:00").and_utc();

        let mut parameters = UpdateParameters {
            db: &MarkedDbConnection::from_test_db(&db).unwrap(),
            blockscout: &MarkedDbConnection::from_test_db(&blockscout).unwrap(),
            blockscout_applied_migrations: BlockscoutMigrations::latest(),
            update_time_override: Some(current_time),
            force_full: false,
        };
        let cx = UpdateContext::from_params_now_or_override(parameters.clone());
        NewTxnsWindow::update_recursively(&cx).await.unwrap();
        assert_eq!(
            &chart_output_to_expected(
                NewTxnsWindow::query_data_static(&cx, UniversalRange::full(), None, false)
                    .await
                    .unwrap()
            ),
            &map_str_tuple_to_owned(vec![
                ("2022-11-09", "5"),
                ("2022-11-10", "12"),
                ("2022-11-11", "14"),
                ("2022-11-12", "5"),
                // update day is not included
            ]),
        );

        let current_time = dt("2022-12-10T00:00:00").and_utc();
        parameters.update_time_override = Some(current_time);
        let cx = UpdateContext::from_params_now_or_override(parameters.clone());
        NewTxnsWindow::update_recursively(&cx).await.unwrap();
        assert_eq!(
            &chart_output_to_expected(
                NewTxnsWindow::query_data_static(&cx, UniversalRange::full(), None, false)
                    .await
                    .unwrap()
            ),
            &map_str_tuple_to_owned(vec![
                // values outside the window are removed
                ("2022-11-10", "12"),
                ("2022-11-11", "14"),
                ("2022-11-12", "5"),
                ("2022-12-01", "5"),
            ]),
        );

        imitate_reindex(&blockscout, init_time.date_naive()).await;

        let current_time = dt("2022-12-11T00:00:00").and_utc();
        parameters.update_time_override = Some(current_time);
        let cx = UpdateContext::from_params_now_or_override(parameters);
        NewTxnsWindow::update_recursively(&cx).await.unwrap();
        assert_eq!(
            &chart_output_to_expected(
                NewTxnsWindow::query_data_static(&cx, UniversalRange::full(), None, false)
                    .await
                    .unwrap()
            ),
            &map_str_tuple_to_owned(vec![
                // values outside the window are removed
                // new values within the window are added
                ("2022-11-11", "18"),
                ("2022-11-12", "5"),
                ("2022-12-01", "5"),
            ]),
        );
    }
}
