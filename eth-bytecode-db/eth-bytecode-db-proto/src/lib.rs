#![allow(clippy::derive_partial_eq_without_eq)]
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
