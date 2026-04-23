use pretty_assertions::assert_eq;
use visualizer_server::Settings;

#[test]
fn test_example_settings() {
    std::env::set_var("VISUALIZER__CONFIG", "config/base.toml");
    let example_settings = Settings::new().expect("Failed to parse config");
    let default_settings = Settings::default();
    assert_eq!(default_settings, example_settings);
}
