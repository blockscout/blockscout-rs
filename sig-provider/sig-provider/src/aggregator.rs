use crate::SignatureSource;
use sig_provider_proto::blockscout::sig_provider::v1::Abi;
use std::{collections::HashSet, sync::Arc};

pub struct SourceAggregator {
    sources: Arc<Vec<Arc<dyn SignatureSource + Send + Sync + 'static>>>,
}

macro_rules! proxy {
    ($sources:ident, $request:ident, $fn:ident) => {{
        let tasks = $sources.iter().map(|source| source.$fn($request));
        let responses: Vec<_> = futures::future::join_all(tasks)
            .await
            .into_iter()
            .zip($sources.iter())
            .filter_map(|(resp, source)| match resp {
                Ok(resp) => Some(resp),
                Err(error) => {
                    tracing::error!(
                        "could not call {} for host {}, error: {}",
                        stringify!($fn),
                        source.source(),
                        error
                    );
                    None
                }
            })
            .collect();
        responses
    }};
}

impl SourceAggregator {
    // You should provide sources in priority descending order (first - max priority)
    pub fn new(sources: Vec<Arc<dyn SignatureSource + Send + Sync + 'static>>) -> SourceAggregator {
        SourceAggregator {
            sources: Arc::new(sources),
        }
    }

    fn merge_signatures<I: IntoIterator<Item = String>, II: IntoIterator<Item = I>>(
        sigs: II,
    ) -> Vec<String> {
        let mut content: HashSet<String> = HashSet::default();
        sigs.into_iter()
            .flatten()
            .filter(|sig| content.insert(sig.clone()))
            .collect()
    }

    pub async fn create_signatures(&self, abi: String) -> Result<(), anyhow::Error> {
        let sources = self.sources.clone();
        tokio::spawn(async move {
            let abi = &abi;
            let _responses = proxy!(sources, abi, create_signatures);
        });
        Ok(())
    }

    pub async fn get_function_signatures(&self, hex: &str) -> Result<Vec<String>, anyhow::Error> {
        let sources = &self.sources;
        let responses = proxy!(sources, hex, get_function_signatures);
        let signatures = Self::merge_signatures(responses);
        Ok(signatures)
    }

    pub async fn get_event_signatures(&self, hex: &str) -> Result<Vec<String>, anyhow::Error> {
        let sources = &self.sources;
        let responses = proxy!(sources, hex, get_event_signatures);
        let signatures = Self::merge_signatures(responses);
        Ok(signatures)
    }

    pub async fn get_function_abi(&self, _tx_input: String) -> Result<Abi, anyhow::Error> {
        anyhow::bail!("unimplemented")
    }

    pub async fn get_event_abi(
        &self,
        _data: String,
        _topics: Vec<String>,
    ) -> Result<Abi, anyhow::Error> {
        anyhow::bail!("unimplemented")
    }
}
