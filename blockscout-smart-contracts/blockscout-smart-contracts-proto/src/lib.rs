#![allow(clippy::derive_partial_eq_without_eq)]
pub mod blockscout {
    pub mod blockscout_smart_contracts {
        pub mod v1 {
            include!(concat!(
                env!("OUT_DIR"),
                "/blockscout.blockscout_smart_contracts.v1.rs"
            ));
        }
    }
}
