#![allow(dead_code)]

pub mod smart_contract_veriifer_mock;
pub mod test_input_data;

pub mod verifier_alliance_types;

use async_trait::async_trait;
use blockscout_display_bytes::Bytes as DisplayBytes;
use blockscout_service_launcher::test_database::TestDbGuard;
use entity::{
    bytecode_parts, bytecodes, events, files, parts, sea_orm_active_enums, source_files, sources,
    verified_contracts,
};
use eth_bytecode_db::verification::{
    BytecodeType, Client, Error, Source, SourceType, VerificationMetadata, VerificationRequest,
};
use pretty_assertions::assert_eq;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::VerifyResponse;
use smart_contract_veriifer_mock::SmartContractVerifierServer;
use std::{collections::HashSet, str::FromStr, sync::Arc};
use test_input_data::TestInputData;
use tonic::transport::Uri;

#[async_trait]
pub trait VerifierService<Request> {
    type GrpcT: From<Request>;

    fn add_into_service(&mut self, request: Self::GrpcT, response: VerifyResponse);

    fn build_server(self) -> SmartContractVerifierServer;

    fn generate_request(
        &self,
        id: u8,
        verification_metadata: Option<VerificationMetadata>,
    ) -> Request;

    fn source_type(&self) -> SourceType;

    async fn verify(client: Client, request: Request) -> Result<Source, Error>;
}

pub fn generate_verification_request<T>(
    id: u8,
    content: T,
    metadata: Option<VerificationMetadata>,
) -> VerificationRequest<T> {
    VerificationRequest {
        bytecode: DisplayBytes::from([id]).to_string(),
        bytecode_type: BytecodeType::CreationInput,
        compiler_version: "compiler_version".to_string(),
        content,
        metadata,
        is_authorized: false,
    }
}

pub async fn init_db(db_prefix: &str, test_name: &str) -> TestDbGuard {
    let db_name = format!("{db_prefix}_{test_name}");
    TestDbGuard::new::<migration::Migrator>(db_name.as_str()).await
}

pub async fn start_server_and_init_client<Service, Request>(
    db_client: Arc<DatabaseConnection>,
    mut service: Service,
    input_data: Vec<TestInputData<Request>>,
) -> Client
where
    Service: VerifierService<Request>,
{
    // Initialize service
    for input in input_data {
        let response = input.verifier_response.clone();
        let request = Service::GrpcT::from(input.eth_bytecode_db_request);
        service.add_into_service(request, response)
    }
    // Initialize server
    let server_addr = service.build_server().start().await;

    let uri = Uri::from_str(&format!("http://{}", server_addr.to_string().as_str()))
        .expect("Returned server address is invalid Uri");
    Client::new_arc(db_client, uri)
        .await
        .expect("Client initialization failed")
}

pub async fn test_returns_valid_source<Service, Request>(db_prefix: &str, service: Service)
where
    Service: VerifierService<Request>,
    Request: Clone,
{
    let db = init_db(db_prefix, "test_returns_valid_source").await;
    let input_data =
        test_input_data::input_data_1(service.generate_request(1, None), service.source_type());
    let client =
        start_server_and_init_client(db.client().clone(), service, vec![input_data.clone()]).await;

    let source = Service::verify(client, input_data.eth_bytecode_db_request)
        .await
        .expect("Verification failed");

    assert_eq!(input_data.eth_bytecode_db_source, source, "Invalid source");
}

pub async fn test_data_is_added_into_database<Service, Request>(db_prefix: &str, service: Service)
where
    Request: Clone,
    Service: VerifierService<Request>,
{
    let source_type = service.source_type();
    let db = init_db(db_prefix, "test_data_is_added_into_database").await;
    let input_data = test_input_data::input_data_1(service.generate_request(1, None), source_type);
    let client =
        start_server_and_init_client(db.client().clone(), service, vec![input_data.clone()]).await;

    let _source = Service::verify(client, input_data.eth_bytecode_db_request)
        .await
        .expect("Verification failed");

    let db_client = db.client();
    let db_client = db_client.as_ref();

    /* Assert inserted into "sources" */

    let sources = sources::Entity::find()
        .all(db_client)
        .await
        .expect("Error while reading source");
    assert_eq!(
        1,
        sources.len(),
        "Invalid number of sources returned. Expected 1, actual {}",
        sources.len()
    );
    let db_source = &sources[0];
    assert_eq!(
        sea_orm_active_enums::SourceType::from(source_type),
        db_source.source_type,
        "Invalid source type"
    );
    assert_eq!(
        "compiler_version", db_source.compiler_version,
        "Invalid compiler version"
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>("{ \"language\": \"Solidity\" }").unwrap(),
        db_source.compiler_settings,
        "Invalid compiler settings"
    );
    assert_eq!("source_file1.sol", db_source.file_name, "Invalid file name");
    assert_eq!(
        "contract_name", db_source.contract_name,
        "Invalid contract name"
    );
    assert_eq!(
        Some(serde_json::from_str::<serde_json::Value>("{ \"abi\": \"metadata\" }").unwrap()),
        db_source.abi,
        "Invalid abi"
    );
    assert_eq!(
        Some(
            serde_json::from_str::<serde_json::Value>("{\"userdoc\":{\"kind\":\"user\"}}").unwrap()
        ),
        db_source.compilation_artifacts,
        "Invalid compilation artifacts"
    );
    assert_eq!(
        Some(
            serde_json::from_str::<serde_json::Value>(
                "{\"sourceMap\":\"1:2:3:-:0;;;;;;;;;;;;;;;;;;;\"}"
            )
            .unwrap()
        ),
        db_source.creation_input_artifacts,
        "Invalid creation input artifacts"
    );
    assert_eq!(
        Some(
            serde_json::from_str::<serde_json::Value>(
                "{\"sourceMap\":\"10:11:12:-:0;;;;;;;;;;;;;;;;;;;\"}"
            )
            .unwrap()
        ),
        db_source.deployed_bytecode_artifacts,
        "Invalid deployed bytecode artifacts"
    );
    assert_eq!(
        vec![0x01u8, 0x23u8, 0x45u8, 0x67u8],
        db_source.raw_creation_input,
        "Invalid raw creation input"
    );
    assert_eq!(
        vec![0x89u8, 0xabu8, 0xcdu8, 0xefu8],
        db_source.raw_deployed_bytecode,
        "Invalid raw deployed bytecode"
    );

    /* Assert inserted into "files" */

    let files = files::Entity::find()
        .all(db_client)
        .await
        .expect("Error while reading files");
    assert_eq!(
        2,
        files.len(),
        "Invalid number of files returned. Expected 2, actual {}",
        files.len()
    );
    assert!(
        files
            .iter()
            .any(|value| value.name == "source_file1.sol" && value.content == "content1"),
        "Source file 1 has not been added into 'files'"
    );
    assert!(
        files
            .iter()
            .any(|value| value.name == "source_file2.sol" && value.content == "content2"),
        "Source file 1 has not been added into 'files'"
    );

    /* Assert inserted into "source_files" */

    let source_files = source_files::Entity::find()
        .all(db_client)
        .await
        .expect("Error while reading source files");
    assert_eq!(
        2,
        source_files.len(),
        "Invalid number of source files returned. Expected 2, actual {}",
        source_files.len()
    );
    assert!(
        source_files
            .iter()
            .all(|value| value.source_id == db_source.id),
        "Invalid source id in retrieved source files"
    );
    let expected_file_ids = files.iter().map(|file| file.id).collect::<HashSet<_>>();
    assert_eq!(
        expected_file_ids,
        source_files
            .iter()
            .map(|value| value.file_id)
            .collect::<HashSet<_>>(),
        "Invalid file ids in retrieved source files"
    );

    /* Assert inserted into "bytecodes" */

    let bytecodes = bytecodes::Entity::find()
        .all(db_client)
        .await
        .expect("Error while reading bytecodes");
    assert_eq!(
        2,
        bytecodes.len(),
        "Invalid number of bytecodes returned. Expected 2, actual {}",
        bytecodes.len()
    );
    assert!(
        bytecodes
            .iter()
            .all(|value| value.source_id == db_source.id),
        "Invalid source id in retrieved bytecodes"
    );
    let expected_bytecode_types = [
        sea_orm_active_enums::BytecodeType::CreationInput,
        sea_orm_active_enums::BytecodeType::DeployedBytecode,
    ];
    assert!(
        expected_bytecode_types.iter().all(|expected| bytecodes
            .iter()
            .any(|bytecode| &bytecode.bytecode_type == expected)),
        "Invalid bytecode types in retrieved bytecodes"
    );

    /* Assert inserted into parts */

    let parts = parts::Entity::find()
        .all(db_client)
        .await
        .expect("Error while reading parts");
    assert_eq!(
        4,
        parts.len(),
        "Invalid number of parts returned. Expected 4, actual {}",
        parts.len()
    );
    let expected_main_parts_data = HashSet::from([vec![0x01u8, 0x23u8], vec![0x89u8, 0xabu8]]);
    assert_eq!(
        expected_main_parts_data,
        parts
            .iter()
            .filter(|part| part.part_type == sea_orm_active_enums::PartType::Main)
            .map(|part| part.data.clone())
            .collect::<HashSet<_>>(),
        "Invalid data returned for main parts"
    );
    let expected_meta_parts_data = HashSet::from([vec![0x45u8, 0x67u8], vec![0xcdu8, 0xefu8]]);
    assert_eq!(
        expected_meta_parts_data,
        parts
            .iter()
            .filter(|part| part.part_type == sea_orm_active_enums::PartType::Metadata)
            .map(|part| part.data.clone())
            .collect::<HashSet<_>>(),
        "Invalid data returned for meta parts"
    );

    /* Assert inserted into bytecode_parts */

    let bytecode_parts = bytecode_parts::Entity::find()
        .all(db_client)
        .await
        .expect("Error while reading bytecode parts");
    assert_eq!(
        4,
        bytecode_parts.len(),
        "Invalid number of bytecode parts returned. Expected 4, actual {}",
        bytecode_parts.len()
    );

    let creation_bytecode_id = bytecodes
        .iter()
        .filter(|bytecode| {
            bytecode.bytecode_type == sea_orm_active_enums::BytecodeType::CreationInput
        })
        .map(|bytecode| bytecode.id)
        .next()
        .unwrap();
    let deployed_bytecode_id = bytecodes
        .iter()
        .filter(|bytecode| {
            bytecode.bytecode_type == sea_orm_active_enums::BytecodeType::DeployedBytecode
        })
        .map(|bytecode| bytecode.id)
        .next()
        .unwrap();
    let creation_main_part_id = parts
        .iter()
        .filter(|part| part.data == vec![0x01u8, 0x23u8])
        .map(|part| part.id)
        .next()
        .unwrap();
    let creation_meta_part_id = parts
        .iter()
        .filter(|part| part.data == vec![0x45u8, 0x67u8])
        .map(|part| part.id)
        .next()
        .unwrap();
    let deployed_main_part_id = parts
        .iter()
        .filter(|part| part.data == vec![0x89u8, 0xabu8])
        .map(|part| part.id)
        .next()
        .unwrap();
    let deployed_meta_part_id = parts
        .iter()
        .filter(|part| part.data == vec![0xcdu8, 0xefu8])
        .map(|part| part.id)
        .next()
        .unwrap();
    assert!(
        bytecode_parts
            .iter()
            .any(
                |bytecode_part| bytecode_part.bytecode_id == creation_bytecode_id
                    && bytecode_part.order == 0
                    && bytecode_part.part_id == creation_main_part_id
            ),
        "Invalid creation input main bytecode part"
    );
    assert!(
        bytecode_parts
            .iter()
            .any(
                |bytecode_part| bytecode_part.bytecode_id == creation_bytecode_id
                    && bytecode_part.order == 1
                    && bytecode_part.part_id == creation_meta_part_id
            ),
        "Invalid creation input meta bytecode part"
    );
    assert!(
        bytecode_parts
            .iter()
            .any(
                |bytecode_part| bytecode_part.bytecode_id == deployed_bytecode_id
                    && bytecode_part.order == 0
                    && bytecode_part.part_id == deployed_main_part_id
            ),
        "Invalid deployed bytecode main bytecode part"
    );
    assert!(
        bytecode_parts
            .iter()
            .any(
                |bytecode_part| bytecode_part.bytecode_id == deployed_bytecode_id
                    && bytecode_part.order == 1
                    && bytecode_part.part_id == deployed_meta_part_id
            ),
        "Invalid deployed bytecode meta bytecode part"
    );
}

pub async fn test_historical_data_is_added_into_database<Service, Request>(
    db_prefix: &str,
    service: Service,
    mut verification_settings: serde_json::Value,
    verification_type: sea_orm_active_enums::VerificationType,
) where
    Request: Clone,
    Service: VerifierService<Request>,
{
    let source_type = service.source_type();
    let db = init_db(db_prefix, "test_historical_data_is_added_into_database").await;
    let input_data = test_input_data::input_data_1(service.generate_request(1, None), source_type);
    let client =
        start_server_and_init_client(db.client().clone(), service, vec![input_data.clone()]).await;

    let _source = Service::verify(client, input_data.eth_bytecode_db_request)
        .await
        .expect("Verification failed");

    let db_client = db.client();
    let db_client = db_client.as_ref();

    let source_id = sources::Entity::find()
        .one(db_client)
        .await
        .expect("Error while reading source")
        .unwrap()
        .id;

    let verified_contracts = verified_contracts::Entity::find()
        .all(db_client)
        .await
        .expect("Error while reading verified contracts");
    assert_eq!(
        1,
        verified_contracts.len(),
        "Invalid number of verified contracts returned. Expected 1, actual {}",
        verified_contracts.len()
    );
    let verified_contract = &verified_contracts[0];

    assert_eq!(source_id, verified_contract.source_id, "Invalid source id");
    assert_eq!(
        vec![0x01u8],
        verified_contract.raw_bytecode,
        "Invalid raw bytecode"
    );
    assert_eq!(
        sea_orm_active_enums::BytecodeType::CreationInput,
        verified_contract.bytecode_type,
        "Invalid bytecode type"
    );
    verification_settings
        .as_object_mut()
        .expect("Verification settings is not a map")
        .insert("metadata".into(), serde_json::Value::Null);
    assert_eq!(
        verification_settings, verified_contract.verification_settings,
        "Invalid verification settings"
    );
    assert_eq!(
        verification_type, verified_contract.verification_type,
        "Invalid verification type"
    );
}

pub async fn test_historical_data_saves_chain_id_and_contract_address<Service, Request>(
    db_prefix: &str,
    service: Service,
) where
    Request: Clone,
    Service: VerifierService<Request>,
{
    let source_type = service.source_type();
    let db = init_db(
        db_prefix,
        "test_historical_data_saves_chain_id_and_contract_address",
    )
    .await;
    let chain_id = 1;
    let contract_address = bytes::Bytes::from([10u8; 20].as_ref());
    let input_data = test_input_data::input_data_1(
        service.generate_request(
            1,
            Some(VerificationMetadata {
                chain_id: Some(chain_id),
                contract_address: Some(contract_address.clone()),
                transaction_hash: None,
                block_number: None,
                transaction_index: None,
                deployer: None,
                creation_code: None,
                runtime_code: None,
            }),
        ),
        source_type,
    );
    let client =
        start_server_and_init_client(db.client().clone(), service, vec![input_data.clone()]).await;

    let _source = Service::verify(client, input_data.eth_bytecode_db_request)
        .await
        .expect("Verification failed");

    let db_client = db.client();
    let db_client = db_client.as_ref();

    let source_id = sources::Entity::find()
        .one(db_client)
        .await
        .expect("Error while reading source")
        .unwrap()
        .id;

    let verified_contract = verified_contracts::Entity::find()
        .filter(verified_contracts::Column::SourceId.eq(source_id))
        .one(db_client)
        .await
        .expect("Error while reading verified contracts")
        .expect("No contract was found");

    assert_eq!(
        Some(chain_id),
        verified_contract.chain_id,
        "Invalid chain id saved"
    );
    assert_eq!(
        Some(contract_address.to_vec()),
        verified_contract.contract_address,
        "Invalid contract address saved"
    );
}

pub async fn test_verification_of_same_source_results_stored_once<Service, Request>(
    db_prefix: &str,
    service: Service,
) where
    Request: Clone,
    Service: VerifierService<Request>,
{
    let source_type = service.source_type();
    let db = init_db(
        db_prefix,
        "test_verification_of_same_source_results_stored_once",
    )
    .await;
    let input_data = test_input_data::input_data_1(service.generate_request(1, None), source_type);
    let client =
        start_server_and_init_client(db.client().clone(), service, vec![input_data.clone()]).await;

    let source = Service::verify(client.clone(), input_data.eth_bytecode_db_request.clone())
        .await
        .expect("Verification failed");

    let source_2 = Service::verify(client, input_data.eth_bytecode_db_request)
        .await
        .expect("Duplicative verification failed");

    assert_eq!(
        source, source_2,
        "The same requests must return the same responses"
    );

    let db_client = db.client();
    let db_client = db_client.as_ref();

    /* Assert inserted into "sources" */

    let sources = sources::Entity::find()
        .all(db_client)
        .await
        .expect("Error while reading source");
    assert_eq!(
        1,
        sources.len(),
        "Invalid number of sources returned. Expected 1, actual {}",
        sources.len()
    );

    /* Assert inserted into "files" */

    let files = files::Entity::find()
        .all(db_client)
        .await
        .expect("Error while reading files");
    assert_eq!(
        2,
        files.len(),
        "Invalid number of files returned. Expected 2, actual {}",
        files.len()
    );

    /* Assert inserted into "source_files" */

    let source_files = source_files::Entity::find()
        .all(db_client)
        .await
        .expect("Error while reading source files");
    assert_eq!(
        2,
        source_files.len(),
        "Invalid number of source files returned. Expected 2, actual {}",
        source_files.len()
    );

    /* Assert inserted into "bytecodes" */

    let bytecodes = bytecodes::Entity::find()
        .all(db_client)
        .await
        .expect("Error while reading bytecodes");
    assert_eq!(
        2,
        bytecodes.len(),
        "Invalid number of bytecodes returned. Expected 2, actual {}",
        bytecodes.len()
    );

    /* Assert inserted into parts */

    let parts = parts::Entity::find()
        .all(db_client)
        .await
        .expect("Error while reading parts");
    assert_eq!(
        4,
        parts.len(),
        "Invalid number of parts returned. Expected 4, actual {}",
        parts.len()
    );

    /* Assert inserted into bytecode_parts */

    let bytecode_parts = bytecode_parts::Entity::find()
        .all(db_client)
        .await
        .expect("Error while reading bytecode parts");
    assert_eq!(
        4,
        bytecode_parts.len(),
        "Invalid number of bytecode parts returned. Expected 4, actual {}",
        bytecode_parts.len()
    );
}

pub async fn test_verification_of_updated_source_replace_the_old_result<Service, Request>(
    db_prefix: &str,
    service_generator: impl Fn() -> Service,
) where
    Request: Clone,
    Service: VerifierService<Request>,
{
    let db = init_db(
        db_prefix,
        "test_verification_of_updated_source_replace_the_old_result",
    )
    .await;

    {
        let service = service_generator();
        let source_type = service.source_type();
        let input_data =
            test_input_data::input_data_1(service.generate_request(1, None), source_type);
        let client =
            start_server_and_init_client(db.client().clone(), service, vec![input_data.clone()])
                .await;
        let _source = Service::verify(client.clone(), input_data.eth_bytecode_db_request.clone())
            .await
            .expect("Verification failed");
    }

    let updated_service = service_generator();
    let source_type = updated_service.source_type();
    let updated_input_data = {
        let TestInputData {
            eth_bytecode_db_request,
            verifier_response: mut updated_verifier_response,
            ..
        } = test_input_data::input_data_1(updated_service.generate_request(1, None), source_type);
        if let Some(source) = updated_verifier_response.source.as_mut() {
            let mut compilation_artifacts: serde_json::Value =
                serde_json::from_str(source.compilation_artifacts()).unwrap();
            compilation_artifacts
                .as_object_mut()
                .unwrap()
                .insert("additionalValue".to_string(), serde_json::Value::default());
            source.compilation_artifacts = Some(compilation_artifacts.to_string());
        }

        TestInputData::from_verifier_source_and_extra_data(
            eth_bytecode_db_request,
            updated_verifier_response.source.unwrap(),
            updated_verifier_response.extra_data.unwrap(),
        )
    };
    let client = start_server_and_init_client(
        db.client().clone(),
        updated_service,
        vec![updated_input_data.clone()],
    )
    .await;
    let source = Service::verify(
        client.clone(),
        updated_input_data.eth_bytecode_db_request.clone(),
    )
    .await
    .expect("Verification failed");

    assert_eq!(
        updated_input_data.eth_bytecode_db_source, source,
        "Invalid source"
    );

    let db_client = db.client();
    let db_client_ref = db_client.as_ref();

    /* Assert inserted into "sources" */

    let db_source = sources::Entity::find()
        .one(db_client_ref)
        .await
        .expect("Error while reading source")
        .expect("No sources when there should be one");

    assert_eq!(
        updated_input_data
            .verifier_response
            .source
            .unwrap()
            .compilation_artifacts
            .as_ref()
            .map(|v| serde_json::from_str(v).unwrap()),
        db_source.compilation_artifacts,
        "Invalid compilation artifacts"
    );
}

pub async fn test_verification_inserts_event_descriptions<Service, Request>(
    db_prefix: &str,
    service_generator: impl Fn() -> Service,
) where
    Request: Clone,
    Service: VerifierService<Request>,
{
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    struct Event {
        pub selector: Vec<u8>,
        pub name: String,
        pub inputs: String,
    }

    impl From<events::Model> for Event {
        fn from(value: events::Model) -> Self {
            Self {
                selector: value.selector,
                name: value.name,
                inputs: value.inputs.to_string(),
            }
        }
    }

    let db = init_db(db_prefix, "test_verification_inserts_event_descriptions").await;

    let service = service_generator();
    let source_type = service.source_type();
    let input_data = test_input_data::input_data_1(service.generate_request(1, None), source_type);

    let mut expected_events = HashSet::new();
    {
        let abi = r#"[{"inputs":[{"internalType":"uint256","name":"val","type":"uint256"}],"stateMutability":"nonpayable","type":"constructor"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"uint256","name":"a","type":"uint256"}],"name":"A","type":"event"},{"anonymous":true,"inputs":[{"indexed":false,"internalType":"uint256","name":"start","type":"uint256"},{"indexed":false,"internalType":"uint256","name":"middle","type":"uint256"},{"indexed":false,"internalType":"uint256","name":"end","type":"uint256"}],"name":"Anonymous","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"internalType":"string","name":"a","type":"string"},{"indexed":true,"internalType":"uint256","name":"b","type":"uint256"},{"indexed":true,"internalType":"uint256","name":"c","type":"uint256"},{"indexed":true,"internalType":"bytes","name":"d","type":"bytes"}],"name":"B","type":"event"},{"stateMutability":"payable","type":"fallback"},{"inputs":[],"name":"f","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"stateMutability":"payable","type":"receive"}]"#;
        expected_events.insert(Event {
            selector: blockscout_display_bytes::Bytes::from_str(
                "0xa17a9e66f0c355e3aa3b9ea969991204d6b1d2e62a47877f612cb2371d79e06a",
            )
            .unwrap()
            .to_vec(),
            name: "A".to_string(),
            inputs: serde_json::Value::from_str(
                r#"[{"indexed":true,"internalType":"uint256","name":"a","type":"uint256"}]"#,
            )
            .unwrap()
            .to_string(),
        });
        expected_events.insert(Event {
            selector: blockscout_display_bytes::Bytes::from_str("0xbcf5c814cb65249e306ec7aeaef6fc1ca35e1b8e18c08b054c9f9c76160bc923").unwrap().to_vec(),
            name: "B".to_string(),
            inputs: serde_json::Value::from_str(r#"[{"indexed":false,"internalType":"string","name":"a","type":"string"},{"indexed":true,"internalType":"uint256","name":"b","type":"uint256"},{"indexed":true,"internalType":"uint256","name":"c","type":"uint256"},{"indexed":true,"internalType":"bytes","name":"d","type":"bytes"}]"#).unwrap().to_string(),
        });

        let input_data = input_data.clone().with_abi(abi.to_string());
        let client = start_server_and_init_client(
            db.client().clone(),
            service_generator(),
            vec![input_data.clone()],
        )
        .await;
        let _source = Service::verify(client.clone(), input_data.eth_bytecode_db_request)
            .await
            .expect("Verification failed");

        let db_events: HashSet<_> = events::Entity::find()
            .all(db.client().as_ref())
            .await
            .expect("Error while reading events")
            .into_iter()
            .map(Event::from)
            .collect();

        assert_eq!(
            expected_events, db_events,
            "Events after first submission do not match"
        );
    }

    {
        let abi = r#"[{"inputs":[{"internalType":"uint256","name":"val","type":"uint256"}],"stateMutability":"nonpayable","type":"constructor"},{"anonymous":true,"inputs":[{"indexed":false,"internalType":"uint256","name":"start","type":"uint256"},{"indexed":false,"internalType":"uint256","name":"middle","type":"uint256"},{"indexed":false,"internalType":"uint256","name":"end","type":"uint256"}],"name":"Anonymous","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"string","name":"a","type":"string"},{"indexed":false,"internalType":"uint256","name":"b","type":"uint256"},{"indexed":true,"internalType":"uint256","name":"c","type":"uint256"},{"indexed":true,"internalType":"bytes","name":"d","type":"bytes"}],"name":"B","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"internalType":"string","name":"c","type":"string"}],"name":"C","type":"event"},{"stateMutability":"payable","type":"fallback"},{"stateMutability":"payable","type":"receive"},{"inputs":[],"name":"f","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"}]"#;
        expected_events.insert(Event {
            selector: blockscout_display_bytes::Bytes::from_str("0xbcf5c814cb65249e306ec7aeaef6fc1ca35e1b8e18c08b054c9f9c76160bc923").unwrap().to_vec(),
            name: "B".to_string(),
            inputs: serde_json::Value::from_str(r#"[{"indexed":true,"internalType":"string","name":"a","type":"string"},{"indexed":false,"internalType":"uint256","name":"b","type":"uint256"},{"indexed":true,"internalType":"uint256","name":"c","type":"uint256"},{"indexed":true,"internalType":"bytes","name":"d","type":"bytes"}]"#).unwrap().to_string(),
        });
        expected_events.insert(Event {
            selector: blockscout_display_bytes::Bytes::from_str(
                "0x7076fab50c7b30ea53db9880b1c8ea59a80cdaf0341135a4c2ec691b8cdd4a9a",
            )
            .unwrap()
            .to_vec(),
            name: "C".to_string(),
            inputs: serde_json::Value::from_str(
                r#"[{"indexed":false,"internalType":"string","name":"c","type":"string"}]"#,
            )
            .unwrap()
            .to_string(),
        });

        let input_data = input_data.clone().with_abi(abi.to_string());
        let client = start_server_and_init_client(
            db.client().clone(),
            service_generator(),
            vec![input_data.clone()],
        )
        .await;
        let _source = Service::verify(client.clone(), input_data.eth_bytecode_db_request)
            .await
            .expect("Verification failed");

        let db_events: HashSet<_> = events::Entity::find()
            .all(db.client().as_ref())
            .await
            .expect("Error while reading events")
            .into_iter()
            .map(Event::from)
            .collect();

        assert_eq!(
            expected_events, db_events,
            "Events after second submission do not match"
        );
    }
}
