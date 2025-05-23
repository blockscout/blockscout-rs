//! Tests for fully initialized blockscout without reindexing.
//!
//! The tests also must not change the state of blockscout db.

use std::str::FromStr;

use blockscout_service_launcher::test_database::TestDbGuard;
use chrono::NaiveDate;
use stats::tests::{init_db::init_db_blockscout, mock_blockscout::fill_mock_blockscout_data};
use tokio::sync::OnceCell;

mod common_tests;
mod stats_full;
mod stats_no_specific;
mod stats_not_indexed;
mod stats_not_updated;

static MOCK_BLOCKSCOUT: OnceCell<TestDbGuard> = OnceCell::const_new();

/// All tests using this must not change the state of blockscout db.
async fn get_mock_blockscout() -> &'static TestDbGuard {
    MOCK_BLOCKSCOUT
        .get_or_init(|| async {
            let test_name = "tests_with_mock_blockscout";
            let blockscout_db = init_db_blockscout(test_name).await;
            fill_mock_blockscout_data(&blockscout_db, NaiveDate::from_str("2023-03-01").unwrap())
                .await;
            blockscout_db
        })
        .await
}
