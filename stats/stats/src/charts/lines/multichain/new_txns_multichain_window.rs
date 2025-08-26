use std::collections::HashSet;

use crate::{
    ChartError, ChartKey, ChartProperties, IndexingStatus, Named,
    charts::db_interaction::read::find_all_points,
    data_source::{
        UpdateContext,
        kinds::{
            data_manipulation::map::MapParseTo,
            local_db::{
                LocalDbChartSource,
                parameters::{
                    DefaultCreate, DefaultQueryVec, update::clear_and_query_all::ClearAllAndPassVec,
                },
            },
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour, StatementFromRange},
        },
        types::IndexerMigrations,
    },
    indexing_status::{BlockscoutIndexingStatus, IndexingStatusTrait, UserOpsIndexingStatus},
    lines::NEW_TXNS_WINDOW_RANGE,
    range::UniversalRange,
    types::{Timespan, TimespanDuration, TimespanValue, timespans::DateValue},
    utils::day_start,
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::Statement;

use super::new_txns_multichain::NewTxnsMultichainStatement;

fn new_txns_multichain_window_statement(
    update_day: NaiveDate,
    completed_migrations: &IndexerMigrations,
    enabled_update_charts_recursive: &HashSet<ChartKey>,
) -> Statement {
    // `update_day` is not included because the data would
    // be incomplete.
    let window = day_start(
        &update_day.saturating_sub(TimespanDuration::from_timespan_repeats(
            NEW_TXNS_WINDOW_RANGE,
        )),
    )..day_start(&update_day);
    NewTxnsMultichainStatement::get_statement(
        Some(window),
        completed_migrations,
        enabled_update_charts_recursive,
    )
}

pub struct NewTxnsMultichainWindowQuery;

impl RemoteQueryBehaviour for NewTxnsMultichainWindowQuery {
    type Output = Vec<TimespanValue<NaiveDate, String>>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Vec<TimespanValue<NaiveDate, String>>, ChartError> {
        let update_day = cx.time.date_naive();
        let statement = new_txns_multichain_window_statement(
            update_day,
            &cx.indexer_applied_migrations,
            &cx.enabled_update_charts_recursive,
        );
        find_all_points::<DateValue<String>>(cx, statement).await
    }
}

// should only be used in this chart for query efficiency.
// because is not directly stored in local DB.
pub type NewTxnsMultichainWindowRemote = RemoteDatabaseSource<NewTxnsMultichainWindowQuery>;
pub type NewTxnsMultichainWindowRemoteInt = MapParseTo<NewTxnsMultichainWindowRemote, i64>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newTxnsMultichainWindow".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }

    fn indexing_status_requirement() -> IndexingStatus {
        IndexingStatus {
            blockscout: BlockscoutIndexingStatus::NoneIndexed,
            user_ops: UserOpsIndexingStatus::LEAST_RESTRICTIVE,
        }
    }
}

pub type NewTxnsMultichainWindow = LocalDbChartSource<
    NewTxnsMultichainWindowRemote,
    (),
    DefaultCreate<Properties>,
    ClearAllAndPassVec<NewTxnsMultichainWindowRemote, DefaultQueryVec<Properties>, Properties>,
    DefaultQueryVec<Properties>,
    Properties,
>;

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        data_source::{DataSource, UpdateParameters},
        query_dispatch::QuerySerialized,
        tests::{
            mock_multichain::{add_mock_multichain_data, fill_mock_multichain_data},
            point_construction::dt,
            simple_test::{
                chart_output_to_expected, map_str_tuple_to_owned, prepare_multichain_chart_test,
            },
        },
    };

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_txns_window_multichain_clears_and_overwrites() {
        let (init_time, db, indexer) = prepare_multichain_chart_test::<NewTxnsMultichainWindow>(
            "update_new_txns_window_multichain_clears_and_overwrites",
            None,
        )
        .await;
        {
            let current_date = init_time.date_naive();
            fill_mock_multichain_data(&indexer, current_date).await;
        }
        let current_time = dt("2023-02-04T00:00:00").and_utc();

        let mut parameters = UpdateParameters {
            stats_db: &db,
            is_multichain_mode: true,
            indexer_db: &indexer,
            indexer_applied_migrations: IndexerMigrations::latest(),
            enabled_update_charts_recursive: NewTxnsMultichainWindow::all_dependencies_chart_keys(),
            update_time_override: Some(current_time),
            force_full: false,
        };
        let cx = UpdateContext::from_params_now_or_override(parameters.clone());
        NewTxnsMultichainWindow::update_recursively(&cx)
            .await
            .unwrap();
        assert_eq!(
            &chart_output_to_expected(
                NewTxnsMultichainWindow::query_data_static(
                    &cx,
                    UniversalRange::full(),
                    None,
                    false
                )
                .await
                .unwrap()
            ),
            &map_str_tuple_to_owned(vec![
                ("2023-02-02", "25"),
                ("2023-02-03", "49"),
                // update day is not included
            ]),
        );

        let current_time = dt("2023-03-05T00:00:00").and_utc();
        parameters.update_time_override = Some(current_time);
        let cx = UpdateContext::from_params_now_or_override(parameters.clone());
        NewTxnsMultichainWindow::update_recursively(&cx)
            .await
            .unwrap();
        assert_eq!(
            &chart_output_to_expected(
                NewTxnsMultichainWindow::query_data_static(
                    &cx,
                    UniversalRange::full(),
                    None,
                    false
                )
                .await
                .unwrap()
            ),
            &map_str_tuple_to_owned(vec![
                // values outside the window are removed
                ("2023-02-03", "49"),
                ("2023-02-04", "60"),
            ]),
        );

        add_mock_multichain_data(&indexer, NaiveDate::from_str("2023-03-05").unwrap()).await;
        let current_time = dt("2023-03-06T00:00:00").and_utc();
        parameters.update_time_override = Some(current_time);
        let cx = UpdateContext::from_params_now_or_override(parameters);
        NewTxnsMultichainWindow::update_recursively(&cx)
            .await
            .unwrap();
        assert_eq!(
            &chart_output_to_expected(
                NewTxnsMultichainWindow::query_data_static(
                    &cx,
                    UniversalRange::full(),
                    None,
                    false
                )
                .await
                .unwrap()
            ),
            &map_str_tuple_to_owned(vec![
                // values outside the window are removed
                // new values within the window are added
                ("2023-02-04", "60"),
                ("2023-03-05", "10"),
            ]),
        );
    }
}
