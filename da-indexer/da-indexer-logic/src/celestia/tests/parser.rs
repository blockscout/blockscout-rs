use base64::prelude::*;
use celestia_types::{Blob, ExtendedDataSquare};
use serde::Deserialize;

use crate::celestia::parser::parse_eds;

#[tokio::test]
pub async fn parse_eds_with_blobs() {
    let eds = include_bytes!("data/eds_with_blobs.json");
    let mut deserializer = serde_json::Deserializer::from_slice(eds);
    let eds = ExtendedDataSquare::deserialize(&mut deserializer).unwrap();
    let blobs = parse_eds(&eds, eds.square_len()).unwrap();
    assert!(blobs.len() == 2);
    check_blob(
        &blobs[0],
        "0000000000000000000000000000000000000000000b912881caa28249", 
        "ALn3qnfThjUNBlTjNofbal8AAAAAAE942tqxhPHKOr/57y8zzfM8uYH3oEjTDLvZqYkGtb/3zT2V5zdr7wMOy5nrl8kKfpwhv6dR480TyTQBBxYBBgaGOgZHHgZGBroAAAAAAP//AQ==", 
        "2tIUeVtju66yqNLxBGdTL+E6E/KnjvrP0Rt83xFhmd0=",
    );

    check_blob(
        &blobs[1],
        "0000000000000000000000000000000000000000000e105c56e34def97", 
        "AMAA93SstWmfvYOyZM44EJsAAAAAAJF42trJocx4/cU6xm9zPrG+PGw/2571zl8me7WLirJLvzDyLnzQY1n4pZshLy0+NuqzytzfrSW18rZ9fIw/5BtxIv4Df5CR/IE/7KgiKOjgH/4DNVASSQv/gT/sB6Fc9gd27A/q+MEk+4N/zA/+gRkIBJMiDTF/sIehekYEG4IYRsEoGAWjYBSMghEPAAAAAP//AQ==", 
        "iY0byb3PZYoo2jc6zFz49/WkY/8MXV7rOl3Ob36HE+A=",
    );
}

#[tokio::test]
pub async fn parse_eds_without_blobs() {
    let eds = include_bytes!("data/eds_without_blobs.json");
    let mut deserializer = serde_json::Deserializer::from_slice(eds);
    let eds = ExtendedDataSquare::deserialize(&mut deserializer).unwrap();
    let blobs = parse_eds(&eds, eds.square_len()).unwrap();
    assert!(blobs.is_empty());
}

fn check_blob(blob: &Blob, namespace: &str, data: &str, commitment: &str) {
    assert_eq!(blob.namespace.as_bytes(), hex::decode(namespace).unwrap());
    assert_eq!(blob.data, BASE64_STANDARD.decode(data).unwrap());
    assert_eq!(
        blob.commitment.0[..],
        BASE64_STANDARD.decode(commitment).unwrap()
    );
}
