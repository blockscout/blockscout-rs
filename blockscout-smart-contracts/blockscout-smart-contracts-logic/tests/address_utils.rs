use blockscout_smart_contracts_logic::address_utils::{format_address_hex_from_db, parse_address_to_bytes};

#[test]
fn parse_address_accepts_0x_prefixed() {
    let hex = "0x0000000000000000000000000000000000000001";
    let bytes = parse_address_to_bytes(hex).expect("should parse");
    assert_eq!(bytes.len(), 20);
    assert!(bytes[..19].iter().all(|b| *b == 0));
    assert_eq!(bytes[19], 1);
}

#[test]
fn parse_address_accepts_plain_hex() {
    let hex = "0000000000000000000000000000000000000001";
    let bytes = parse_address_to_bytes(hex).expect("should parse");
    assert_eq!(bytes.len(), 20);
    assert!(bytes[..19].iter().all(|b| *b == 0));
    assert_eq!(bytes[19], 1);
}

#[test]
fn parse_address_rejects_non_hex() {
    let err = parse_address_to_bytes("0xZZZZ").unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

#[test]
fn parse_address_rejects_wrong_length() {
    let err = parse_address_to_bytes("0x1").unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

#[test]
fn formats_20_byte_address_as_0x_hex() {
    let mut bytes = vec![0u8; 20];
    bytes[19] = 1; // ...0001
    let s = format_address_hex_from_db(&bytes, "fallback");
    assert_eq!(s, "0x0000000000000000000000000000000000000001");
}

#[test]
fn returns_fallback_when_length_is_not_20() {
    let bytes = vec![0u8; 19];
    let s = format_address_hex_from_db(&bytes, "original_input");
    assert_eq!(s, "original_input");
}
