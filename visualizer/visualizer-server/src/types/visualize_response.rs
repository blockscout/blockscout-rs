use crate::proto;
use amplify::{From, Wrapper};
use bytes::Bytes;

#[derive(Wrapper, From, Clone, Debug, PartialEq)]
pub struct VisualizeResponseWrapper(proto::VisualizeResponse);

impl From<visualizer::Response> for VisualizeResponseWrapper {
    fn from(response: visualizer::Response) -> Self {
        Self(proto::VisualizeResponse {
            png: response.png.map(Bytes::from),
            svg: response.svg.map(Bytes::from),
        })
    }
}
