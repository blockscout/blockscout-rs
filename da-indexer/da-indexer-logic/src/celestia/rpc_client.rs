use std::future::Future;

use celestia_rpc::Error;
use celestia_types::{AppVersion, ExtendedDataSquare};
use http::{header, HeaderValue};
use jsonrpsee::{
    core::client::{self, ClientT},
    http_client::{HeaderMap, HttpClientBuilder},
    ws_client::WsClientBuilder,
};

/// The maximum request size in the default client in celestia_rpc is not sufficient for some blocks,
/// therefore, we need to customize client initialization
pub async fn new_celestia_client(
    conn_str: &str,
    auth_token: Option<&str>,
    max_request_size: u32,
    max_response_size: u32,
) -> celestia_rpc::Result<celestia_rpc::Client> {
    let mut headers = HeaderMap::new();

    if let Some(token) = auth_token {
        let val = HeaderValue::from_str(&format!("Bearer {token}"))?;
        headers.insert(header::AUTHORIZATION, val);
    }

    let protocol = conn_str.split_once(':').map(|(proto, _)| proto);
    let client = match protocol {
        Some("http") | Some("https") => celestia_rpc::Client::Http(
            HttpClientBuilder::default()
                .set_headers(headers)
                .max_request_size(max_request_size)
                .max_response_size(max_response_size)
                .build(conn_str)?,
        ),
        Some("ws") | Some("wss") => celestia_rpc::Client::Ws(
            WsClientBuilder::default()
                .set_headers(headers)
                .max_request_size(max_request_size)
                .max_response_size(max_response_size)
                .build(conn_str)
                .await?,
        ),
        _ => return Err(Error::ProtocolNotSupported(conn_str.into())),
    };

    Ok(client)
}

// celestia_rpc::Client doesn't support new version of share.GetEDS method
// so we need to implement it manually
pub mod rpc {
    use celestia_types::eds::RawExtendedDataSquare;
    use jsonrpsee::proc_macros::rpc;

    #[rpc(client)]
    pub trait ShareV2 {
        #[method(name = "share.GetEDS")]
        async fn share_get_eds_v2(
            &self,
            height: u64,
        ) -> Result<RawExtendedDataSquare, client::Error>;
    }
}

pub trait ShareV2Client: ClientT {
    /// GetEDS gets the full EDS identified by the given root.
    fn share_get_eds_v2<'a, 'b, 'fut>(
        &'a self,
        height: u64,
        app_version: u64,
    ) -> impl Future<Output = Result<ExtendedDataSquare, client::Error>> + Send + 'fut
    where
        'a: 'fut,
        'b: 'fut,
        Self: Sized + Sync + 'fut,
    {
        async move {
            let app_version = AppVersion::from_u64(app_version).ok_or_else(|| {
                let e = format!("Invalid or unsupported AppVersion: {app_version}");
                client::Error::Custom(e)
            })?;

            let raw_eds = rpc::ShareV2Client::share_get_eds_v2(self, height).await?;

            ExtendedDataSquare::from_raw(raw_eds, app_version)
                .map_err(|e| client::Error::Custom(e.to_string()))
        }
    }
}

impl<T> ShareV2Client for T where T: ClientT {}
