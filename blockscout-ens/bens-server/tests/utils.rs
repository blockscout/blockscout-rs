#![allow(unused_macros, unused_imports)]

use actix_web::web::redirect;
use anyhow::Context;
use bens_logic::test_utils::mocked_blockscout_client;
use bens_server::Settings;
use blockscout_service_launcher::{
    launcher::ConfigSettings,
    test_server::{get_test_server_settings, init_server, send_get_request},
};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::collections::HashMap;
use url::Url;

macro_rules! data_file_as_json {
    ($name:expr) => {{
        data_file_as_json!($name, &serde_json::json!({}))
    }};
    ($name:expr, $initial_context:expr) => {{
        fn merge(a: &mut serde_json::Value, b: &serde_json::Value) {
            match (a, b) {
                (&mut serde_json::Value::Object(ref mut a), &serde_json::Value::Object(ref b)) => {
                    for (k, v) in b {
                        merge(a.entry(k.clone()).or_insert(serde_json::Value::Null), v);
                    }
                }
                (a, b) => {
                    *a = b.clone();
                }
            }
        }

        fn render_file_with_context(
            file_content: &str,
            context: &serde_json::Value,
        ) -> serde_json::Value {
            let tera_context =
                tera::Context::from_value(context.clone()).expect("failed to create context");
            let rendered = tera::Tera::default()
                .render_str(file_content, &tera_context)
                .expect("failed to render template");
            let value: serde_json::Value =
                serde_json::from_str(&rendered).expect("failed to parse content");
            value
        }

        // render context file with initial context
        let mut context_from_file =
            render_file_with_context(include_str!("data/context.json"), $initial_context);
        merge(&mut context_from_file, $initial_context);
        // render data file with whole context
        render_file_with_context(include_str!(concat!("data/", $name)), &context_from_file)
    }};
}
pub(crate) use data_file_as_json;

pub async fn check_list_result(
    base: &Url,
    route: &str,
    query_params: HashMap<String, String>,
    expected_items: Vec<Value>,
    maybe_expected_paginated: Option<(u32, Option<String>)>,
) -> (Value, Value) {
    let route_with_query = build_query(route, &query_params);
    let request: Value = send_get_request(base, &route_with_query).await;
    let mut expected: HashMap<String, Value> =
        HashMap::from_iter([("items".to_owned(), json!(expected_items))]);
    if let Some((page_size, page_token)) = maybe_expected_paginated {
        if let Some(page_token) = page_token {
            expected.insert(
                "next_page_params".to_owned(),
                json!({
                    "page_token": page_token,
                    "page_size": page_size,
                }),
            );
        } else {
            expected.insert("next_page_params".to_owned(), json!(null));
        }
    }
    (request, json!(expected))
}

pub async fn start_server(pool: &PgPool) -> (Url, Settings) {
    let (settings, base) = prepare(pool).await.unwrap();
    {
        let settings = settings.clone();
        init_server(|| async { bens_server::run(settings).await }, &base).await;
    }
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    (base, settings)
}

pub fn build_query(route: &str, query_params: &HashMap<String, String>) -> String {
    if !query_params.is_empty() {
        let query = query_params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");
        format!("{route}?{query}")
    } else {
        route.to_string()
    }
}

pub fn settings_context(settings: &Settings) -> Value {
    let genome_base_url = settings
        .subgraphs_reader
        .networks
        .get(&10200)
        .expect("network not found")
        .blockscout
        .url
        .clone();

    let ens_base_url = settings
        .subgraphs_reader
        .networks
        .get(&1)
        .expect("network not found")
        .blockscout
        .url
        .clone();

    json!({
        "genome_base_url": genome_base_url,
        "ens_base_url": ens_base_url,
    })
}

async fn prepare(pool: &PgPool) -> Result<(Settings, Url), anyhow::Error> {
    let postgres_url =
        std::env::var("DATABASE_URL").context("env should be here from sqlx::test")?;
    let db_url = format!(
        "{postgres_url}{}",
        pool.connect_options()
            .get_database()
            .context("Failed to get database name")?
    );
    let blockscout_client = mocked_blockscout_client().await;
    std::env::set_var("BENS__DATABASE__CONNECT__URL", db_url);
    std::env::set_var("BENS__CONFIG", "./tests/config.test.json");
    std::env::set_var(
        "BENS__SUBGRAPHS_READER__NETWORKS__1__BLOCKSCOUT__URL",
        blockscout_client.url().to_string(),
    );
    std::env::set_var(
        "BENS__SUBGRAPHS_READER__NETWORKS__10200__BLOCKSCOUT__URL",
        blockscout_client.url().to_string(),
    );
    let mut settings = Settings::build().context("Failed to build settings")?;
    let (server_settings, base) = get_test_server_settings();

    settings.server = server_settings;

    Ok((settings, base))
}
