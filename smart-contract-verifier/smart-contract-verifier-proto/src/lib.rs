#![allow(clippy::derive_partial_eq_without_eq)]
pub mod blockscout {
    pub mod smart_contract_verifier {
        pub mod v2 {
            include!(concat!(
                env!("OUT_DIR"),
                "/blockscout.smart_contract_verifier.v2.rs"
            ));
        }
    }
}
