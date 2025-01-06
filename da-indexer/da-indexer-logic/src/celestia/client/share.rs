use std::future::Future;

use celestia_types::{AppVersion, ExtendedDataSquare};
use jsonrpsee::core::client::{ClientT, Error};

// celestia_rpc::Client doesn't support new version of share.GetEDS method
// so we need to implement it manually
mod rpc {
    use celestia_types::eds::RawExtendedDataSquare;
    use jsonrpsee::proc_macros::rpc;

    #[rpc(client)]
    pub trait Share {
        #[method(name = "share.GetEDS")]
        async fn share_get_eds(&self, height: u64) -> Result<RawExtendedDataSquare, client::Error>;
    }
}

pub trait ShareClient: ClientT {
    /// GetEDS gets the full EDS identified by the given root.
    fn share_get_eds<'a, 'b, 'fut>(
        &'a self,
        height: u64,
        app_version: u64,
    ) -> impl Future<Output = Result<ExtendedDataSquare, Error>> + Send + 'fut
    where
        'a: 'fut,
        'b: 'fut,
        Self: Sized + Sync + 'fut,
    {
        async move {
            let app_version = AppVersion::from_u64(app_version).ok_or_else(|| {
                let e = format!("Invalid or unsupported AppVersion: {app_version}");
                Error::Custom(e)
            })?;

            let raw_eds = rpc::ShareClient::share_get_eds(self, height).await?;

            ExtendedDataSquare::from_raw(raw_eds, app_version)
                .map_err(|e| Error::Custom(e.to_string()))
        }
    }
}

impl<T> ShareClient for T where T: ClientT {}
