use actix_web::{
    dev::RequestHead,
    http::{uri::PathAndQuery, StatusCode, Uri},
    web::Bytes,
};
use awc::{Client, ClientRequest};
use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, str, time};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Instance {
    pub id: String,
    pub title: String,
    pub url: url::Url,
}

#[derive(Debug, Clone)]
pub struct BlockscoutProxy {
    instances: Vec<Instance>,
    concurrent_requests: usize,
    request_timeout: time::Duration,
}

impl BlockscoutProxy {
    pub fn new(
        instances: Vec<Instance>,
        concurrent_requests: usize,
        request_timeout: time::Duration,
    ) -> Self {
        Self {
            instances,
            concurrent_requests,
            request_timeout,
        }
    }

    pub fn instances(&self) -> Vec<Instance> {
        self.instances.clone()
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
pub struct InstanceResponse {
    pub instance: Instance,
    pub content: String,
    #[serde(with = "http_serde::status_code")]
    pub status: StatusCode,
    #[serde(with = "http_serde::uri")]
    pub uri: Uri,
    pub elapsed_secs: String,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
pub struct Response(pub HashMap<String, InstanceResponse>);

impl BlockscoutProxy {
    #[tracing::instrument(skip(self, body, request_head), level = "debug")]
    pub async fn make_requests(
        &self,
        path_and_query: Option<&PathAndQuery>,
        body: Bytes,
        request_head: &RequestHead,
    ) -> Response {
        let client = Client::builder().timeout(self.request_timeout).finish();

        let responses = stream::iter(self.instances.iter())
            .map(|instance| async {
                let mut url = instance.url.clone().to_string();
                if let Some(path_and_query) = path_and_query {
                    url = url.trim_end_matches('/').to_string();
                    url = format!("{url}{path_and_query}")
                };
                let request = client.request_from(url, request_head);
                let response = Self::send_request(instance, request, body.clone()).await;
                (instance.id.clone(), response)
            })
            .buffer_unordered(self.concurrent_requests)
            .collect::<HashMap<_, _>>()
            .await;
        Response(responses)
    }

    #[tracing::instrument(skip(request, body), level = "debug")]
    async fn send_request(
        instance: &Instance,
        request: ClientRequest,
        body: Bytes,
    ) -> InstanceResponse {
        let uri = request.get_uri().to_owned();
        let now = time::Instant::now();
        let (content, status) = match Self::perform_request(request, body).await {
            Ok((body, status)) => (body, status),
            Err(err) => (err.to_string(), StatusCode::INTERNAL_SERVER_ERROR),
        };
        let elapsed_secs = now.elapsed().as_secs_f64().to_string();
        tracing::debug!(elapsed = ?elapsed_secs, "request finished");
        InstanceResponse {
            instance: instance.clone(),
            content,
            status,
            uri,
            elapsed_secs,
        }
    }

    async fn perform_request(
        request: ClientRequest,
        body: Bytes,
    ) -> Result<(String, StatusCode), anyhow::Error> {
        let mut response = request
            .send_body(body.clone())
            .await
            .map_err(|e| anyhow::Error::msg(e.to_string()))?;
        let bytes = response.body().await?;
        let content = str::from_utf8(bytes.as_ref())?.to_string();
        Ok((content, response.status()))
    }
}
