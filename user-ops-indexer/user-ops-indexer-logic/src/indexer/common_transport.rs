use async_trait::async_trait;
use ethabi::ethereum_types::U256;
use ethers::prelude::{Http, JsonRpcClient, ProviderError, PubsubClient, Ws};
use serde::{de::DeserializeOwned, Serialize};
use std::{fmt::Debug, str::FromStr};

#[cfg(test)]
use ethers::prelude::MockProvider;

#[derive(Clone, Debug)]
pub enum CommonTransport {
    Ws(Ws),
    Http(Http),
    #[cfg(test)]
    Mock(MockProvider),
}

impl CommonTransport {
    pub async fn new(rpc_url: String) -> Result<Self, ProviderError> {
        if rpc_url.trim().starts_with("ws") {
            // ethers-rs does not handle ws reconnects well, neither it can guarantee that no
            // events would be lost even if reconnect is successful, so it's better to restart
            // the whole indexer at once instead of trying to reconnect.
            Ok(Self::Ws(Ws::connect_with_reconnects(rpc_url, 0).await?))
        } else {
            Ok(Self::Http(
                Http::from_str(&rpc_url).map_err(|e| ProviderError::CustomError(e.to_string()))?,
            ))
        }
    }

    pub fn supports_subscriptions(&self) -> bool {
        matches!(self, CommonTransport::Ws(_))
    }
}

#[async_trait]
impl JsonRpcClient for CommonTransport {
    type Error = ProviderError;

    async fn request<T, R>(&self, method: &str, params: T) -> Result<R, Self::Error>
    where
        T: Debug + Serialize + Send + Sync,
        R: DeserializeOwned + Send,
    {
        match self {
            CommonTransport::Ws(ws) => ws
                .request(method, params)
                .await
                .map_err(ProviderError::from),
            CommonTransport::Http(http) => http
                .request(method, params)
                .await
                .map_err(ProviderError::from),
            #[cfg(test)]
            CommonTransport::Mock(mock) => mock
                .request(method, params)
                .await
                .map_err(ProviderError::from),
        }
    }
}

impl PubsubClient for CommonTransport {
    type NotificationStream = <Ws as PubsubClient>::NotificationStream;

    fn subscribe<T: Into<U256>>(&self, id: T) -> Result<Self::NotificationStream, Self::Error> {
        match self {
            CommonTransport::Ws(ws) => ws.subscribe(id).map_err(ProviderError::from),
            _ => Err(ProviderError::UnsupportedRPC),
        }
    }

    fn unsubscribe<T: Into<U256>>(&self, id: T) -> Result<(), Self::Error> {
        match self {
            CommonTransport::Ws(ws) => ws.unsubscribe(id).map_err(ProviderError::from),
            _ => Err(ProviderError::UnsupportedRPC),
        }
    }
}
