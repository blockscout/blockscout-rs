mod database;
mod health;
mod solidity_verifier;
mod sourcify_verifier;
mod verifier_base;
mod vyper_verifier;

pub use database::DatabaseService;
pub use health::HealthService;
pub use solidity_verifier::SolidityVerifierService;
pub use sourcify_verifier::SourcifyVerifierService;
pub use vyper_verifier::VyperVerifierService;

/****************************************************************************/

const API_KEY_NAME: &str = "x-api-key";

fn is_key_authorized(
    authorized_keys: &std::collections::HashSet<String>,
    metadata: tonic::metadata::MetadataMap,
) -> Result<bool, tonic::Status> {
    let api_key = metadata
        .get(API_KEY_NAME)
        .map(|api_key| api_key.to_str())
        .transpose()
        .map_err(|err| {
            tonic::Status::invalid_argument(format!(
                "invalid api key value ({API_KEY_NAME}): {err}"
            ))
        })?;

    let is_authorized = api_key
        .map(|key| authorized_keys.contains(key))
        .unwrap_or_default();
    Ok(is_authorized)
}

macro_rules! trace_request_metadata {
    ($($field:ident=$value:expr),+) => {
        tracing::info!(
            $($field = $value,)+
        )
    };
}

macro_rules! trace_verification_request {
    ($contract_address:expr, $chain_id:expr) => {{
        $crate::services::trace_request_metadata!(
            chain_id = $chain_id,
            contract_address = $contract_address
        )
    }};
    ($request:expr) => {{
        let chain_id = $request
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.chain_id.clone())
            .unwrap_or_default();
        let contract_address = $request
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.contract_address.clone())
            .unwrap_or_default();
        $crate::services::trace_verification_request!(chain_id, contract_address)
    }};
}

use trace_request_metadata;
use trace_verification_request;
