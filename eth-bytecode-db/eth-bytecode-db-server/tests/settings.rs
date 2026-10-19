// SPDX-License-Identifier: LicenseRef-Blockscout

use blockscout_service_launcher::{database::DatabaseConnectSettings, launcher::ConfigSettings};
use eth_bytecode_db_server::Settings;

/// The database pool is only tunable if the env keys actually reach
/// `connect_options`, and `deny_unknown_fields` turns a wrong key into a
/// start-up failure, so pin the mapping down here.
#[test]
fn database_connect_options_are_configurable_from_env() {
    std::env::set_var(
        "ETH_BYTECODE_DB__DATABASE__CONNECT__URL",
        "postgres://user:pass@localhost:5432/eth_bytecode_db",
    );
    std::env::set_var(
        "ETH_BYTECODE_DB__VERIFIER__HTTP_URL",
        "http://localhost:8050/",
    );
    std::env::set_var(
        "ETH_BYTECODE_DB__DATABASE__CONNECT_OPTIONS__MAX_CONNECTIONS",
        "42",
    );
    std::env::set_var(
        "ETH_BYTECODE_DB__DATABASE__CONNECT_OPTIONS__ACQUIRE_TIMEOUT",
        "7",
    );

    let settings = Settings::build().expect("failed to parse config");

    assert_eq!(
        DatabaseConnectSettings::Url(
            "postgres://user:pass@localhost:5432/eth_bytecode_db".to_string()
        ),
        settings.database.connect,
    );
    assert_eq!(Some(42), settings.database.connect_options.max_connections);
    assert_eq!(
        Some(std::time::Duration::from_secs(7)),
        settings.database.connect_options.acquire_timeout,
    );
}
