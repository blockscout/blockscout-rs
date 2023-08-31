mod types;

use mockall::mock;

mock! {
    Middleware<Output: 'static + Send + Sync> {}

    #[async_trait::async_trait]
    impl<Output: 'static + Send + Sync> smart_contract_verifier::Middleware<Output> for Middleware<Output> {
        async fn call(&self, output: &Output) -> ();
    }
}

#[rstest::fixture]
fn middleware<Output: 'static + Send + Sync>() -> MockMiddleware<Output> {
    let mut middleware = MockMiddleware::<Output>::new();
    middleware.expect_call().times(1).return_const(());
    middleware
}

mod solidity {
    use super::*;
    use smart_contract_verifier::{
        solidity, Compilers, ListFetcher, SolidityClient, SolidityCompiler, SoliditySuccess,
        DEFAULT_SOLIDITY_COMPILER_LIST,
    };
    use std::{collections::BTreeMap, sync::Arc};
    use tokio::sync::{OnceCell, Semaphore};
    use types::solidity::VerificationRequest;

    async fn global_compilers() -> &'static Arc<Compilers<SolidityCompiler>> {
        static COMPILERS: OnceCell<Arc<Compilers<SolidityCompiler>>> = OnceCell::const_new();
        COMPILERS
            .get_or_init(|| async {
                let url = DEFAULT_SOLIDITY_COMPILER_LIST
                    .try_into()
                    .expect("Getting url");
                let compilers_dir = tempfile::tempdir().expect("Temp dir creation failed");
                let fetcher = ListFetcher::new(url, compilers_dir.into_path(), None, None)
                    .await
                    .expect("Fetch releases");
                let threads_semaphore = Arc::new(Semaphore::new(4));
                let compilers = Compilers::new(
                    Arc::new(fetcher),
                    SolidityCompiler::new(),
                    threads_semaphore,
                );
                Arc::new(compilers)
            })
            .await
    }

    fn default_request() -> VerificationRequest {
        let deployed_bytecode: &str = "0x6080604052600080fdfea26469706673582212201bc3e5a6822adc0f0b84464a262e0b8b02a4a145e5971e7bce020c5f2334dfcb64736f6c63430008070033";
        let creation_bytecode: &str = "0x6080604052348015600f57600080fd5b50603f80601d6000396000f3fe6080604052600080fdfea26469706673582212201bc3e5a6822adc0f0b84464a262e0b8b02a4a145e5971e7bce020c5f2334dfcb64736f6c63430008070033";
        let compiler_version: &str = "0.8.7+commit.e28d00a7";
        let sources = BTreeMap::from([(
            "A.sol".to_string(),
            "pragma solidity >=0.4.5; contract A {}".to_string(),
        )]);

        VerificationRequest::new(
            deployed_bytecode,
            creation_bytecode,
            compiler_version,
            sources,
            None,
            None,
            None,
        )
        .expect("Invalid verification request")
    }

    #[rstest::rstest]
    #[tokio::test]
    async fn multi_part(middleware: impl smart_contract_verifier::Middleware<SoliditySuccess>) {
        let compilers = global_compilers().await;
        let client = SolidityClient::new_arc(compilers.clone()).with_middleware(middleware);

        let request = default_request();
        solidity::multi_part::verify(
            Arc::new(client),
            solidity::multi_part::VerificationRequest::from(request),
        )
        .await
        .expect("Verification failed");
    }

    #[rstest::rstest]
    #[tokio::test]
    async fn standard_json(middleware: impl smart_contract_verifier::Middleware<SoliditySuccess>) {
        let compilers = global_compilers().await;
        let client = SolidityClient::new_arc(compilers.clone()).with_middleware(middleware);

        let request = default_request();
        solidity::standard_json::verify(
            Arc::new(client),
            solidity::standard_json::VerificationRequest::from(request),
        )
        .await
        .expect("Verification failed");
    }
}

mod vyper {
    use super::*;
    use smart_contract_verifier::{
        vyper, Compilers, ListFetcher, VyperClient, VyperCompiler, VyperSuccess,
        DEFAULT_VYPER_COMPILER_LIST,
    };
    use std::{collections::BTreeMap, sync::Arc};
    use tokio::sync::{OnceCell, Semaphore};
    use types::vyper::VerificationRequest;

    async fn global_compilers() -> &'static Arc<Compilers<VyperCompiler>> {
        static COMPILERS: OnceCell<Arc<Compilers<VyperCompiler>>> = OnceCell::const_new();
        COMPILERS
            .get_or_init(|| async {
                let url = DEFAULT_VYPER_COMPILER_LIST.try_into().expect("Getting url");
                let compilers_dir = tempfile::tempdir().expect("Temp dir creation failed");
                let fetcher = ListFetcher::new(url, compilers_dir.into_path(), None, None)
                    .await
                    .expect("Fetch releases");
                let threads_semaphore = Arc::new(Semaphore::new(4));
                let compilers =
                    Compilers::new(Arc::new(fetcher), VyperCompiler::new(), threads_semaphore);
                Arc::new(compilers)
            })
            .await
    }

    fn default_request() -> VerificationRequest {
        let deployed_bytecode: &str = "0x6003361161000c57610053565b60003560e01c34610059576360fe47b18118610032576024361861005957600435600055005b63b108b1db811861005157600436186100595760005460405260206040f35b505b60006000fd5b600080fda165767970657283000306000b";
        let creation_bytecode: &str = "0x3461008557600060005561006b61001960003961006b6000f36003361161000c57610053565b60003560e01c34610059576360fe47b18118610032576024361861005957600435600055005b63b108b1db811861005157600436186100595760005460405260206040f35b505b60006000fd5b600080fda165767970657283000306000b005b600080fd";
        let compiler_version: &str = "v0.3.6+commit.4a2124d0";
        let sources = BTreeMap::from([(
            "A.sol".to_string(),
            "stored_data: public(uint256)\n\n@external\ndef __init__():\n    self.stored_data = 0\n\n@external\ndef set(new_value: uint256):\n    self.stored_data = new_value".to_string(),
        )]);

        VerificationRequest::new(
            deployed_bytecode,
            creation_bytecode,
            compiler_version,
            sources,
            None,
        )
        .expect("Invalid verification request")
    }

    #[rstest::rstest]
    #[tokio::test]
    async fn multi_part(middleware: impl smart_contract_verifier::Middleware<VyperSuccess>) {
        let compilers = global_compilers().await;
        let client = VyperClient::new_arc(compilers.clone()).with_middleware(middleware);

        let request = default_request();
        vyper::multi_part::verify(
            Arc::new(client),
            vyper::multi_part::VerificationRequest::from(request),
        )
        .await
        .expect("Verification failed");
    }
}

// mod sourcify {
//     use super::*;
//     use smart_contract_verifier::{
//         sourcify, sourcify::api::VerificationRequest, SourcifyApiClient, SourcifySuccess,
//         DEFAULT_SOURCIFY_HOST,
//     };
//     use std::{collections::BTreeMap, sync::Arc};
//     use url::Url;
//
//     fn default_request() -> VerificationRequest {
//         VerificationRequest {
//             address: "0x8a8FA3Da120534a4945666520112a2A0D0A3aC55".to_string(),
//             chain: "5".to_string(),
//             files: BTreeMap::from([
//                 ("metadata.json".to_string(), r#"{"compiler":{"version":"0.8.7+commit.e28d00a7"},"language":"Solidity","output":{"abi":[],"devdoc":{"kind":"dev","methods":{},"version":1},"userdoc":{"kind":"user","methods":{},"version":1}},"settings":{"compilationTarget":{"contracts/Basic.sol":"Main"},"evmVersion":"london","libraries":{},"metadata":{"bytecodeHash":"ipfs"},"optimizer":{"enabled":false,"runs":200},"remappings":[]},"sources":{"contracts/Basic.sol":{"keccak256":"0x16b344408fcd46f38344e2192b7b82677bb29a8c5e822145e55d67d156fd02b5","urls":["bzz-raw://99f0cffe21a52b8f1d0a6d687f974173d6ed3ccfe4fa25af22bcb8b343372363","dweb:/ipfs/Qmc5BsPNZkDeooYWLxwq6HCNKmH6GzXLmQSJ3LmrnFrNzw"]}},"version":1}"#.to_string()),
//                 ("contracts/Basic.sol".to_string(), "pragma solidity >0.4.5; contract Main {uint256 a; }".to_string())
//             ]),
//             chosen_contract: None
//         }
//     }
//
//     #[rstest::rstest]
//     #[tokio::test]
//     async fn verify(middleware: impl smart_contract_verifier::Middleware<SourcifySuccess>) {
//         let host = Url::try_from(DEFAULT_SOURCIFY_HOST).expect("Invalid Sourcify host Url");
//         let client = SourcifyApiClient::new(host, 10, 3.try_into().unwrap())
//             .expect("Sourcify client build failed")
//             .with_middleware(middleware);
//
//         let request = default_request();
//
//         sourcify::api::verify(Arc::new(client), request)
//             .await
//             .expect("Verification failed");
//     }
// }
