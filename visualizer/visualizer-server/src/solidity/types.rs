use crate::proto::{
    blockscout::visualizer::v1::{
        VisualizeContractsRequest, VisualizeResponse, VisualizeStorageRequest,
    },
    google::protobuf::FieldMask,
};
use bytes::Bytes;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::PathBuf,
};
use visualizer::{OutputMask, ResponseFieldMask};

fn sources(sources: HashMap<String, String>) -> BTreeMap<PathBuf, String> {
    sources
        .into_iter()
        .map(|(path, content)| (PathBuf::from(path), content))
        .collect()
}

fn output_mask(field_mask: Option<FieldMask>) -> Result<OutputMask, anyhow::Error> {
    let mut output_mask: OutputMask = field_mask
        .map(|mask| {
            mask.paths
                .into_iter()
                .map(|s| ResponseFieldMask::try_from(s.as_str()))
                .collect::<Result<HashSet<_>, anyhow::Error>>()
                .map(OutputMask)
        })
        .unwrap_or_else(|| Ok(Default::default()))?;
    // empty output mask means that all fields must present
    if output_mask.0.is_empty() {
        output_mask = OutputMask::full();
    };
    Ok(output_mask)
}

impl TryFrom<VisualizeContractsRequest> for visualizer::VisualizeContractsRequest {
    type Error = anyhow::Error;

    fn try_from(request: VisualizeContractsRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            sources: sources(request.sources),
            output_mask: output_mask(request.output_mask)?,
        })
    }
}

impl TryFrom<VisualizeStorageRequest> for visualizer::VisualizeStorageRequest {
    type Error = anyhow::Error;

    fn try_from(request: VisualizeStorageRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            sources: sources(request.sources),
            file_path: PathBuf::from(request.file_name),
            contract_name: request.contract_name,
            output_mask: output_mask(request.output_mask)?,
        })
    }
}

impl From<visualizer::Response> for VisualizeResponse {
    fn from(response: visualizer::Response) -> Self {
        Self {
            png: response.png.map(Bytes::from),
            svg: response.svg.map(Bytes::from),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn valid_mask(field_mask: &str, expected_mask: OutputMask) {
        let field_mask: Option<FieldMask> =
            serde_json::from_str(field_mask).expect("invalid field mask");
        let actual_mask = output_mask(field_mask).expect("must be valid output mask");
        assert_eq!(actual_mask, expected_mask);
    }

    fn invalid_mask(field_mask: &str, expected_error: &str) {
        let field_mask: Option<FieldMask> =
            serde_json::from_str(field_mask).expect("invalid field mask");
        let actual_error = output_mask(field_mask).expect_err("must be invalid output mask");
        assert!(
            actual_error.to_string().contains(expected_error),
            "actual error doesn't contains expected part: {}",
            actual_error
        );
    }

    fn mask(fields: Vec<ResponseFieldMask>) -> OutputMask {
        OutputMask(fields.into_iter().collect())
    }

    #[test]
    fn output_mask_valid() {
        valid_mask("null", OutputMask::full());
        valid_mask(r#"{"paths": []}"#, OutputMask::full());

        valid_mask(
            r#"{
                "paths": [
                    "svg",
                    "png"
                ]
            }"#,
            OutputMask::full(),
        );
        valid_mask(
            r#"{
                "paths": [
                    "svg"
                ]
            }"#,
            mask(vec![ResponseFieldMask::Svg]),
        );
    }

    #[test]
    fn output_mask_invalid() {
        invalid_mask(
            r#"{
            "paths": [
                "svg.png"
            ]
        }"#,
            "invalid response filed mask: svg.png",
        );
        invalid_mask(
            r#"{
            "paths": [
                ".svg"
            ]
        }"#,
            "invalid response filed mask: .svg",
        );
        invalid_mask(
            r#"{
            "paths": [
                "svg",
                "abcd"
            ]
        }"#,
            "invalid response filed mask: abcd",
        );
    }
}
