#![allow(clippy::derive_partial_eq_without_eq)]

#[cfg(feature = "http-client")]
pub mod http_client;

pub mod blockscout {
    pub mod smart_contract_verifier {
        pub mod v2 {
            include!(concat!(
                env!("OUT_DIR"),
                "/blockscout.smart_contract_verifier.v2.rs"
            ));
        }
    }
}
