use sea_orm::DatabaseConnection;
use std::sync::Arc;
use verifier_alliance_database::{insert_contract_deployment, insert_verified_contract};
use verifier_alliance_database_tests::build_all_tests;

async fn initialization(
    database_connection: Arc<DatabaseConnection>,
    test_case: verifier_alliance_database_tests::TestCase,
) {
    let contract_deployment_data = helpers::contract_deployment_data(&test_case);
    let inserted_contract_deployment =
        insert_contract_deployment(&database_connection, contract_deployment_data)
            .await
            .expect("error while inserting contract deployment");

    let verified_contract_data =
        helpers::verified_contract_data(&test_case, inserted_contract_deployment.id);
    insert_verified_contract(&database_connection, verified_contract_data)
        .await
        .expect("error while inserting verified contract");
}

build_all_tests!(initialization);

mod helpers {
    use sea_orm::prelude::Uuid;
    use std::str::FromStr;
    use verification_common::verifier_alliance::Match;
    use verifier_alliance_database::{
        CompiledContract, CompiledContractCompiler, CompiledContractLanguage,
        InsertContractDeployment, VerifiedContract, VerifiedContractMatches,
    };

    pub fn contract_deployment_data(
        test_case: &verifier_alliance_database_tests::TestCase,
    ) -> InsertContractDeployment {
        InsertContractDeployment::Regular {
            chain_id: test_case.chain_id,
            address: test_case.address.clone(),
            transaction_hash: test_case.transaction_hash.clone(),
            block_number: test_case.block_number,
            transaction_index: test_case.transaction_index,
            deployer: test_case.deployer.clone(),
            creation_code: test_case.deployed_creation_code.clone(),
            runtime_code: test_case.deployed_runtime_code.clone(),
        }
    }

    pub fn verified_contract_data(
        test_case: &verifier_alliance_database_tests::TestCase,
        contract_deployment_id: Uuid,
    ) -> VerifiedContract {
        let compiler = CompiledContractCompiler::from_str(&test_case.compiler.to_lowercase())
            .expect("invalid compiler");
        let language = CompiledContractLanguage::from_str(&test_case.language.to_lowercase())
            .expect("invalid language");
        VerifiedContract {
            contract_deployment_id,
            compiled_contract: CompiledContract {
                compiler,
                version: test_case.version.clone(),
                language,
                name: test_case.name.clone(),
                fully_qualified_name: test_case.fully_qualified_name.clone(),
                sources: test_case.sources.clone(),
                compiler_settings: test_case.compiler_settings.clone(),
                compilation_artifacts: test_case.compilation_artifacts.clone(),
                creation_code: test_case.compiled_creation_code.clone(),
                creation_code_artifacts: test_case.creation_code_artifacts.clone(),
                runtime_code: test_case.compiled_runtime_code.clone(),
                runtime_code_artifacts: test_case.runtime_code_artifacts.clone(),
            },
            matches: VerifiedContractMatches::Complete {
                creation_match: Match {
                    metadata_match: test_case.creation_metadata_match,
                    transformations: test_case.creation_transformations.clone(),
                    values: test_case.creation_values.clone(),
                },
                runtime_match: Match {
                    metadata_match: test_case.runtime_metadata_match,
                    transformations: test_case.runtime_transformations.clone(),
                    values: test_case.runtime_values.clone(),
                },
            },
        }
    }
}
