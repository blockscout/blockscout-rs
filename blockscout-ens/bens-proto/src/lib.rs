#![allow(clippy::derive_partial_eq_without_eq)]
pub mod blockscout {
    pub mod bens {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/blockscout.bens.v1.rs"));
        }
    }
}
