use blockscout_service_launcher::test_server::send_get_request;

use stats::ResolutionKind;
use url::Url;

use crate::common::enabled_resolutions;

pub async fn test_lines_ok(base: Url) {
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
    ];
    assert_eq!(section_ids, expected_section_ids, "wrong sections response");

    let mut enabled_resolutions = enabled_resolutions(line_charts).await;

    // does not return data for latest dates,
    // so todo: test later with other main page stuff
    enabled_resolutions.remove("newTxnsWindow");

    for line_name in [
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
        // "nativeCoinHoldersGrowth",
        // "nativeCoinSupply",
        // "newNativeCoinHolders",
        "newBlocks",
        "newNativeCoinTransfers",
        "newTxns",
        // "newTxnsWindow",
        "txnsFee",
        "txnsGrowth",
        "newOperationalTxns",
        "operationalTxnsGrowth",
        "txnsSuccessRate",
        "newVerifiedContracts",
        "newContracts",
        "verifiedContractsGrowth",
        "contractsGrowth",
    ] {
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
    assert!(
        enabled_resolutions.is_empty(),
        "some charts were not tested ({:?})",
        enabled_resolutions
    );
}
