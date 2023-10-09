use super::util::{fix_sources_paths, output_mask, sources};
use crate::proto;
use amplify::{From, Wrapper};
use std::path::PathBuf;

#[derive(Wrapper, From, Clone, Debug, PartialEq)]
pub struct VisualizeStorageRequestWrapper(proto::VisualizeStorageRequest);

impl TryFrom<VisualizeStorageRequestWrapper> for visualizer::VisualizeStorageRequest {
    type Error = tonic::Status;

    fn try_from(request: VisualizeStorageRequestWrapper) -> Result<Self, Self::Error> {
        let request = request.0;
        Ok(Self {
            sources: fix_sources_paths(sources(request.sources)),
            file_path: PathBuf::from(request.file_name),
            contract_name: request.contract_name,
            output_mask: output_mask(request.output_mask)
                .map_err(|e| tonic::Status::invalid_argument(e.to_string()))?,
        })
    }
}
