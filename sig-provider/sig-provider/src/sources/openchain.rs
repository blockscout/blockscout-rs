use crate::SignatureSource;
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

    async fn fetch(&self, path: &str) -> Result<json::GetResponse, anyhow::Error> {
        let response = self
            .client
            .get(self.host.join(path).unwrap())
            .send()
            .await
            .map_err(anyhow::Error::msg)?;
        match response.status() {
            reqwest::StatusCode::OK => Ok(response.json().await?),
            status => Err(anyhow::anyhow!(
                "invalid status code got as a result: {}",
                status
            )),
        }
    }

    fn convert(sigs: Option<json::SigMap>, hash: &str) -> Vec<String> {
        // TODO: sort using "filtered" field
        sigs.and_then(|mut sigs| {
            sigs.remove(hash)
                .map(|sigs| sigs.into_iter().map(|sig| sig.name).collect())
        })
        .unwrap_or_default()
    }
}

#[async_trait::async_trait]
impl SignatureSource for Source {
    async fn create_signatures(&self, _abi: &str) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn get_function_signatures(&self, hex: &str) -> Result<Vec<String>, anyhow::Error> {
        let hash = super::hash(hex);
        let resp = self
            .fetch(&format!(
                "/signature-database/v1/lookup?function={hash}&filter=false"
            ))
            .await?;
        let signatures = Self::convert(resp.result.function, &hash);
        Ok(signatures)
    }

    async fn get_event_signatures(&self, hex: &str) -> Result<Vec<String>, anyhow::Error> {
        let hash = super::hash(hex);
        let resp = self
            .fetch(&format!(
                "/signature-database/v1/lookup?event={hash}&filter=false"
            ))
            .await?;
        let signatures = Self::convert(resp.result.event, &hash);
        Ok(signatures)
    }

    fn source(&self) -> String {
        self.host.to_string()
    }
}

mod json {
    use std::collections::HashMap;

    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    pub struct Signature {
        pub name: String,
    }

    pub type SigMap = HashMap<String, Vec<Signature>>;

    #[derive(Debug, Deserialize)]
    pub struct SigTypes {
        pub function: Option<SigMap>,
        pub event: Option<SigMap>,
        pub _error: Option<SigMap>,
    }

    #[derive(Debug, Deserialize)]
    pub struct GetResponse {
        pub result: SigTypes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    const DEFAULT_HOST: &str = "https://api.4byte.sourcify.dev/";

    #[rstest::fixture]
    fn source() -> Source {
        let host = url::Url::from_str(DEFAULT_HOST).expect("default host is not an url");
        Source::new(host)
    }

    #[rstest::rstest]
    #[tokio::test]
    async fn create(source: Source) {
        let abi = r#"[{"constant":false,"inputs":[],"name":"f","outputs":[],"type":"function"},{"inputs":[],"type":"constructor"},{"anonymous":false,"inputs":[{"name":"","type":"string","indexed":true}],"name":"E","type":"event"}]"#;
        source
            .create_signatures(abi)
            .await
            .expect("error while submitting a new signature");
    }

    #[rstest::rstest]
    #[tokio::test]
    async fn get_function_signatures(source: Source) {
        let (signature, hex) = ("f()", "0x26121ff0");
        let result = source
            .get_function_signatures(hex)
            .await
            .expect("error while getting function signature");
        assert!(result.contains(&signature.into()))
    }

    #[rstest::rstest]
    #[tokio::test]
    async fn get_event_signatures(source: Source) {
        let (signature, hex) = (
            "E(string)",
            "0x3e9992c940c54ea252d3a34557cc3d3014281525c43d694f89d5f3dfd820b07d",
        );
        let result = source
            .get_event_signatures(hex)
            .await
            .expect("error while getting event signature");
        assert!(result.contains(&signature.into()))
    }
}
