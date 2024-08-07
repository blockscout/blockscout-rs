#![allow(clippy::derive_partial_eq_without_eq, unused_imports)]
pub mod blockscout {
    pub mod stats {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/blockscout.stats.v1.rs"));

            fn default_resolution_i32() -> i32 {
                Resolution::Unspecified as i32
            }
        }
    }
}

#[cfg(test)]
mod tests;
