use blockscout_service_launcher::test_server::send_get_request;

use stats_proto::blockscout::stats::v1::TransactionsPageStats;
use url::Url;

use crate::array_of_variables_with_names;

pub async fn test_transactions_page_ok(base: Url) {
    let TransactionsPageStats {
        pending_transactions,
        transactions_fee_24h,
        average_transactions_fee_24h,
        transactions_24h,
    } = send_get_request(&base, "/api/v1/pages/transactions").await;
    let counters = array_of_variables_with_names!([
        pending_transactions,
        transactions_fee_24h,
        average_transactions_fee_24h,
        transactions_24h,
    ]);
    for (name, counter) in counters {
        #[allow(clippy::expect_fun_call)]
        let counter = counter.expect(&format!("page counter {} must be available", name));
        assert!(!counter.description.is_empty());
        assert!(!counter.title.is_empty());
    }
}
