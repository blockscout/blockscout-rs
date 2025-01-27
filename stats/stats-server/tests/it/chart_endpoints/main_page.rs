use blockscout_service_launcher::test_server::send_get_request;

use pretty_assertions::assert_eq;
use stats::lines::NEW_TXNS_WINDOW_RANGE;
use stats_proto::blockscout::stats::v1::MainPageStats;
use url::Url;

use crate::array_of_variables_with_names;

pub async fn test_main_page_ok(base: Url, expect_arbitrum: bool) {
    let main_page: MainPageStats = send_get_request(&base, "/api/v1/pages/main").await;
    let MainPageStats {
        average_block_time,
        total_addresses,
        total_blocks,
        total_transactions,
        yesterday_transactions,
        total_operational_transactions,
        yesterday_operational_transactions,
        daily_new_transactions,
        daily_new_operational_transactions,
    } = main_page;
    let mut counters = array_of_variables_with_names!([
        average_block_time,
        total_addresses,
        total_blocks,
        total_transactions,
        yesterday_transactions,
    ])
    .to_vec();
    if expect_arbitrum {
        counters.extend(array_of_variables_with_names!([
            total_operational_transactions,
            yesterday_operational_transactions,
        ]));
    }
    for (name, counter) in counters {
        #[allow(clippy::expect_fun_call)]
        let counter = counter.expect(&format!("page counter {} must be available", name));
        assert!(!counter.description.is_empty());
        assert!(!counter.title.is_empty());
    }

    let mut window_line_charts = array_of_variables_with_names!([daily_new_transactions]).to_vec();
    if expect_arbitrum {
        window_line_charts.extend(array_of_variables_with_names!([
            daily_new_operational_transactions
        ]));
    }
    for (name, window_chart) in window_line_charts {
        let window_chart = window_chart.expect(&format!("{} chart must be available", name));
        let transactions_info = window_chart.info.unwrap();
        assert!(!transactions_info.id.is_empty());
        assert_eq!(transactions_info.resolutions, vec!["DAY"]);
        assert_eq!(window_chart.chart.len(), NEW_TXNS_WINDOW_RANGE as usize);
    }
}
