use crate::routes::eth_bytecode_db::AllianceSolidityMultiPartBatchImport;

mod verifier_alliance {
    use super::*;
    use crate::test_cases;
    use std::path::PathBuf;

    #[rstest::rstest]
    #[tokio::test]
    #[timeout(std::time::Duration::from_secs(60))]
    #[ignore = "Needs database to run"]
    async fn success(#[files("tests/alliance_test_cases/*.json")] test_case_path: PathBuf) {
        test_cases::verifier_alliance::success::<AllianceSolidityMultiPartBatchImport>(
            test_case_path,
        )
        .await;
    }
}
