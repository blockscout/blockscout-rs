use blockscout_service_launcher::test_server::send_get_request;

use pretty_assertions::assert_eq;
use stats::{
    lines::{NewTxnsWindow, NEW_TXNS_WINDOW_RANGE},
    Named,
};
use stats_proto::blockscout::stats::v1::MainPageStats;
use url::Url;

use crate::array_of_variables_with_names;

pub async fn test_main_page_ok(base: Url) {
    let main_page: MainPageStats = send_get_request(&base, "/api/v1/pages/main").await;
    let MainPageStats {
        average_block_time,
        total_addresses,
        total_blocks,
        total_transactions,
        yesterday_transactions,
        daily_new_transactions,
    } = main_page;
    let counters = array_of_variables_with_names!([
        average_block_time,
        total_addresses,
        total_blocks,
        total_transactions,
        yesterday_transactions,
    ]);
    for (name, counter) in counters {
        #[allow(clippy::expect_fun_call)]
        let counter = counter.expect(&format!("page counter {} must be available", name));
        assert!(!counter.description.is_empty());
        assert!(!counter.title.is_empty());
    }

    let daily_new_transactions =
        daily_new_transactions.expect("daily new transactions chart must be available");
    let transactions_info = daily_new_transactions.info.unwrap();
    assert_eq!(transactions_info.id, NewTxnsWindow::name());
    assert_eq!(transactions_info.resolutions, vec!["DAY"]);
    assert_eq!(
        daily_new_transactions.chart.len(),
        NEW_TXNS_WINDOW_RANGE as usize
    );
}
