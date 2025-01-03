use blockscout_service_launcher::test_server::send_get_request;

use pretty_assertions::assert_eq;
use stats::{
    lines::{NewTxnsWindow, NEW_TXNS_WINDOW_RANGE},
    Named,
};
use stats_proto::blockscout::stats::v1::MainPageStats;
use url::Url;

pub async fn test_main_page_ok(base: Url) {
    let main_page: MainPageStats = send_get_request(&base, "/api/v1/pages/main").await;
    let MainPageStats {
        average_block_time,
        total_addresses,
        total_blocks,
        total_txns,
        yesterday_txns,
        transactions,
    } = main_page;
    let counters = [
        ("average_block_time", average_block_time),
        ("total_addresses", total_addresses),
        ("total_blocks", total_blocks),
        ("total_txns", total_txns),
        ("yesterday_txns", yesterday_txns),
    ];
    for (name, counter) in counters {
        #[allow(clippy::expect_fun_call)]
        let counter = counter.expect(&format!("page counter {} must be available", name));
        assert!(!counter.description.is_empty());
        assert!(!counter.title.is_empty());
    }

    let transactions = transactions.expect("daily transactions chart must be available");
    let transactions_info = transactions.info.unwrap();
    assert_eq!(transactions_info.id, NewTxnsWindow::name());
    assert_eq!(transactions_info.resolutions, vec!["DAY"]);
    assert_eq!(transactions.chart.len(), NEW_TXNS_WINDOW_RANGE as usize);
}
