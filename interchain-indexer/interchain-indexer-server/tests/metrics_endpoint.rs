// SPDX-License-Identifier: LicenseRef-Blockscout

mod helpers;

use std::net::{Ipv4Addr, SocketAddr, TcpListener};

use interchain_indexer_logic::indexer::metrics::LOG_STREAM_HEAD_BLOCK;

/// The indexer metrics live in the `prometheus` default registry of the
/// version the logic crate links; the metrics endpoint serves the default
/// registry of the version `blockscout-service-launcher` links. Two different
/// `prometheus` versions in the dependency tree mean two default registries,
/// and then no indexer metric reaches `/metrics` while everything still
/// compiles. This happened from the service's first commit until the
/// workspace moved to the launcher's version.
#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_metrics_endpoint_serves_indexer_metrics() {
    let metrics_addr = {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        SocketAddr::from((Ipv4Addr::LOCALHOST, listener.local_addr().unwrap().port()))
    };
    let db = helpers::init_db("test", "metrics_endpoint").await;
    helpers::init_interchain_indexer_server(db.db_url(), |mut settings| {
        settings.metrics.enabled = true;
        settings.metrics.addr = metrics_addr;
        settings
    })
    .await;

    LOG_STREAM_HEAD_BLOCK
        .with_label_values(&["0", "0", "metrics_endpoint_test"])
        .set(1.0);

    let body = reqwest::get(format!("http://{metrics_addr}/metrics"))
        .await
        .expect("metrics endpoint is reachable")
        .text()
        .await
        .unwrap();
    assert!(
        body.contains(r#"interchain_indexer_log_stream_head_block{bridge_id="0",chain_id="0",tag="metrics_endpoint_test"} 1"#),
        "an indexer metric is missing from /metrics; is there more than one \
         `prometheus` version in the dependency tree? Body:\n{body}"
    );
}
