#![allow(clippy::derive_partial_eq_without_eq)]
pub mod blockscout {
    pub mod ethereum_bytecode_database {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/blockscout.ethereum_bytecode_database.v1.rs"));
        }
    }
}