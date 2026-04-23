#![allow(clippy::derive_partial_eq_without_eq)]
pub mod blockscout {
    pub mod stylus_verifier {
        pub mod v1 {
            include!(concat!(
                env!("OUT_DIR"),
                "/blockscout.stylus_verifier.v1.rs"
            ));
        }
    }
}

pub mod grpc {
    pub mod health {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/grpc.health.v1.rs"));
        }
    }
}
