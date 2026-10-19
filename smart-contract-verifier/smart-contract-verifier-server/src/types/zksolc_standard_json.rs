// SPDX-License-Identifier: LicenseRef-Blockscout

use super::StandardJsonParseError;
use crate::proto::zksync::solidity::VerifyStandardJsonRequest;
use amplify::{From, Wrapper};
use anyhow::anyhow;
use blockscout_display_bytes::Bytes as DisplayBytes;
use smart_contract_verifier::{
    zksync::{zksolc_standard_json, VerificationRequest},
    CompactVersion, DetailedVersion,
};
use std::str::FromStr;

#[derive(Wrapper, From, Clone, Debug, PartialEq)]
pub struct VerifyStandardJsonRequestWrapper(VerifyStandardJsonRequest);

impl TryFrom<VerifyStandardJsonRequestWrapper> for VerificationRequest {
    type Error = StandardJsonParseError;

    fn try_from(request: VerifyStandardJsonRequestWrapper) -> Result<Self, Self::Error> {
        let request = request.into_inner();

        let code = DisplayBytes::from_str(&request.code)
            .map_err(|err| anyhow!("Invalid deployed bytecode: {:#?}", err))?
            .0;
        let constructor_arguments = request
            .constructor_arguments
            .as_deref()
            .map(DisplayBytes::from_str)
            .transpose()
            .map_err(|err| anyhow!("Invalid constructor arguments: {:#?}", err))?
            .map(|v| v.0);
        let zk_compiler = CompactVersion::from_str(&request.zk_compiler)
            .map_err(|err| anyhow!("Invalid zk compiler: {}", err))?;
        let solc_compiler = DetailedVersion::from_str(&request.solc_compiler)
            .map_err(|err| anyhow!("Invalid solc compiler: {}", err))?;

        let content: zksolc_standard_json::input::Input = serde_json::from_str(&request.input)?;
        content.settings.validate()?;

        Ok(Self {
            code,
            constructor_arguments,
            zk_compiler,
            solc_compiler,
            content,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request_with_settings(settings: serde_json::Value) -> VerifyStandardJsonRequestWrapper {
        VerifyStandardJsonRequest {
            code: "00".to_string(),
            constructor_arguments: None,
            zk_compiler: "v1.4.0".to_string(),
            solc_compiler: "v0.8.17+commit.8df45f5f".to_string(),
            input: serde_json::json!({
                "language": "Solidity",
                "sources": {},
                "settings": settings,
            })
            .to_string(),
        }
        .into()
    }

    #[test]
    fn rejects_llvm_options_as_bad_request() {
        let request = request_with_settings(serde_json::json!({
            "optimizer": { "enabled": false },
            "LLVMOptions": ["--exec-on-ir-change=/bin/true"],
        }));

        let error = VerificationRequest::try_from(request).unwrap_err();
        assert!(matches!(error, StandardJsonParseError::BadRequest(_)));
        assert!(error.to_string().contains("LLVMOptions is not supported"));
    }

    #[test]
    fn accepts_standard_input_without_llvm_options() {
        for settings in [
            serde_json::json!({ "optimizer": { "enabled": false } }),
            serde_json::json!({ "optimizer": { "enabled": false }, "LLVMOptions": [] }),
        ] {
            VerificationRequest::try_from(request_with_settings(settings)).unwrap();
        }
    }
}
