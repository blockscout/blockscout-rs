use crate::routes::eth_bytecode_db::SoliditySourcesVerifyStandardJson;

mod verifier_alliance {
    use super::*;
    use crate::test_cases;
    use std::path::PathBuf;

    #[rstest::rstest]
    #[tokio::test]
    async fn success(#[files("tests/alliance_test_cases/*.json")] test_case_path: PathBuf) {
        test_cases::verifier_alliance::success::<SoliditySourcesVerifyStandardJson>(test_case_path)
            .await;
    }
}
