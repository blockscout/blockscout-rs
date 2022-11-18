#![allow(clippy::derive_partial_eq_without_eq)]
pub mod blockscout {
    pub mod eth_bytecode_db {
        pub mod v1 {
            include!(concat!(
                env!("OUT_DIR"),
                "/blockscout.eth_bytecode_db.v1.rs"
            ));
        }
    }
}
