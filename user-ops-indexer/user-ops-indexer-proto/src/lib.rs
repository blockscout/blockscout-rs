#![allow(clippy::derive_partial_eq_without_eq, unused_imports)]

pub mod blockscout {
    pub mod user_ops_indexer {
        pub mod v1 {
            include!(concat!(
                env!("OUT_DIR"),
                "/blockscout.user_ops_indexer.v1.rs"
            ));
        }
    }
}
