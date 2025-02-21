#![allow(clippy::derive_partial_eq_without_eq)]
pub mod blockscout {
    pub mod tac_operation_lifecycle {
        pub mod v1 {
            include!(concat!(
                env!("OUT_DIR"),
                "/blockscout.tac_operation_lifecycle.v1.rs"
            ));
        }
    }
}
