#![allow(clippy::derive_partial_eq_without_eq)]
pub mod blockscout {
    pub mod proxy_verifier {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/blockscout.proxy_verifier.v1.rs"));
        }
    }
}
