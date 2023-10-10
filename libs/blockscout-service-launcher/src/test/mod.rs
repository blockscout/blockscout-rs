mod server;

#[cfg(feature = "database")]
mod database;

pub use server::{get_test_server_settings, init_server, send_request};

#[cfg(feature = "database")]
pub use database::TestDbGuard;
