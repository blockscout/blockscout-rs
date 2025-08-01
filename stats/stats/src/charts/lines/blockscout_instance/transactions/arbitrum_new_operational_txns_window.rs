//! New operational transactions for the last N days
//! (usually 30).
//!
//! Basically an extension of [super::NewTxnsWindow]
//! but for operational txns

use std::collections::HashSet;

use crate::{
    ChartError, ChartKey, ChartProperties, IndexingStatus, Named,
    charts::db_interaction::read::find_all_points,
    data_source::{
        UpdateContext,
        kinds::{
            data_manipulation::map::{Map, MapParseTo},
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
    lines::{NEW_TXNS_WINDOW_RANGE, NewBlocksStatement, NewTxnsWindowInt},
    range::UniversalRange,
    types::{Timespan, TimespanDuration, TimespanValue, timespans::DateValue},
    utils::day_start,
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::Statement;

use super::arbitrum_new_operational_txns::ArbitrumCalculateOperationalTxnsVec;

fn new_blocks_window_statement(
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
    NewBlocksStatement::get_statement(
        Some(window),
        completed_migrations,
        enabled_update_charts_recursive,
    )
}

pub struct NewBlocksWindowQuery;

impl RemoteQueryBehaviour for NewBlocksWindowQuery {
    type Output = Vec<TimespanValue<NaiveDate, String>>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Vec<TimespanValue<NaiveDate, String>>, ChartError> {
        let update_day = cx.time.date_naive();
        let statement = new_blocks_window_statement(
            update_day,
            &cx.indexer_applied_migrations,
            &cx.enabled_update_charts_recursive,
        );
        find_all_points::<DateValue<String>>(cx, statement).await
    }
}

// should only be used in this chart for query efficiency.
// because is not directly stored in local DB.
pub type NewBlocksWindowRemote = RemoteDatabaseSource<NewBlocksWindowQuery>;
pub type NewBlocksWindowRemoteInt = MapParseTo<NewBlocksWindowRemote, i64>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newOperationalTxnsWindow".into()
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

pub type ArbitrumNewOperationalTxnsWindowCalculation =
    Map<(NewBlocksWindowRemoteInt, NewTxnsWindowInt), ArbitrumCalculateOperationalTxnsVec>;
pub type ArbitrumNewOperationalTxnsWindow = LocalDbChartSource<
    ArbitrumNewOperationalTxnsWindowCalculation,
    (),
    DefaultCreate<Properties>,
    ClearAllAndPassVec<
        ArbitrumNewOperationalTxnsWindowCalculation,
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
    };

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_arbitrum_operational_txns_window_clears_and_overwrites() {
        let (init_time, db, blockscout) = prepare_chart_test::<ArbitrumNewOperationalTxnsWindow>(
            "update_arbitrum_operational_txns_window_clears_and_overwrites",
            None,
        )
        .await;
        {
            let current_date = init_time.date_naive();
            fill_mock_blockscout_data(&blockscout, current_date).await;
        }
        let current_time = dt("2022-12-01T00:00:00").and_utc();

        let mut parameters = UpdateParameters {
            stats_db: &db,
            is_multichain_mode: false,
            indexer_db: &blockscout,
            indexer_applied_migrations: IndexerMigrations::latest(),
            enabled_update_charts_recursive:
                ArbitrumNewOperationalTxnsWindow::all_dependencies_chart_keys(),
            update_time_override: Some(current_time),
            force_full: false,
        };
        let cx = UpdateContext::from_params_now_or_override(parameters.clone());
        ArbitrumNewOperationalTxnsWindow::update_recursively(&cx)
            .await
            .unwrap();
        assert_eq!(
            &chart_output_to_expected(
                ArbitrumNewOperationalTxnsWindow::query_data_static(
                    &cx,
                    UniversalRange::full(),
                    None,
                    false
                )
                .await
                .unwrap()
            ),
            &map_str_tuple_to_owned(vec![
                ("2022-11-09", "5"),
                ("2022-11-10", "11"),
                ("2022-11-11", "12"),
                ("2022-11-12", "5"),
                // update day is not included
            ]),
        );

        let current_time = dt("2022-12-10T00:00:00").and_utc();
        parameters.update_time_override = Some(current_time);
        let cx = UpdateContext::from_params_now_or_override(parameters.clone());
        ArbitrumNewOperationalTxnsWindow::update_recursively(&cx)
            .await
            .unwrap();
        assert_eq!(
            &chart_output_to_expected(
                ArbitrumNewOperationalTxnsWindow::query_data_static(
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
                ("2022-11-10", "11"),
                ("2022-11-11", "12"),
                ("2022-11-12", "5"),
                ("2022-12-01", "5"),
            ]),
        );

        imitate_reindex(&blockscout, init_time.date_naive()).await;

        let current_time = dt("2022-12-11T00:00:00").and_utc();
        parameters.update_time_override = Some(current_time);
        let cx = UpdateContext::from_params_now_or_override(parameters);
        ArbitrumNewOperationalTxnsWindow::update_recursively(&cx)
            .await
            .unwrap();
        assert_eq!(
            &chart_output_to_expected(
                ArbitrumNewOperationalTxnsWindow::query_data_static(
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
                ("2022-11-11", "16"),
                ("2022-11-12", "5"),
                ("2022-12-01", "5"),
            ]),
        );
    }
}
