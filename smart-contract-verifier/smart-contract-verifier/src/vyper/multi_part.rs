use super::client::Client;
use crate::{
    compiler::DetailedVersion,
    verify_new,
    verify_new::{vyper_compiler_input, VyperInput},
    OnChainContract,
};
use foundry_compilers_new::{artifacts, artifacts::EvmVersion};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

#[derive(Clone, Debug)]
pub struct Content {
    pub sources: BTreeMap<PathBuf, String>,
    pub interfaces: BTreeMap<PathBuf, String>,
    pub evm_version: Option<EvmVersion>,
}

impl TryFrom<Content> for VyperInput {
    type Error = verify_new::Error;

    fn try_from(content: Content) -> Result<Self, Self::Error> {
        let settings = vyper_compiler_input::Settings {
            evm_version: content.evm_version,
            ..Default::default()
        };

        let sources: artifacts::Sources = content
            .sources
            .into_iter()
            .map(|(path, content)| (path, artifacts::Source::new(content)))
            .collect();

        let interfaces: vyper_compiler_input::Interfaces = content
            .interfaces
            .into_iter()
            .map(|(path, content)| {
                vyper_compiler_input::Interface::try_new(&path, content)
                    .map(|interface| (path, interface))
            })
            .collect::<Result<_, _>>()
            .map_err(|err| {
                verify_new::Error::Compilation(vec![format!("cannot parse inteface: {err}")])
            })?;

        Ok(VyperInput {
            language: "Vyper".to_string(),
            sources,
            interfaces,
            settings,
        })
    }
}

#[derive(Clone, Debug)]
pub struct VerificationRequest {
    pub contract: OnChainContract,
    pub compiler_version: DetailedVersion,
    pub content: Content,
}

pub async fn verify(
    client: Arc<Client>,
    request: VerificationRequest,
) -> Result<verify_new::VerificationResult, verify_new::Error> {
    let to_verify = vec![request.contract];
    let compilers = client.new_compilers();

    let vyper_input = VyperInput::try_from(request.content)?;
    let results = verify_new::compile_and_verify(
        to_verify,
        compilers,
        &request.compiler_version,
        vyper_input,
    )
    .await?;
    let result = results
        .into_iter()
        .next()
        .expect("we sent exactly one contract to verify");

    Ok(result)
}
