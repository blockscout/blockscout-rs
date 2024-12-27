use blockscout_service_launcher::test_server::send_get_request;

use stats_proto::blockscout::stats::v1::ContractsPageStats;
use url::Url;

pub async fn test_contracts_page_ok(base: Url) {
    let ContractsPageStats {
        total_contracts,
        new_contracts_24h,
        total_verified_contracts,
        new_verified_contracts_24h,
    } = send_get_request(&base, "/api/v1/pages/contracts").await;
    let counters = [
        ("total_contracts", total_contracts),
        ("new_contracts_24h", new_contracts_24h),
        ("total_verified_contracts", total_verified_contracts),
        ("new_verified_contracts_24h", new_verified_contracts_24h),
    ];
    for (name, counter) in counters {
        #[allow(clippy::expect_fun_call)]
        let counter = counter.expect(&format!("page counter {} must be available", name));
        assert!(!counter.description.is_empty());
        assert!(!counter.title.is_empty());
    }
}
