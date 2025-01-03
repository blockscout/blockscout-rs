use blockscout_service_launcher::test_server::send_get_request;

use stats_proto::blockscout::stats::v1::TransactionsPageStats;
use url::Url;

pub async fn test_transactions_page_ok(base: Url) {
    let TransactionsPageStats {
        pending_txns,
        txns_fee_24h,
        average_txn_fee_24h,
        total_txns,
    } = send_get_request(&base, "/api/v1/pages/transactions").await;
    let counters = [
        ("pending_txns", pending_txns),
        ("txns_fee_24h", txns_fee_24h),
        ("average_txn_fee_24h", average_txn_fee_24h),
        ("total_txns", total_txns),
    ];
    for (name, counter) in counters {
        #[allow(clippy::expect_fun_call)]
        let counter = counter.expect(&format!("page counter {} must be available", name));
        assert!(!counter.description.is_empty());
        assert!(!counter.title.is_empty());
    }
}
