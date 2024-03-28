#![allow(clippy::derive_partial_eq_without_eq)]
pub mod blockscout {
    pub mod basic_cache {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/blockscout.basic_cache.v1.rs"));
        }
    }
}
