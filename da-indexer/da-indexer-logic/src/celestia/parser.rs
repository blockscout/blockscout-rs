use anyhow::{Error, Result};
use celestia_types::{
    blob::Blob, consts::appconsts, nmt::Namespace, Commitment, DataAvailabilityHeader,
    ExtendedDataSquare, Share,
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
    dah.row_roots.iter().any(|row| {
        *PAY_FOR_BLOB_NAMESPACE >= row.min_namespace().into()
            && *PAY_FOR_BLOB_NAMESPACE <= row.max_namespace().into()
    })
}

/// Extracts blobs from the ExtendedDataSquare.
/// The format described here: https://github.com/celestiaorg/celestia-app/blob/main/specs/src/specs/shares.md
pub fn parse_eds(eds: &ExtendedDataSquare, width: usize) -> Result<Vec<Blob>> {
    // sanity check
    if width * width != eds.data_square.len() {
        return Err(Error::msg("data square length mismatch"));
    }

    let mut blobs: Vec<Blob> = vec![];
    let mut sequence_length = 0;
    let mut parsed_length = 0;

    for row in eds.data_square.chunks(width).take(width / 2) {
        for share in row.iter().take(width / 2) {
            let share = Share::from_raw(share)?;
            let ns = share.namespace();

            if ns == *TAIL_PADDING_NAMESPACE {
                break;
            }

            if ns.is_reserved_on_celestia() {
                continue;
            }

            let info_byte = share.info_byte();

            let mut share_data;
            if info_byte.is_sequence_start() {
                assert!(parsed_length == sequence_length);

                sequence_length = share.sequence_length().unwrap() as usize;
                parsed_length = 0;

                if sequence_length == 0
                    && blobs.last().is_some()
                    && blobs.last().unwrap().namespace == ns
                {
                    // Namespace Padding Share, should be ignored
                    continue;
                }

                blobs.push(Blob {
                    namespace: ns,
                    data: vec![0; sequence_length],
                    share_version: info_byte.version(),
                    commitment: Commitment([0; 32]),
                });

                // first share: skip info byte and sequence length
                share_data = &share.data()[1 + appconsts::SEQUENCE_LEN_BYTES..];
            } else {
                // continuation share: skip info byte
                share_data = &share.data()[1..];
            }

            let data_length = share_data.len().min(sequence_length - parsed_length);
            share_data = &share_data[..data_length];

            let last_blob = blobs.last_mut().unwrap();
            last_blob.data[parsed_length..(parsed_length + data_length)]
                .copy_from_slice(share_data);
            parsed_length += data_length;

            if parsed_length == sequence_length {
                last_blob.commitment =
                    Commitment::from_blob(ns, info_byte.version(), &last_blob.data)?;
            }
        }
    }
    Ok(blobs)
}
