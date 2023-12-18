use crate::{
    eth_bytecode_db::json::SearchEventDescriptionResponse, sources::CompleteSignatureSource,
};
use reqwest_middleware::ClientWithMiddleware;

pub struct Source {
    host: url::Url,
    client: ClientWithMiddleware,
}

impl Source {
    pub fn new(host: url::Url) -> Source {
        Source {
            host,
            client: super::new_client(),
        }
    }

    async fn send_post_request<Request, Response>(
        &self,
        path: &str,
        request: &Request,
    ) -> Result<Response, anyhow::Error>
    where
        Request: serde::Serialize,
        Response: serde::de::DeserializeOwned,
    {
        let response = self
            .client
            .post(self.host.join(path).unwrap())
            .json(request)
            .send()
            .await
            .map_err(anyhow::Error::msg)?;
        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "invalid status code got as a result: {}",
                response.status(),
            ))
        }
    }
}

#[async_trait::async_trait]
impl CompleteSignatureSource for Source {
    async fn get_event_signatures(
        &self,
        hex: &str,
    ) -> Result<Vec<alloy_json_abi::Event>, anyhow::Error> {
        let route = "/api/v2/event-descriptions:search";
        let request = serde_json::json!({"selector": super::hash(hex)});
        println!("RequesT: {request:?}");
        Ok(self
            .send_post_request::<_, SearchEventDescriptionResponse>(route, &request)
            .await?
            .event_descriptions
            .into_iter()
            .map(alloy_json_abi::Event::try_from)
            .collect::<Result<_, _>>()?)
    }

    fn source(&self) -> String {
        self.host.to_string()
    }
}

mod json {
    use anyhow::Context;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct SearchEventDescriptionsRequest {}

    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct SearchEventDescriptionResponse {
        pub event_descriptions: Vec<EventDescription>,
    }

    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct EventDescription {
        name: String,
        inputs: String,
    }

    impl TryFrom<EventDescription> for alloy_json_abi::Event {
        type Error = anyhow::Error;
        fn try_from(value: EventDescription) -> Result<Self, Self::Error> {
            let inputs: Vec<alloy_json_abi::EventParam> = serde_json::from_str(&value.inputs)
                .context("deserializing event_description inputs")?;
            Ok(Self {
                name: value.name,
                inputs,
                anonymous: false,
            })
        }
    }
}
