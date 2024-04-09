use crate::{
    verification::{
        smart_contract_verifier,
        types::{AllianceBatchImportResult, AllianceImportRequest},
        Client, Error,
    },
    ToHex,
};
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2 as eth_bytecode_db_v2;
use serde::{Deserialize, Serialize};
use smart_contract_verifier_proto::http_client::solidity_verifier_client;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardJson {
    pub input: String,
}

impl From<AllianceImportRequest<StandardJson>>
    for smart_contract_verifier::BatchVerifySolidityStandardJsonRequest
{
    fn from(value: AllianceImportRequest<StandardJson>) -> Self {
        let contracts = value
            .contracts
            .into_iter()
            .map(|v| smart_contract_verifier::Contract {
                creation_code: v.creation_code.as_ref().map(ToHex::to_hex),
                runtime_code: Some(v.runtime_code.to_hex()),
                metadata: Some(smart_contract_verifier::VerificationMetadata {
                    chain_id: Some(v.chain_id),
                    contract_address: Some(v.contract_address.to_hex()),
                }),
            })
            .collect();
        Self {
            contracts,
            compiler_version: value.compiler_version,
            input: value.content.input,
        }
    }
}

impl TryFrom<eth_bytecode_db_v2::VerifierAllianceBatchImportSolidityStandardJsonRequest>
    for AllianceImportRequest<StandardJson>
{
    type Error = eth_bytecode_db_proto::tonic::Status;

    fn try_from(
        value: eth_bytecode_db_v2::VerifierAllianceBatchImportSolidityStandardJsonRequest,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            contracts: value
                .contracts
                .into_iter()
                .map(TryFrom::try_from)
                .collect::<Result<Vec<_>, _>>()?,
            compiler_version: value.compiler_version,
            content: StandardJson { input: value.input },
        })
    }
}

pub async fn import_solidity_standard_json(
    client: Client,
    request: AllianceImportRequest<StandardJson>,
) -> Result<AllianceBatchImportResult, Error> {
    let deployment_data = request.contracts.clone();

    let verifier_request = request.into();
    let verifier_response = solidity_verifier_client::batch_verify_standard_json(
        &client.verifier_http_client,
        verifier_request,
    )
    .await?;

    let result = super::process_batch_import_response(
        client.alliance_db_client.as_ref().unwrap(),
        verifier_response,
        deployment_data,
    )
    .await?;

    Ok(result)
}
