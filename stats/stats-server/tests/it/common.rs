use std::collections::HashMap;

use reqwest::{RequestBuilder, Response};

pub async fn send_arbitrary_request(request: RequestBuilder) -> Response {
    let response = request
        .send()
        .await
        .unwrap_or_else(|_| panic!("Failed to send request"));

    if !response.status().is_success() {
        let status = response.status();
        let message = response.text().await.expect("Read body as text");
        panic!("Invalid status code (success expected). Status: {status}. Message: {message}")
    }
    response
}

pub async fn enabled_resolutions(
    line_charts: stats_proto::blockscout::stats::v1::LineCharts,
) -> HashMap<String, Vec<String>> {
    line_charts
        .sections
        .iter()
        .flat_map(|sec| sec.charts.clone())
        .map(|l| (l.id, l.resolutions))
        .collect()
}
