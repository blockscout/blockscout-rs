#![allow(dead_code)]

mod database_helpers;
pub mod smart_contract_veriifer_mock;
mod test_input_data;

use blockscout_display_bytes::Bytes as DisplayBytes;
use database_helpers::TestDbGuard;
use entity::{
    bytecode_parts, bytecodes, files, parts, sea_orm_active_enums, source_files, sources,
    verified_contracts,
};
use eth_bytecode_db::verification::{
    BytecodeType, Client, Error, Source, SourceType, VerificationRequest,
};
use pretty_assertions::assert_eq;
use sea_orm::{DatabaseConnection, EntityTrait};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::VerifyResponse;
use smart_contract_veriifer_mock::SmartContractVerifierServer;
use std::{collections::HashSet, future::Future, str::FromStr, sync::Arc};
use test_input_data::TestInputData;
use tonic::transport::Uri;

pub trait VerifierService<GrpcT, Request>
where
    GrpcT: From<Request>,
{
    fn add_into_service(&mut self, request: GrpcT, response: VerifyResponse);

    fn build_server(self) -> SmartContractVerifierServer;

    fn generate_request(&self, id: u8) -> Request;

    fn source_type(&self) -> SourceType;

    fn init_service(&mut self, input_data: Vec<TestInputData<Request>>) {
        for input in input_data {
            let response = input.response.clone();
            let request = GrpcT::from(input.request);
            self.add_into_service(request, response)
        }
    }
}

pub fn generate_verification_request<T>(id: u8, content: T) -> VerificationRequest<T> {
    VerificationRequest {
        bytecode: DisplayBytes::from([id]).to_string(),
        bytecode_type: BytecodeType::CreationInput,
        compiler_version: "compiler_version".to_string(),
        content,
    }
}

async fn init_db(db_prefix: &str, test_name: &str) -> TestDbGuard {
    #[allow(unused_variables)]
    let db_url: Option<String> = None;
    // Uncomment if providing url explicitly is more convenient
    let db_url = Some("postgres://postgres:admin@localhost:9432/".into());
    let db_name = format!("{}_{}", db_prefix, test_name);
    TestDbGuard::new(db_name.as_str(), db_url).await
}

async fn start_server_and_init_client_new<GrpcT, Request>(
    service: impl VerifierService<GrpcT, Request>,
    db_client: Arc<DatabaseConnection>,
) -> Client
where
    GrpcT: From<Request>,
{
    let server_addr = service.build_server().start().await;

    let uri = Uri::from_str(&format!("http://{}", server_addr.to_string().as_str()))
        .expect("Returned server address is invalid Uri");
    Client::new_arc(db_client, uri)
        .await
        .expect("Client initialization failed")
}

pub async fn returns_valid_source<GrpcT, Request, F, Fut>(
    db_prefix: &str,
    mut service: impl VerifierService<GrpcT, Request>,
    verify: F,
) where
    F: Fn(Client, Request) -> Fut,
    Fut: Future<Output = Result<Source, Error>>,
    Request: Clone,
    GrpcT: From<Request>,
{
    let db = init_db(db_prefix, "returns_valid_source").await;
    let input_data =
        test_input_data::input_data_1(service.generate_request(1), service.source_type());
    service.init_service(vec![input_data.clone()]);
    let client = start_server_and_init_client_new(service, db.client().clone()).await;

    let source = verify(client, input_data.request)
        .await
        .expect("Verification failed");

    assert_eq!(input_data.source, source, "Invalid source");
}

pub async fn test_data_is_added_into_database<GrpcT, Request, F, Fut>(
    db_prefix: &str,
    mut service: impl VerifierService<GrpcT, Request>,
    verify: F,
) where
    F: Fn(Client, Request) -> Fut,
    Fut: Future<Output = Result<Source, Error>>,
    Request: Clone,
    GrpcT: From<Request>,
{
    let source_type = service.source_type();
    let db = init_db(db_prefix, "test_data_is_added_into_database").await;
    let input_data = test_input_data::input_data_1(service.generate_request(1), source_type);
    service.init_service(vec![input_data.clone()]);
    let client = start_server_and_init_client_new(service, db.client().clone()).await;

    let _source = verify(client, input_data.request)
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

pub async fn historical_data_is_added_into_database<GrpcT, Request, F, Fut>(
    db_prefix: &str,
    mut service: impl VerifierService<GrpcT, Request>,
    verify: F,
    verification_settings: serde_json::Value,
    verification_type: sea_orm_active_enums::VerificationType,
) where
    F: Fn(Client, Request) -> Fut,
    Fut: Future<Output = Result<Source, Error>>,
    Request: Clone,
    GrpcT: From<Request>,
{
    let source_type = service.source_type();
    let db = init_db(db_prefix, "historical_data_is_added_into_database").await;
    let input_data = test_input_data::input_data_1(service.generate_request(1), source_type);
    service.init_service(vec![input_data.clone()]);
    let client = start_server_and_init_client_new(service, db.client().clone()).await;

    let _source = verify(client, input_data.request)
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
    println!(
        "{}",
        serde_json::to_string(&verified_contract.verification_settings).unwrap()
    );
    assert_eq!(
        verification_settings, verified_contract.verification_settings,
        "Invalid verificaiton settings"
    );
    assert_eq!(
        verification_type, verified_contract.verification_type,
        "Invalid verification type"
    );
}
