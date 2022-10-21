use pretty_assertions::assert_eq;
use smart_contract_verifier_http::Settings;

fn rewrite_system_specific_example_settings(
    example_settings: &mut Settings,
    default_settings: &Settings,
) {
    example_settings.compilers.max_threads = default_settings.compilers.max_threads;
    #[cfg(not(target_os = "linux"))]
    {
        use std::mem::discriminant;

        example_settings.solidity.compilers_dir = default_settings.solidity.compilers_dir.clone();
        if discriminant(&example_settings.solidity.fetcher)
            == discriminant(&default_settings.solidity.fetcher)
        {
            example_settings.solidity.fetcher = default_settings.solidity.fetcher.clone();
        }
        example_settings.vyper.compilers_dir = default_settings.vyper.compilers_dir.clone();
        if discriminant(&example_settings.vyper.fetcher)
            == discriminant(&default_settings.vyper.fetcher)
        {
            example_settings.vyper.fetcher = default_settings.vyper.fetcher.clone();
        }
    }
}

#[test]
fn test_example_settings() {
    std::env::set_var("SMART_CONTRACT_VERIFIER__CONFIG", "config/base.toml");
    let (example_settings, default_settings) = {
        let mut example_settings = Settings::new().expect("Failed to parse config");
        let default_settings = Settings::default();

        rewrite_system_specific_example_settings(&mut example_settings, &default_settings);

        (example_settings, default_settings)
    };
    assert_eq!(default_settings, example_settings);
}
