use super::super::{smart_contract_verifier::ListCompilerVersionsRequest, Client};
use anyhow::Error;

pub async fn solidity_versions(mut client: Client) -> Result<Vec<String>, Error> {
    let response = client
        .solidity_client
        .list_compiler_versions(ListCompilerVersionsRequest::default())
        .await
        .map_err(Error::new)?
        .into_inner();

    Ok(response.compiler_versions)
}

pub async fn vyper_versions(mut client: Client) -> Result<Vec<String>, Error> {
    let response = client
        .vyper_client
        .list_compiler_versions(ListCompilerVersionsRequest::default())
        .await
        .map_err(Error::new)?
        .into_inner();

    Ok(response.compiler_versions)
}
