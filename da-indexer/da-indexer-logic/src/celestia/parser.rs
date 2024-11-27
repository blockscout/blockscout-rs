use anyhow::Result;
use celestia_types::{
    blob::Blob, nmt::Namespace, AppVersion, DataAvailabilityHeader, ExtendedDataSquare,
};

lazy_static! {
    static ref TAIL_PADDING_NAMESPACE: Namespace = Namespace::from_raw(
        &hex::decode("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE").unwrap()
    )
    .unwrap();
    static ref PAY_FOR_BLOB_NAMESPACE: Namespace = Namespace::from_raw(
        &hex::decode("0000000000000000000000000000000000000000000000000000000004").unwrap()
    )
    .unwrap();
}

/// Checks if the DataAvailabilityHeader might contain blobs.
pub fn maybe_contains_blobs(dah: &DataAvailabilityHeader) -> bool {
    dah.row_roots().iter().any(|row| {
        *PAY_FOR_BLOB_NAMESPACE >= row.min_namespace().into()
            && *PAY_FOR_BLOB_NAMESPACE <= row.max_namespace().into()
    })
}

/// Extracts blobs from the ExtendedDataSquare.
pub fn parse_eds(eds: &ExtendedDataSquare, app_version: u64) -> Result<Vec<Blob>> {
    let app_version = AppVersion::from_u64(app_version)
        .ok_or_else(|| anyhow::anyhow!("invalid or unsupported app_version: {app_version}"))?;

    Blob::reconstruct_all(eds.data_square(), app_version).map_err(|err| {
        tracing::error!("failed to parse EDS: {:?}", err);
        anyhow::anyhow!(err)
    })
}
