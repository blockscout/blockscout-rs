use super::verifier;
use crate::types::Route;
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2 as eth_bytecode_db_v2;

macro_rules! impl_route {
    ($name:ident, $route:expr, $request:path, $response:path, $verifier_route:path) => {
        pub struct $name;
        impl Route for $name {
            const ROUTE: &'static str = $route;
            type Request = $request;
            type Response = $response;
            type VerifierRoute = $verifier_route;
        }
    };
}

impl_route!(
    AllianceSolidityMultiPartBatchImport,
    "/api/v2/alliance/solidity/multi-part:batch-import",
    eth_bytecode_db_v2::VerifierAllianceBatchImportSolidityMultiPartRequest,
    eth_bytecode_db_v2::VerifierAllianceBatchImportResponse,
    verifier::SoliditySourcesBatchVerifyMultiPart
);

impl_route!(
    AllianceSolidityStandardJsonBatchImport,
    "/api/v2/alliance/solidity/standard-json:batch-import",
    eth_bytecode_db_v2::VerifierAllianceBatchImportSolidityStandardJsonRequest,
    eth_bytecode_db_v2::VerifierAllianceBatchImportResponse,
    verifier::SoliditySourcesBatchVerifyStandardJson
);

impl_route!(
    SoliditySourcesVerifyMultiPart,
    "/api/v2/verifier/solidity/sources:verify-multi-part",
    eth_bytecode_db_v2::VerifySolidityMultiPartRequest,
    eth_bytecode_db_v2::VerifyResponse,
    verifier::SoliditySourcesVerifyMultiPart
);

impl_route!(
    SoliditySourcesVerifyStandardJson,
    "/api/v2/verifier/solidity/sources:verify-standard-json",
    eth_bytecode_db_v2::VerifySolidityStandardJsonRequest,
    eth_bytecode_db_v2::VerifyResponse,
    verifier::SoliditySourcesVerifyStandardJson
);
