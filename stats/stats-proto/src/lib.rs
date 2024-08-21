#![allow(clippy::derive_partial_eq_without_eq, unused_imports)]
pub mod blockscout {
    pub mod stats {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/blockscout.stats.v1.rs"));
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

#[cfg(test)]
mod tests;
