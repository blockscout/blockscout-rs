use super::router::Router;
use crate::{
    handlers::sourcify,
    settings::{Extensions, SourcifySettings},
};
use actix_web::web;
use smart_contract_verifier::SourcifyApiClient;

pub struct SourcifyRouter {
    api_client: web::Data<SourcifyApiClient>,
}

impl SourcifyRouter {
    pub async fn new(
        settings: SourcifySettings,
        /* Otherwise, results in compilation warning if all extensions are disabled */
        #[allow(unused_variables)] extensions: Extensions,
    ) -> anyhow::Result<Self> {
        /* Otherwise, results in compilation warning if all extensions are disabled */
        #[allow(unused_mut)]
        let mut api_client = {
            SourcifyApiClient::new(
                settings.api_url,
                settings.request_timeout,
                settings.verification_attempts,
            )
            .expect("failed to build sourcify client")
        };

        #[cfg(feature = "sig-provider-extension")]
        if let Some(sig_provider) = extensions.sig_provider {
            // TODO(#221): create only one instance of middleware/connection
            api_client = api_client
                .with_middleware(sig_provider_extension::SigProvider::new(sig_provider).await?);
        }

        Ok(Self {
            api_client: web::Data::new(api_client),
        })
    }
}

impl Router for SourcifyRouter {
    fn register_routes(&self, service_config: &mut web::ServiceConfig) {
        service_config
            .app_data(self.api_client.clone())
            .route("/verify", web::post().to(sourcify::verify));
    }
}
