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

#[macro_export]
macro_rules! array_of_variables_with_names {
    ([
        $($var:ident),+ $(,)?
    ]) => {
        [
            $((stringify!($var), $var)),+
        ]
    };
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    #[test]
    fn array_of_variables_with_names_works() {
        let (var1, var2, var3, var4, var5) = (1, 2, 3, 4, 5);
        assert_eq!(
            array_of_variables_with_names!([var1, var2, var3, var4, var5]),
            [
                ("var1", var1),
                ("var2", var2),
                ("var3", var3),
                ("var4", var4),
                ("var5", var5),
            ]
        )
    }
}
