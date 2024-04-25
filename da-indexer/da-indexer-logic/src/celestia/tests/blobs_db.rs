use celestia_types::{nmt::Namespace, Blob as CelestiaBlob, Commitment};

use crate::celestia::{
    repository::{blobs, blocks},
    tests::init_db,
};
use sha3::{Digest, Sha3_256};

#[tokio::test]
async fn smoke_test() {
    let db = init_db("blobs_db_smoke").await;
    let height_range = 1..=5;
    let blobs_range = 1..=5;

    for height in height_range.clone() {
        let blobs = blobs_range.clone().map(celestia_blob).collect::<Vec<_>>();
        blocks::upsert(db.client().as_ref(), height, &[], 5, 0)
            .await
            .unwrap();
        blobs::upsert_many(db.client().as_ref(), height, blobs)
            .await
            .unwrap();
    }

    for height in height_range {
        let blobs = blobs_range.clone().map(celestia_blob).collect::<Vec<_>>();
        for blob in blobs {
            let blob_db =
                blobs::find_by_height_and_commitment(&db.client(), height, &blob.commitment.0)
                    .await
                    .unwrap()
                    .unwrap();
            assert_eq!(blob.namespace.as_bytes(), blob_db.namespace);
            assert_eq!(blob.data, blob_db.data);
            assert_eq!(&blob.commitment.0[..], blob_db.commitment);
        }
    }

    assert!(
        blobs::find_by_height_and_commitment(&db.client(), 0, &[0_u8; 32])
            .await
            .unwrap()
            .is_none()
    );
}

fn celestia_blob(seed: u32) -> CelestiaBlob {
    let namespace =
        Namespace::new(0, &[&[0_u8; 18], &sha3("namespace", seed)[..10]].concat()).unwrap();
    let data = sha3("data", seed).to_vec();
    let share_version = 0;
    let commitment = Commitment::from_blob(namespace, share_version, &data).unwrap();
    CelestiaBlob {
        namespace,
        data,
        share_version,
        commitment,
    }
}

pub fn sha3(domain: &str, seed: u32) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(domain.as_bytes());
    hasher.update(seed.to_be_bytes());
    let result = hasher.finalize();
    result.into()
}
