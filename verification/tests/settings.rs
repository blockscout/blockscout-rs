use pretty_assertions::assert_eq;
use verification::Settings;

// For linux systems we assume that all os specific values
// are filled with defaults, so no need to rewrite them.
#[cfg(target_os = "linux")]
fn rewrite_os_specific_example_settings(_example_settings: &mut Settings, _default_settings: &Settings) {}

// For other systems we just use the values from default settings.
#[cfg(not(target_os = "linux"))]
fn rewrite_os_specific_example_settings(example_settings: &mut Settings, default_settings: &Settings) {
    // For now, only server address is os system dependant
    example_settings.server.addr = default_settings.server.addr;
}

#[test]
fn test_example_settings() {
    std::env::set_var("VERIFICATION__CONFIG", "config/base.toml");
    let (example_settings, default_settings) = {
        let mut example_settings = Settings::new().expect("Failed to parse config");
        let default_settings = Settings::default();

        rewrite_os_specific_example_settings(&mut example_settings, &default_settings);

        (example_settings, default_settings)
    };
    assert_eq!(default_settings, example_settings);
}