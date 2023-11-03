#![allow(clippy::derive_partial_eq_without_eq)]
pub mod blockscout {
    pub mod {{crate_name}} {
        pub mod v1 {
            include!(concat!(
                env!("OUT_DIR"),
                "/blockscout.{{crate_name}}.v1.rs"
            ));
        }
    }
}
