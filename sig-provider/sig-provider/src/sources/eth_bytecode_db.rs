use crate::SignatureSource;
use anyhow::Error;
use reqwest_middleware::ClientWithMiddleware;

pub struct Source {
    host: url::Url,
    _client: ClientWithMiddleware,
}

impl Source {
    pub fn _new(host: url::Url) -> Source {
        Source {
            host,
            _client: super::new_client(),
        }
    }
}

#[async_trait::async_trait]
impl SignatureSource for Source {
    async fn create_signatures(&self, _abi: &str) -> Result<(), Error> {
        Ok(())
    }

    async fn get_function_signatures(&self, _hex: &str) -> Result<Vec<String>, Error> {
        Ok(Vec::new())
    }

    async fn get_event_signatures(&self, _hex: &str) -> Result<Vec<String>, Error> {
        const _ROUTE: &str = "/api/v2/event-descriptions:search";
        todo!()
    }

    fn source(&self) -> String {
        self.host.to_string()
    }
}
