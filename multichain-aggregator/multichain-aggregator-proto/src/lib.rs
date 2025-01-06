#![allow(clippy::derive_partial_eq_without_eq)]
pub mod blockscout {
    pub mod multichain_aggregator {
        pub mod v1 {
            include!(concat!(
                env!("OUT_DIR"),
                "/blockscout.multichain_aggregator.v1.rs"
            ));
        }
    }
}
