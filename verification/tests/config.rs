use pretty_assertions::assert_eq;
use verification::Config;

#[test]
fn test_example_config() {
    let example_config =
        Config::from_file("example_config.toml".into()).expect("Failed to parse config");
    let default_config = Config::default();
    assert_eq!(default_config, example_config);
}
