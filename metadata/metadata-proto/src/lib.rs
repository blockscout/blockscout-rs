#![allow(clippy::derive_partial_eq_without_eq)]
pub mod blockscout {
    pub mod metadata {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/blockscout.metadata.v1.rs"));
        }
    }
}
