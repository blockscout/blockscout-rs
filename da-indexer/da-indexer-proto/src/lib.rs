#![allow(clippy::derive_partial_eq_without_eq)]
pub mod blockscout {
    pub mod da_indexer {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/blockscout.da_indexer.v1.rs"));
        }
    }
}
