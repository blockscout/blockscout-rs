// SPDX-License-Identifier: LicenseRef-Blockscout

#![allow(clippy::derive_partial_eq_without_eq)]
// `tonic::Status` is ~176 bytes, which trips the lint in generated and http-client code.
#![allow(clippy::result_large_err)]
pub mod blockscout {
    pub mod sig_provider {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/blockscout.sig_provider.v1.rs"));
        }
    }
}
