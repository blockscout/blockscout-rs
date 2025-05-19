use base64::prelude::*;
use chrono::NaiveDateTime;

pub fn is_generic_hash(q: &str) -> bool {
    q.starts_with("0x") && q.len() == 66 && q[2..].chars().all(|c| c.is_ascii_hexdigit())
}

pub fn is_tac_address(q: &str) -> bool {
    q.starts_with("0x") && q.len() == 42 && q[2..].chars().all(|c| c.is_ascii_hexdigit())
}

pub fn is_ton_address(q: &str) -> bool {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(q)
        .ok()
        .map(|bytes| bytes.len() == 36)
        .unwrap_or(false)
}
