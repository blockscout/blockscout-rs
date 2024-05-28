use assert_str::assert_str_eq;
use bytes::Bytes;
use serde::Deserialize;
use serde_json::json;
use serde_with::serde_as;
use std::{collections::BTreeMap, fs, path::PathBuf, str::from_utf8};
use walkdir::WalkDir;

const CONTRACTS_DIR: &str = "tests/contracts";
const SAMPLES_DIR: &str = "tests/samples";

fn get_dir_files(project_path: &PathBuf) -> BTreeMap<PathBuf, String> {
    let mut sources = BTreeMap::new();

    if project_path.is_dir() {
        for entry in WalkDir::new(project_path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let relative_path = entry
                .path()
                .strip_prefix(project_path)
                .expect("Failed to strip prefix")
                .to_path_buf();
            if entry.path().is_file() {
                let content = fs::read_to_string(entry.path()).unwrap();
                sources.insert(relative_path, content);
            }
        }
    } else {
        let content = fs::read_to_string(project_path).unwrap();
        sources.insert(project_path.clone(), content);
    }

    sources
}

#[serde_as]
#[derive(Deserialize)]
struct Response {
    #[serde_as(as = "serde_with::base64::Base64")]
    svg: Bytes,
}

async fn test_setup(request: serde_json::Value, route: &str) -> reqwest::Response {
    let mut url = super::init_server().await;
    url.set_path(route);

    reqwest::Client::new()
        .post(url)
        .json(&request)
        .send()
        .await
        .expect("failed to send request")
}

async fn visualize_contract_success(request: serde_json::Value, expected_svg: String) {
    let response = test_setup(request, "/api/v1/solidity:visualize-contracts").await;
    assert!(
        response.status().is_success(),
        "response: {:?}",
        response.text().await
    );
    let result: Response = response
        .json()
        .await
        .expect("could not deserialize response");

    let result_svg = from_utf8(&result.svg).expect("failed to convert result svg to string");

    assert_str_eq!(result_svg, expected_svg);
}

async fn visualize_contracts_success_from_dir(project_name: &str, sample_name: &str) {
    let project_path = PathBuf::from(format!("{CONTRACTS_DIR}/{project_name}",));

    let request = json!({
        "sources": get_dir_files(&project_path),
    });
    let svg_path = format!("{SAMPLES_DIR}/uml/{sample_name}.svg");
    let expected_svg = fs::read_to_string(&svg_path)
        .unwrap_or_else(|_| panic!("Error while reading {sample_name}.svg",));
    visualize_contract_success(request, expected_svg).await;
}

async fn visualize_storage_success(request: serde_json::Value, expected_svg: String) {
    let response = test_setup(request, "/api/v1/solidity:visualize-storage").await;

    assert!(
        response.status().is_success(),
        "response: {:?}",
        response.text().await
    );
    let result: Response = response
        .json()
        .await
        .expect("could not deserialize response");
    let result_svg = from_utf8(&result.svg).expect("failed to convert result svg to string");

    assert_str_eq!(result_svg, expected_svg);
}

async fn visualize_storage_success_from_dir(
    project_name: &str,
    main_contract: &str,
    main_contract_filename: &str,
    sample_name: &str,
) {
    let project_path = PathBuf::from(format!("{CONTRACTS_DIR}/{project_name}"));

    let request = json!({
        "sources": get_dir_files(&project_path),
        "contract_name": main_contract,
        "file_name": main_contract_filename,
    });

    let svg_path = format!("{SAMPLES_DIR}/storage/{sample_name}.svg");
    let expected_svg = fs::read_to_string(&svg_path)
        .unwrap_or_else(|_| panic!("Error while reading {sample_name}.svg",));

    visualize_storage_success(request, expected_svg).await;
}

mod success_simple_tests {
    use super::*;

    #[actix_web::test]
    async fn uml_simple_contract() {
        visualize_contracts_success_from_dir("SimpleContract.sol", "simple_contract").await;
    }

    #[actix_web::test]
    async fn storage_simple_contract() {
        visualize_storage_success_from_dir(
            "SimpleContract.sol",
            "SimpleStorage",
            "SimpleContract.sol",
            "simple_contract",
        )
        .await;
    }

    #[actix_web::test]
    async fn storage_simple_contract_alt_path() {
        let contract_path = format!("{CONTRACTS_DIR}/SimpleContract.sol",);
        let storage_path = format!("{SAMPLES_DIR}/storage/simple_contract.svg",);
        let contract =
            fs::read_to_string(&contract_path).expect("Error while reading SimpleContract.sol");
        let storage =
            fs::read_to_string(&storage_path).expect("Error while reading simple_contract.svg");

        let request = json!({
            "sources": {"c/d/SimpleContract.sol": contract},
            "contract_name": "SimpleStorage",
            "file_name": "c/d/SimpleContract.sol",
        });

        visualize_storage_success(request, storage).await;
    }
}

mod success_advanced_tests {
    use super::*;

    #[actix_web::test]
    async fn uml_large_project() {
        visualize_contracts_success_from_dir(
            "large_project_many_methods",
            "large_project_many_methods",
        )
        .await;
    }

    #[actix_web::test]
    async fn storage_large_project() {
        visualize_storage_success_from_dir(
            "large_project_many_methods",
            "MyToken",
            "Token.sol",
            "large_project_many_methods",
        )
        .await;
    }

    #[actix_web::test]
    async fn uml_many_libraries() {
        visualize_contracts_success_from_dir("many_libraries", "many_libraries").await;
    }

    #[actix_web::test]
    async fn uml_same_contract_names() {
        visualize_contracts_success_from_dir("same_contract_names", "same_contract_names").await;
    }

    #[actix_web::test]
    async fn storage_same_contract_names() {
        visualize_storage_success_from_dir(
            "same_contract_names",
            "A",
            "Main.sol",
            "same_contract_names",
        )
        .await;
    }

    #[actix_web::test]
    async fn storage_same_filenames_different_contracts() {
        visualize_storage_success_from_dir(
            "same_filenames_different_contracts",
            "A",
            "SameName.sol",
            "same_filenames_different_contracts",
        )
        .await;
    }

    #[actix_web::test]
    async fn uml_starting_slash() {
        let contract_path = format!("{CONTRACTS_DIR}/SimpleContract.sol",);
        let contract =
            fs::read_to_string(&contract_path).expect("Error while reading SimpleContract.sol");
        let svg_path = format!("{SAMPLES_DIR}/uml/simple_contract.svg",);
        let expected_svg = fs::read_to_string(&svg_path)
            .unwrap_or_else(|_| panic!("Error while reading simple_contract.svg",));
        let request = json!({
            "sources": {
                "/usr/SimpleContract.sol": contract,
            }
        });
        visualize_contract_success(request, expected_svg).await;
    }

    #[actix_web::test]
    async fn uml_empty_file_name() {
        let contract_path = format!("{CONTRACTS_DIR}/SimpleContract.sol",);
        let contract =
            fs::read_to_string(&contract_path).expect("Error while reading SimpleContract.sol");
        let svg_path = format!("{SAMPLES_DIR}/uml/simple_contract.svg",);
        let expected_svg = fs::read_to_string(&svg_path)
            .unwrap_or_else(|_| panic!("Error while reading simple_contract.svg",));
        let request = json!({
            "sources": {
                ".sol": contract,
            }
        });
        visualize_contract_success(request, expected_svg).await;
    }

    // filename that starts with @
    #[actix_web::test]
    async fn uml_starting_at_sign() {
        visualize_contracts_success_from_dir("openzeppelin_lib", "openzeppelin_lib").await;
    }
}

mod success_known_issues {
    use super::*;

    #[actix_web::test]
    async fn uml_contract_with_compile_error() {
        // sol2uml ignores not syntax compile errors
        visualize_contracts_success_from_dir("ContractCompileError.sol", "contract_compile_error")
            .await;
    }

    #[actix_web::test]
    async fn storage_contract_with_compile_error() {
        // sol2uml ignores not syntax compile errors also in storage mod
        visualize_storage_success_from_dir(
            "ContractCompileError.sol",
            "Main",
            "ContractCompileError.sol",
            "contract_compile_error",
        )
        .await;
    }

    #[actix_web::test]
    async fn uml_import_missing_contract() {
        // sol2uml just doesn`t show missing contract on uml diagram
        visualize_contracts_success_from_dir(
            "ImportMissingContract.sol",
            "import_missing_contract",
        )
        .await;
    }

    #[actix_web::test]
    async fn storage_import_missing_contract() {
        // sol2uml ignores missing contract if it doesn`t affect storage
        visualize_storage_success_from_dir(
            "ImportMissingContract.sol",
            "Main",
            "ImportMissingContract.sol",
            "import_missing_contract",
        )
        .await;
    }

    #[actix_web::test]
    async fn uml_import_missing_inherited_contract() {
        // sol2uml just doesn`t show missing contract on uml, even if some of
        // existing contracts is inherited from it
        visualize_contracts_success_from_dir(
            "ImportMissingInheritedContract.sol",
            "import_missing_inherited_contract",
        )
        .await;
    }

    #[actix_web::test]
    async fn uml_import_missing_library() {
        // sol2uml just doesn`t show missing library on uml
        visualize_contracts_success_from_dir("ImportMissingLibrary.sol", "import_missing_library")
            .await;
    }

    #[actix_web::test]
    async fn uml_long_names() {
        visualize_contracts_success_from_dir("LongNames.sol", "long_names").await;
    }

    #[actix_web::test]
    async fn storage_long_names() {
        visualize_storage_success_from_dir("LongNames.sol", "Main", "LongNames.sol", "long_names")
            .await;
    }

    #[actix_web::test]
    async fn storage_same_filenames() {
        // when contracts with the same name have the same filename, then
        // storage will be returned for the contract with the smallest filename in sort order
        visualize_storage_success_from_dir(
            "same_filenames",
            "A",
            "main_dir/SameName.sol",
            "same_filenames",
        )
        .await;
    }
}

mod failure_tests {
    use super::*;

    #[actix_web::test]
    async fn storage_wrong_main_contract() {
        let contract_path = PathBuf::from(format!("{CONTRACTS_DIR}/SimpleContract.sol",));

        let request = json!({
            "sources": get_dir_files(&contract_path),
            "contract_name": "dsd",
            "file_name": "SimpleContract.sol",
        });
        let response = test_setup(request, "/api/v1/solidity:visualize-storage").await;

        assert!(
            response.status().is_client_error(),
            "Invalid status code (failed expected): {}",
            response.status()
        );

        let message = response
            .text()
            .await
            .expect("could not deserialize response text");
        assert!(
            message.contains("Failed to find contract with name"),
            "Invalid response message: {message}",
        );
    }

    #[actix_web::test]
    async fn uml_library_with_syntax_error() {
        let project_path = PathBuf::from(format!("{CONTRACTS_DIR}/library_syntax_error",));

        let request = json!({ "sources": get_dir_files(&project_path) });

        let response = test_setup(request, "/api/v1/solidity:visualize-contracts").await;
        assert!(
            response.status().is_client_error(),
            "Invalid status code (failed expected): {}",
            response.status()
        );
        let err = response
            .text()
            .await
            .expect("could not deserialize response text");
        assert!(
            err.contains("Failed to parse solidity code",),
            "Invalid response, wrong error type: {err}",
        )
    }

    #[actix_web::test]
    async fn storage_import_missing_inherited_contract() {
        // sol2uml returns error if main contract is inherited from missing contract
        // cause it affects main contract storage
        let project_path = PathBuf::from(format!(
            "{CONTRACTS_DIR}/ImportMissingInheritedContract.sol",
        ));

        let request = json!({
            "sources": get_dir_files(&project_path),
            "contract_name": "Main",
            "file_name": "ImportMissingInheritedContract.sol",
        });

        let response = test_setup(request, "/api/v1/solidity:visualize-storage").await;
        assert!(
            response.status().is_client_error(),
            "Invalid status code (failed expected): {}",
            response.status()
        );
        let err = response
            .text()
            .await
            .expect("could not deserialize response text");
        assert!(
            err.contains("Failed to find inherited contract",),
            "Invalid response, wrong error type: {err}",
        )
    }
}
