use super::util::{fix_sources_paths, output_mask, sources};
use crate::proto;
use amplify::{From, Wrapper};

#[derive(Wrapper, From, Clone, Debug, PartialEq)]
pub struct VisualizeContractsRequestWrapper(proto::VisualizeContractsRequest);

impl TryFrom<VisualizeContractsRequestWrapper> for visualizer::VisualizeContractsRequest {
    type Error = tonic::Status;

    fn try_from(request: VisualizeContractsRequestWrapper) -> Result<Self, Self::Error> {
        let request = request.0;
        Ok(Self {
            sources: fix_sources_paths(sources(request.sources)),
            output_mask: output_mask(request.output_mask)
                .map_err(|e| tonic::Status::invalid_argument(e.to_string()))?,
        })
    }
}
