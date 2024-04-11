#![allow(clippy::derive_partial_eq_without_eq, unused_variables)]
pub mod blockscout {
    pub mod scoutcloud {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/blockscout.scoutcloud.v1.rs"));
        }
    }
}
