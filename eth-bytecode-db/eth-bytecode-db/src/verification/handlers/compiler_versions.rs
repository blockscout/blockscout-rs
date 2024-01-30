use super::super::{smart_contract_verifier::ListCompilerVersionsRequest, Client};
use anyhow::Error;
use smart_contract_verifier_proto::http_client::{solidity_verifier_client, vyper_verifier_client};

pub async fn solidity_versions(client: Client) -> Result<Vec<String>, Error> {
    let response = solidity_verifier_client::list_compiler_versions(
        &client.verifier_http_client,
        ListCompilerVersionsRequest::default(),
    )
    .await?;

    Ok(response.compiler_versions)
}

pub async fn vyper_versions(client: Client) -> Result<Vec<String>, Error> {
    let response = vyper_verifier_client::list_compiler_versions(
        &client.verifier_http_client,
        ListCompilerVersionsRequest::default(),
    )
    .await?;

    Ok(response.compiler_versions)
}
