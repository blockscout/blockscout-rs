use blockscout_service_launcher::test_server::send_get_request;
use pretty_assertions::assert_eq;

use stats_proto::blockscout::stats::v1::Counters;
use url::Url;

pub async fn test_counters_ok(base: Url) {
    let counters: Counters = send_get_request(&base, "/api/v1/counters").await;
    for counter in counters.counters.iter() {
        assert!(!counter.description.is_empty());
        assert!(!counter.title.is_empty());
    }
    // Also check the order set in layout
    let counter_names: Vec<_> = counters.counters.iter().map(|c| c.id.as_str()).collect();
    let expected_counter_names: Vec<_> = [
        "averageBlockTime",
        "completedTxns",
        "lastNewContracts",
        "lastNewVerifiedContracts",
        "totalAccounts",
        "totalAddresses",
        "totalBlocks",
        "totalContracts",
        // "totalNativeCoinHolders", // disabled
        "totalNativeCoinTransfers",
        "totalTokens",
        "totalTxns",
        "totalOperationalTxns",
        "totalUserOps",
        "totalVerifiedContracts",
        "newTxns24h",
        "pendingTxns30m",
        "txnsFee24h",
        "averageTxnFee24h",
        // on a different page; they are checked by other endpoint tests and
        // `check_all_enabled_charts_have_endpoints`

        // "newContracts24h",
        // "newVerifiedContracts24h",
        // "yesterdayTxns",
    ]
    .into_iter()
    .collect();

    assert_eq!(counter_names, expected_counter_names);
}
