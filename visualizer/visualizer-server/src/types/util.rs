use crate::proto::FieldMask;
use lazy_static::lazy_static;
use regex::{Regex, RegexBuilder};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::{Path, PathBuf},
};
use visualizer::{OutputMask, ResponseFieldMask};

lazy_static! {
    static ref REGEX_ONLY_CHARS: Regex = RegexBuilder::new(r"[^a-z0-9_./-]")
        .case_insensitive(true)
        .multi_line(true)
        .build()
        .unwrap();
    static ref REGEX_NO_DOTS: Regex = Regex::new(r"(^|/)[.]+($|/)").unwrap();
}

pub fn sources(sources: HashMap<String, String>) -> BTreeMap<PathBuf, String> {
    sources
        .into_iter()
        .map(|(path, content)| (PathBuf::from(path), content))
        .collect()
}

pub fn output_mask(field_mask: Option<FieldMask>) -> Result<OutputMask, anyhow::Error> {
    let mut output_mask: OutputMask = field_mask
        .map(|mask| {
            mask.paths
                .into_iter()
                .map(|s| ResponseFieldMask::try_from(s.as_str()))
                .collect::<Result<HashSet<_>, anyhow::Error>>()
                .map(OutputMask)
        })
        .unwrap_or_else(|| Ok(Default::default()))?;
    // empty output mask means that all fields must be present
    if output_mask.0.is_empty() {
        output_mask = OutputMask::full();
    };
    Ok(output_mask)
}

pub fn fix_sources_paths(sources: BTreeMap<PathBuf, String>) -> BTreeMap<PathBuf, String> {
    sources
        .into_iter()
        .map(|(path, content)| {
            let path = path
                .as_os_str()
                .to_str()
                .map(|path_str| {
                    let path_str = sanitize_path(path_str.trim_start_matches('/'));
                    Path::new(&path_str).to_path_buf()
                })
                .unwrap_or(path);
            (path, content)
        })
        .collect()
}

// Should be the same as in
// https://github.com/ethereum/sourcify/blob/d0882a5d6158d0f56e121835d79860034f072cd8/services/verification/src/services/Injector.ts#L907
pub fn sanitize_path(path: &str) -> String {
    let path = REGEX_ONLY_CHARS.replace_all(path, "_");
    let path = REGEX_NO_DOTS.replace_all(path.as_ref(), "_");
    path.to_string()
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
            "actual error doesn't contains expected part: {actual_error}"
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

    fn test_sources_paths(input: serde_json::Value, expected: serde_json::Value) {
        let actual =
            fix_sources_paths(serde_json::from_value(input).expect("invalid input: not map"));
        assert_eq!(
            serde_json::to_value(actual).expect("BTree map should be valud Value"),
            expected
        );
    }

    #[test]
    fn valid_fix_sources_paths() {
        test_sources_paths(
            serde_json::json!({
                "/root/kek/a.txt": "content1",
                "root/kek/b.txt": "content1",
                "/a.txt": "content",
            }),
            serde_json::json!({
                "root/kek/a.txt": "content1",
                "root/kek/b.txt": "content1",
                "a.txt": "content",
            }),
        );

        test_sources_paths(
            serde_json::json!({
                "@hello/kitty/a.sol": "content2",
                "/_hello/kitty/b.sol": "content2",
            }),
            serde_json::json!({
                "_hello/kitty/a.sol": "content2",
                "_hello/kitty/b.sol": "content2",
            }),
        );

        test_sources_paths(
            serde_json::json!({
                "/@hello/kitty/a.sol": "content3",
                "/_hello/kitty/b.sol": "content3",
            }),
            serde_json::json!({
                "_hello/kitty/a.sol": "content3",
                "_hello/kitty/b.sol": "content3",
            }),
        );

        test_sources_paths(
            serde_json::json!({
                "/h@llo/kitty/a.sol": "content4",
                "/h_llo/kitty/b.sol": "content4",
            }),
            serde_json::json!({
                "h_llo/kitty/a.sol": "content4",
                "h_llo/kitty/b.sol": "content4",
            }),
        );

        test_sources_paths(
            serde_json::json!({
                "/_hello/!№%:,;()kitty/a.sol": "content5",
                "/hello/kitty/../a.sol": "content5",
            }),
            serde_json::json!({
                "_hello/________kitty/a.sol": "content5",
                "hello/kitty_a.sol": "content5",
            }),
        );
    }
}
