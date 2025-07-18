#![allow(clippy::derive_partial_eq_without_eq)]
pub mod blockscout {
    pub mod zetachain_cctx {
        pub mod v1 {
            include!(concat!(
                env!("OUT_DIR"),
                "/blockscout.zetachain_cctx.v1.rs"
            ));
        }
    }
}
