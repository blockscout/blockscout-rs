// SPDX-License-Identifier: LicenseRef-Blockscout

// `TokenType` is nested in `TokenInfo` (module `token_info`) so its value names
// do not squat the proto package namespace. Nesting must not change the emitted
// JSON — that is exactly what the assertions below pin.
use interchain_indexer_proto::blockscout::interchain_indexer::v1::{
    StatsBridgedTokenItem, TokenInfo, token_info::TokenType,
};

#[test]
fn token_info_serializes_type_without_prefix() {
    for (token_type, expected) in [
        (TokenType::Unspecified, "UNSPECIFIED"),
        (TokenType::Erc20, "ERC20"),
        (TokenType::Native, "NATIVE"),
        (TokenType::Erc721, "ERC721"),
        (TokenType::Erc1155, "ERC1155"),
    ] {
        let token = TokenInfo {
            r#type: token_type as i32,
            ..Default::default()
        };
        let json = serde_json::to_value(&token).unwrap();
        assert_eq!(json["type"], expected);
        assert_eq!(
            serde_json::from_value::<TokenInfo>(json).unwrap().r#type,
            token_type as i32
        );
    }
}

#[test]
fn native_token_info_serializes_null_address() {
    let token = TokenInfo {
        r#type: TokenType::Native as i32,
        address_hash: None,
        name: Some("xDai".to_string()),
        symbol: Some("xDAI".to_string()),
        decimals: Some("18".to_string()),
        ..Default::default()
    };
    let json = serde_json::to_value(&token).unwrap();
    assert_eq!(json["type"], "NATIVE");
    assert_eq!(json.get("address_hash"), Some(&serde_json::Value::Null));
    assert_eq!(json["decimals"], "18");
}

#[test]
fn stats_token_serializes_type_and_nullable_address() {
    for (token_type, address, expected) in [
        (TokenType::Native, None, "NATIVE"),
        (
            TokenType::Erc20,
            Some("0x1111111111111111111111111111111111111111".to_string()),
            "ERC20",
        ),
    ] {
        let token = StatsBridgedTokenItem {
            chain_id: "100".to_string(),
            token_address: address.clone(),
            r#type: token_type as i32,
            ..Default::default()
        };
        let json = serde_json::to_value(&token).unwrap();
        assert_eq!(json["type"], expected);
        assert_eq!(json.get("token_address"), Some(&serde_json::json!(address)));
    }
}
