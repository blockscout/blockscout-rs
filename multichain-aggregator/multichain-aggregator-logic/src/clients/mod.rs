pub mod dapp;
pub mod token_info {
    pub use contracts_info_proto::{
        blockscout::contracts_info::v1 as proto,
        http_client::{contracts_info as endpoints, Client, Config},
    };
}
