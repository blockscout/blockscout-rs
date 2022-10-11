use crate::{
    proto::blockscout::sig_provider::v1::{
        signature_service_server::SignatureService, CreateSignaturesRequest,
        CreateSignaturesResponse, GetSignaturesRequest, GetSignaturesResponse, Signature,
    },
    provider::SignatureProvider,
};
use std::sync::Arc;

pub struct SignatureAggregator {
    sources: Vec<Arc<dyn SignatureProvider + Send + Sync + 'static>>,
}

impl SignatureAggregator {
    pub fn new(
        sources: Vec<Arc<dyn SignatureProvider + Send + Sync + 'static>>,
    ) -> SignatureAggregator {
        SignatureAggregator { sources }
    }

    fn merge_signatures<I: IntoIterator<Item = GetSignaturesResponse>>(sigs: I) -> Vec<Signature> {
        let mut sigs: Vec<_> = sigs
            .into_iter()
            .flat_map(|sig| sig.signatures.into_iter())
            .collect();
        sigs.sort_by(|a, b| a.name.cmp(&b.name));
        sigs.dedup();
        sigs
    }
}

macro_rules! proxy {
    ($self:ident, $request:ident, $fn:ident) => {{
        let request = $request.into_inner();
        let tasks = $self
            .sources
            .iter()
            .map(|source| source.$fn(request.clone()));
        let responses: Vec<_> = futures::future::join_all(tasks)
            .await
            .into_iter()
            .zip($self.sources.iter())
            .filter_map(|(resp, source)| match resp {
                Ok(resp) => Some(resp),
                Err(error) => {
                    tracing::error!(
                        "could not call {} for host {}, error: {}",
                        stringify!($fn),
                        source.host(),
                        error
                    );
                    None
                }
            })
            .collect();
        responses
    }};
}

#[async_trait::async_trait]
impl SignatureService for SignatureAggregator {
    async fn create_signatures(
        &self,
        request: tonic::Request<CreateSignaturesRequest>,
    ) -> Result<tonic::Response<CreateSignaturesResponse>, tonic::Status> {
        let _responses = proxy!(self, request, create_signatures);
        Ok(tonic::Response::new(CreateSignaturesResponse {}))
    }

    async fn get_function_signatures(
        &self,
        request: tonic::Request<GetSignaturesRequest>,
    ) -> Result<tonic::Response<GetSignaturesResponse>, tonic::Status> {
        let responses = proxy!(self, request, get_function_signatures);
        let signatures = Self::merge_signatures(responses.into_iter());
        Ok(tonic::Response::new(GetSignaturesResponse { signatures }))
    }

    async fn get_event_signatures(
        &self,
        request: tonic::Request<GetSignaturesRequest>,
    ) -> Result<tonic::Response<GetSignaturesResponse>, tonic::Status> {
        let responses = proxy!(self, request, get_event_signatures);
        let signatures = Self::merge_signatures(responses.into_iter());
        Ok(tonic::Response::new(GetSignaturesResponse { signatures }))
    }

    async fn get_error_signatures(
        &self,
        request: tonic::Request<GetSignaturesRequest>,
    ) -> Result<tonic::Response<GetSignaturesResponse>, tonic::Status> {
        let responses = proxy!(self, request, get_error_signatures);
        let signatures = Self::merge_signatures(responses.into_iter());
        Ok(tonic::Response::new(GetSignaturesResponse { signatures }))
    }
}
