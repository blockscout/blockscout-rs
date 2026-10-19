// SPDX-License-Identifier: LicenseRef-Blockscout

#![allow(clippy::derive_partial_eq_without_eq)]
// `tonic::Status` is ~176 bytes, which trips the lint in generated and http-client code.
#![allow(clippy::result_large_err)]

pub use tonic;

#[cfg(feature = "http-client")]
pub mod http_client;

pub mod blockscout {
    pub mod eth_bytecode_db {
        pub mod v2 {
            include!(concat!(
                env!("OUT_DIR"),
                "/blockscout.eth_bytecode_db.v2.rs"
            ));
        }
    }
}
