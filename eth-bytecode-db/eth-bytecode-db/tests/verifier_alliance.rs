mod verification_test_helpers;

use async_trait::async_trait;
use eth_bytecode_db::verification::{
    solidity_standard_json, solidity_standard_json::StandardJson, Client, Error, Source,
    SourceType, VerificationMetadata, VerificationRequest,
};
use rstest::{fixture, rstest};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
    VerifyResponse, VerifySolidityStandardJsonRequest,
};
use verification_test_helpers::{
    init_db,
    smart_contract_veriifer_mock::{MockSolidityVerifierService, SmartContractVerifierServer},
    start_server_and_init_client, test_input_data, VerifierService,
};

const DB_PREFIX: &str = "verifier_alliance";

#[async_trait]
impl VerifierService<VerificationRequest<StandardJson>> for MockSolidityVerifierService {
    type GrpcT = VerifySolidityStandardJsonRequest;

    fn add_into_service(
        &mut self,
        request: VerifySolidityStandardJsonRequest,
        response: VerifyResponse,
    ) {
        self.expect_verify_standard_json()
            .withf(move |arg| arg.get_ref() == &request)
            .returning(move |_| Ok(tonic::Response::new(response.clone())));
    }

    fn build_server(self) -> SmartContractVerifierServer {
        SmartContractVerifierServer::new().solidity_service(self)
    }

    fn generate_request(
        &self,
        _id: u8,
        _metadata: Option<VerificationMetadata>,
    ) -> VerificationRequest<StandardJson> {
        unreachable!()
        // generate_verification_request(id, default_request_content(), metadata)
    }

    fn source_type(&self) -> SourceType {
        unreachable!()
    }

    async fn verify(
        client: Client,
        request: VerificationRequest<StandardJson>,
    ) -> Result<Source, Error> {
        solidity_standard_json::verify(client, request).await
    }
}

#[fixture]
fn service() -> MockSolidityVerifierService {
    MockSolidityVerifierService::new()
}

#[rstest]
#[tokio::test]
#[ignore = "Needs database to run"]
pub async fn test_historical_data_is_added_into_database(
    // db_prefix: &str,
    service: MockSolidityVerifierService,
    // mut verification_settings: serde_json::Value,
    // verification_type: sea_orm_active_enums::VerificationType,
) {
    // let source_type = service.source_type();
    let db = init_db(DB_PREFIX, "test_1").await;

    // let request =

    let input_data = test_input_data::input_data_1(service.generate_request(1, None), source_type);

    let input_data =
    let client =
        start_server_and_init_client(db.client().clone(), service, vec![input_data.clone()]).await;

    // let _source = Service::verify(client, input_data.request)
    //     .await
    //     .expect("Verification failed");
    //
    // let db_client = db.client();
    // let db_client = db_client.as_ref();
    //
    // let source_id = sources::Entity::find()
    //     .one(db_client)
    //     .await
    //     .expect("Error while reading source")
    //     .unwrap()
    //     .id;
    //
    // let verified_contracts = verified_contracts::Entity::find()
    //     .all(db_client)
    //     .await
    //     .expect("Error while reading verified contracts");
    // assert_eq!(
    //     1,
    //     verified_contracts.len(),
    //     "Invalid number of verified contracts returned. Expected 1, actual {}",
    //     verified_contracts.len()
    // );
    // let verified_contract = &verified_contracts[0];
    //
    // assert_eq!(source_id, verified_contract.source_id, "Invalid source id");
    // assert_eq!(
    //     vec![0x01u8],
    //     verified_contract.raw_bytecode,
    //     "Invalid raw bytecode"
    // );
    // assert_eq!(
    //     sea_orm_active_enums::BytecodeType::CreationInput,
    //     verified_contract.bytecode_type,
    //     "Invalid bytecode type"
    // );
    // verification_settings
    //     .as_object_mut()
    //     .expect("Verification settings is not a map")
    //     .insert("metadata".into(), serde_json::Value::Null);
    // assert_eq!(
    //     verification_settings, verified_contract.verification_settings,
    //     "Invalid verification settings"
    // );
    // assert_eq!(
    //     verification_type, verified_contract.verification_type,
    //     "Invalid verification type"
    // );
}
