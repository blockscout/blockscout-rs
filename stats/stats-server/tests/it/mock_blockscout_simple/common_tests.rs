//! Since the files are organized according to
//! the initialization requirements, the actual testing code
//! is reused between these cases

use std::collections::HashMap;

use blockscout_service_launcher::test_server::send_get_request;
use pretty_assertions::assert_eq;
use stats::{lines::NEW_TXNS_WINDOW_RANGE, ResolutionKind};
use stats_proto::blockscout::stats::v1::{
    ContractsPageStats, Counters, MainPageStats, TransactionsPageStats,
};
use url::Url;

use crate::{
    array_of_variables_with_names,
    common::{enabled_resolutions, sorted_vec},
};

pub async fn test_lines_ok(base: Url, blockscout_indexed: bool, user_ops_indexed: bool) {
    let line_charts: stats_proto::blockscout::stats::v1::LineCharts =
        send_get_request(&base, "/api/v1/lines").await;

    let section_ids: Vec<&str> = line_charts
        .sections
        .iter()
        .map(|sec| sec.id.as_str())
        .collect();
    let expected_section_ids = [
        "accounts",
        "transactions",
        "blocks",
        "tokens",
        "gas",
        "contracts",
        "user_ops",
    ];
    assert_eq!(section_ids, expected_section_ids, "wrong sections response");

    let mut enabled_resolutions = enabled_resolutions(line_charts).await;

    let mut expected_lines = vec![];
    if blockscout_indexed {
        expected_lines.extend([
            // disabled charts are commented out
            "accountsGrowth",
            "activeAccounts",
            // "activeRecurringAccounts60Days",
            // "activeRecurringAccounts90Days",
            // "activeRecurringAccounts120Days",
            "averageBlockSize",
            "averageBlockRewards",
            "newAccounts",
            "averageGasLimit",
            "averageGasPrice",
            "averageTxnFee",
            "gasUsedGrowth",
            "averageGasUsed",
            "networkUtilization",
            // "nativeCoinHoldersGrowth",
            // "nativeCoinSupply",
            // "newNativeCoinHolders",
            "newBlocks",
            "newNativeCoinTransfers",
            "newTxns",
            "txnsFee",
            "txnsGrowth",
            "opStackNewOperationalTxns",
            "opStackOperationalTxnsGrowth",
            "newOperationalTxns",
            "operationalTxnsGrowth",
            "newEip7702Auths",
            "eip7702AuthsGrowth",
            "txnsSuccessRate",
            "newVerifiedContracts",
            "newContracts",
            "verifiedContractsGrowth",
            "contractsGrowth",
        ]);
    }
    if user_ops_indexed {
        expected_lines.extend([
            "newUserOps",
            "userOpsGrowth",
            "newAccountAbstractionWallets",
            "accountAbstractionWalletsGrowth",
            "activeAccountAbstractionWallets",
            "activeBundlers",
            "activePaymasters",
        ]);
    }

    for line_name in expected_lines {
        let line_resolutions = enabled_resolutions
            .remove(line_name)
            .unwrap_or_else(|| panic!("must return chart info for {}", &line_name));
        assert!(
            line_resolutions.contains(&ResolutionKind::Day.into()),
            "At least day resolution must be enabled for enabled chart `{}`. Enabled resolutions: {:?}",
            &line_name, line_resolutions
        );
        for resolution in line_resolutions {
            let chart: serde_json::Value = send_get_request(
                &base,
                &format!("/api/v1/lines/{line_name}?resolution={resolution}"),
            )
            .await;
            let chart_data = chart
                .as_object()
                .expect("response has to be json object")
                .get("chart")
                .expect("response doesn't have 'chart' field")
                .as_array()
                .expect("'chart' field has to be json array");

            assert!(
                !chart_data.is_empty(),
                "chart data for '{line_name}' '{resolution}' is empty"
            );

            let info = chart
                .get("info")
                .expect("response doesn't have 'info' field");
            let info: stats_proto::blockscout::stats::v1::LineChartInfo =
                serde_json::from_value(info.clone()).expect("must return valid chart info");
            assert_eq!(
                info.id, line_name,
                "returned chart id (left) doesn't match requested (right)",
            )
        }
        // should work even without `resolution` parameter
        let _chart: serde_json::Value =
            send_get_request(&base, &format!("/api/v1/lines/{line_name}")).await;
    }

    // should not return charts that are disabled or waiting for indexing
    assert_eq!(enabled_resolutions, HashMap::new());
}

pub async fn test_counters_ok(base: Url, blockscout_indexed: bool, user_ops_indexed: bool) {
    let counters: Counters = send_get_request(&base, "/api/v1/counters").await;
    for counter in counters.counters.iter() {
        assert!(!counter.description.is_empty());
        assert!(!counter.title.is_empty());
    }
    // Also check the order set in layout
    let counter_names: Vec<_> = counters.counters.iter().map(|c| c.id.as_str()).collect();

    // always present counters are
    // - from main/tx/contracts pages
    //  (that are also available as regular counters)
    // - with fallback query logic
    //  (they are returned even without calling an update)
    //  (they all coincide with the previous case)
    // - that are valid w/o fully indexed blockscout
    let mut expected_counter_names: Vec<_> = vec![
        // main page
        "averageBlockTime",
        "totalAddresses",
        "totalBlocks",
        "totalTxns",
        // 'opStackTotalOperationalTxns' needs indexed blockscout
        "totalOperationalTxns",
        // transactions
        "pendingTxns30m",
        "txnsFee24h",
        "averageTxnFee24h",
        "newTxns24h",
        "opStackNewOperationalTxns24h",
        "newOperationalTxns24h",
        // contracts
        "totalContracts",
        "totalVerifiedContracts",
        // valid w/o
        "totalTokens",
    ];

    if blockscout_indexed {
        expected_counter_names.extend([
            "completedTxns",
            "lastNewContracts",
            "lastNewVerifiedContracts",
            "totalAccounts",
            // "totalNativeCoinHolders", // disabled
            "totalNativeCoinTransfers",
            "opStackTotalOperationalTxns",
            // on a different page; they are checked by other endpoint tests and
            // `check_all_enabled_charts_have_endpoints`.

            // "newContracts24h",
            // "newVerifiedContracts24h",
            // "yesterdayTxns",
        ]);
    }

    if user_ops_indexed {
        expected_counter_names.extend(["totalUserOps", "totalAccountAbstractionWallets"]);
    }

    assert_eq!(
        sorted_vec(counter_names),
        sorted_vec(expected_counter_names)
    );
}

pub async fn test_main_page_ok(base: Url, expect_chain_specific: bool, blockscout_indexed: bool) {
    let main_page: MainPageStats = send_get_request(&base, "/api/v1/pages/main").await;
    let MainPageStats {
        average_block_time,
        total_addresses,
        total_blocks,
        total_transactions,
        yesterday_transactions,
        total_operational_transactions,
        yesterday_operational_transactions,
        op_stack_total_operational_transactions,
        op_stack_yesterday_operational_transactions,
        daily_new_transactions,
        daily_new_operational_transactions,
        op_stack_daily_new_operational_transactions,
    } = main_page;
    let mut counters = array_of_variables_with_names!([
        average_block_time,
        total_addresses,
        total_blocks,
        total_transactions,
        yesterday_transactions,
    ])
    .to_vec();
    if expect_chain_specific {
        counters.extend(array_of_variables_with_names!([
            total_operational_transactions,
            yesterday_operational_transactions,
            op_stack_yesterday_operational_transactions,
        ]));
        if blockscout_indexed {
            counters.extend(array_of_variables_with_names!([
                op_stack_total_operational_transactions,
            ]));
        }
    }
    for (name, counter) in counters {
        let counter =
            counter.unwrap_or_else(|| panic!("main page counter {} must be available", name));
        assert!(!counter.description.is_empty());
        assert!(!counter.title.is_empty());
    }

    let mut window_line_charts = array_of_variables_with_names!([daily_new_transactions]).to_vec();
    if expect_chain_specific {
        window_line_charts.extend(array_of_variables_with_names!([
            daily_new_operational_transactions,
            op_stack_daily_new_operational_transactions
        ]));
    }
    for (name, window_chart) in window_line_charts {
        let window_chart =
            window_chart.unwrap_or_else(|| panic!("{} chart must be available", name));
        let transactions_info = window_chart.info.unwrap();
        assert!(!transactions_info.id.is_empty());
        assert_eq!(transactions_info.resolutions, vec!["DAY"]);
        assert_eq!(window_chart.chart.len(), NEW_TXNS_WINDOW_RANGE as usize);
    }
}

pub async fn test_transactions_page_ok(base: Url, expect_chain_specific: bool) {
    let TransactionsPageStats {
        pending_transactions_30m,
        transactions_fee_24h,
        average_transactions_fee_24h,
        transactions_24h,
        operational_transactions_24h,
        op_stack_operational_transactions_24h,
    } = send_get_request(&base, "/api/v1/pages/transactions").await;
    let mut counters = array_of_variables_with_names!([
        pending_transactions_30m,
        transactions_fee_24h,
        average_transactions_fee_24h,
        transactions_24h,
    ])
    .to_vec();
    if expect_chain_specific {
        counters.extend(array_of_variables_with_names!([
            operational_transactions_24h,
            op_stack_operational_transactions_24h
        ]));
    }
    for (name, counter) in counters {
        let counter = counter
            .unwrap_or_else(|| panic!("transactions page counter {} must be available", name));
        assert!(!counter.description.is_empty());
        assert!(!counter.title.is_empty());
    }
}

pub async fn test_contracts_page_ok(base: Url) {
    let ContractsPageStats {
        total_contracts,
        new_contracts_24h,
        total_verified_contracts,
        new_verified_contracts_24h,
    } = send_get_request(&base, "/api/v1/pages/contracts").await;
    let counters = array_of_variables_with_names!([
        total_contracts,
        new_contracts_24h,
        total_verified_contracts,
        new_verified_contracts_24h,
    ]);
    for (name, counter) in counters {
        let counter =
            counter.unwrap_or_else(|| panic!("contracts page counter {} must be available", name));
        assert!(!counter.description.is_empty());
        assert!(!counter.title.is_empty());
    }
}
