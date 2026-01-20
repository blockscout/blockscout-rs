use api_client_framework::{
    Endpoint, Error, HttpApiClient as Client, HttpApiClientConfig, serialize_query,
};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use url::Url;

pub fn new_client(url: Url) -> Result<Client, Error> {
    let config = HttpApiClientConfig::default();
    Client::new(url, config)
}

pub mod counters {
    use super::*;

    pub struct GetCounters {}

    impl Endpoint for GetCounters {
        type Response = Counters;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            "/stats-service/api/v1/counters".to_string()
        }
    }

    #[derive(Debug, Deserialize)]
    pub struct Counters {
        pub counters: Vec<Counter>,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct Counter {
        pub id: String,
        pub value: String,
        pub title: String,
        pub units: Option<String>,
        pub description: String,
    }
}

pub mod lines {
    use super::*;

    #[derive(Debug, Clone, Copy, Serialize)]
    #[serde(rename_all = "UPPERCASE")]
    pub enum Resolution {
        Week,
    }

    #[derive(Debug, Clone, Serialize)]
    pub struct GetLineChartParams {
        pub from: String,
        pub to: String,
        pub resolution: Resolution,
    }

    pub struct GetLineChart {
        pub name: String,
        pub params: GetLineChartParams,
    }

    impl Endpoint for GetLineChart {
        type Response = LineChart;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            format!("/stats-service/api/v1/lines/{}", self.name)
        }

        fn query(&self) -> Option<String> {
            serialize_query(&self.params)
        }
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct LineChart {
        pub chart: Vec<Point>,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct Point {
        pub date: String,
        pub date_to: Option<String>,
        pub value: String,
        pub is_approximate: Option<bool>,
    }
}
