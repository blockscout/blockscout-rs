//! New transactions for the last N days (usually 30).
//!
//! Allowed to work on a non-indexed networks, as it
//! recalculates whole N day window/range each time.
//!
//! Does not include last day, even as incomplete day.

use crate::{
    data_source::kinds::{
        data_manipulation::map::{Map, MapParseTo, StripExt},
        local_db::{
            parameters::{
                update::clear_and_query_all::ClearAllAndPassVec, DefaultCreate, DefaultQueryVec,
            },
            LocalDbChartSource,
        },
    },
    indexing_status::{BlockscoutIndexingStatus, IndexingStatusTrait, UserOpsIndexingStatus},
    types::new_txns::ExtractAllTxns,
    ChartProperties, IndexingStatus, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

use super::NewTxnsWindowCombinedRemote;

pub type NewTxnsWindowRemote = Map<NewTxnsWindowCombinedRemote, ExtractAllTxns>;

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
    fn indexing_status_requirement() -> IndexingStatus {
        IndexingStatus {
            blockscout: BlockscoutIndexingStatus::NoneIndexed,
            user_ops: UserOpsIndexingStatus::LEAST_RESTRICTIVE,
        }
    }
}

pub type NewTxnsWindow = LocalDbChartSource<
    NewTxnsWindowRemote,
    (),
    DefaultCreate<Properties>,
    ClearAllAndPassVec<NewTxnsWindowRemote, DefaultQueryVec<Properties>, Properties>,
    DefaultQueryVec<Properties>,
    Properties,
>;
pub type NewTxnsWindowInt = MapParseTo<StripExt<NewTxnsWindow>, i64>;

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        data_source::{types::BlockscoutMigrations, DataSource, UpdateContext, UpdateParameters},
        query_dispatch::QuerySerialized,
        range::UniversalRange,
        tests::{
            mock_blockscout::{fill_mock_blockscout_data, imitate_reindex},
            point_construction::dt,
            simple_test::{chart_output_to_expected, map_str_tuple_to_owned, prepare_chart_test},
        },
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
            db: &db,
            blockscout: &blockscout,
            blockscout_applied_migrations: BlockscoutMigrations::latest(),
            enabled_update_charts_recursive: NewTxnsWindow::all_dependencies_chart_keys(),
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
                ("2022-11-09", "6"),
                ("2022-11-10", "14"),
                ("2022-11-11", "16"),
                ("2022-11-12", "6"),
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
                ("2022-11-10", "14"),
                ("2022-11-11", "16"),
                ("2022-11-12", "6"),
                ("2022-12-01", "6"),
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
                ("2022-11-11", "20"),
                ("2022-11-12", "6"),
                ("2022-12-01", "6"),
            ]),
        );
    }
}
