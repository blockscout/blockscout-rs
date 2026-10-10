// SPDX-License-Identifier: LicenseRef-Blockscout

//! Boundary validation shared by the admin methods. Pure functions: they never
//! touch the database.

use super::error::AdminError;
use url::Url;

pub(crate) const MAX_REASON_CHARS: usize = 1000;
pub(crate) const MAX_ICON_URL_BYTES: usize = 2048;
/// Length of an EVM address in bytes. The native token is stored under this many
/// zero bytes.
const EVM_ADDRESS_BYTES: usize = 20;

fn ensure_arg(condition: bool, message: impl FnOnce() -> String) -> Result<(), AdminError> {
    match condition {
        true => Ok(()),
        false => Err(AdminError::InvalidArgument(message())),
    }
}

/// Trims `reason` and checks its length in characters.
pub(crate) fn validate_reason(reason: &str) -> Result<String, AdminError> {
    let reason = reason.trim();
    let chars = reason.chars().count();
    ensure_arg(chars >= 1, || "reason must not be empty".to_string())?;
    ensure_arg(chars <= MAX_REASON_CHARS, || {
        format!("reason must be at most {MAX_REASON_CHARS} characters")
    })?;
    Ok(reason.to_owned())
}

/// Accepts an absolute `https` URL with a host and no credentials, and returns
/// its normalized form. The URL is only stored and returned to clients, never
/// fetched by the service.
pub(crate) fn validate_icon_url(raw: &str) -> Result<String, AdminError> {
    // Checked on the raw string: the WHATWG parser silently strips surrounding
    // whitespace and embedded tabs and newlines.
    ensure_arg(!raw.is_empty(), || "icon_url must not be empty".to_string())?;
    ensure_arg(raw == raw.trim(), || {
        "icon_url must not have leading or trailing whitespace".to_string()
    })?;
    ensure_arg(!raw.chars().any(char::is_control), || {
        "icon_url must not contain control characters".to_string()
    })?;

    let url = Url::parse(raw)
        .map_err(|_| AdminError::InvalidArgument("icon_url is not a valid URL".to_string()))?;
    ensure_arg(url.scheme() == "https", || {
        "icon_url must use the https scheme".to_string()
    })?;
    ensure_arg(url.host().is_some(), || {
        "icon_url must have a host".to_string()
    })?;
    ensure_arg(
        url.username().is_empty() && url.password().is_none(),
        || "icon_url must not contain credentials".to_string(),
    )?;
    ensure_arg(url.as_str().len() <= MAX_ICON_URL_BYTES, || {
        format!("icon_url must be at most {MAX_ICON_URL_BYTES} bytes")
    })?;
    Ok(url.into())
}

/// Resolves the `icon_url` / `clear` pair of an icon request into the value to
/// store: `Some(url)` to set, `None` to clear. Exactly one of the two must be
/// given; `clear = false` is not a way to say "nothing".
pub(crate) fn resolve_icon_change(
    icon_url: Option<&str>,
    clear: Option<bool>,
) -> Result<Option<String>, AdminError> {
    match (icon_url, clear) {
        (Some(_), Some(_)) => Err(AdminError::InvalidArgument(
            "set exactly one of icon_url and clear, not both".to_string(),
        )),
        (Some(url), None) => validate_icon_url(url).map(Some),
        (None, Some(true)) => Ok(None),
        (None, Some(false)) => Err(AdminError::InvalidArgument(
            "clear must be true when set; omit it and give icon_url to set an icon".to_string(),
        )),
        (None, None) => Err(AdminError::InvalidArgument(
            "set exactly one of icon_url and clear".to_string(),
        )),
    }
}

/// Resolves the `address` / `native` pair of a token request into the storage
/// address: 20 bytes, with the native token stored as 20 zero bytes. Exactly one
/// of the two must be given; the zero address is rejected so that one token has
/// one spelling.
pub(crate) fn parse_token_selector(
    address: Option<&str>,
    native: Option<bool>,
) -> Result<Vec<u8>, AdminError> {
    match (address, native) {
        (Some(_), Some(_)) => Err(AdminError::InvalidArgument(
            "set exactly one of address and native, not both".to_string(),
        )),
        (None, None) => Err(AdminError::InvalidArgument(
            "set exactly one of address and native".to_string(),
        )),
        (None, Some(false)) => Err(AdminError::InvalidArgument(
            "native must be true when set; omit it and give address to select a token".to_string(),
        )),
        (None, Some(true)) => Ok(vec![0u8; EVM_ADDRESS_BYTES]),
        (Some(address), None) => {
            let hex_digits = address
                .strip_prefix("0x")
                .or_else(|| address.strip_prefix("0X"))
                .filter(|digits| digits.len() == EVM_ADDRESS_BYTES * 2)
                .ok_or_else(|| {
                    AdminError::InvalidArgument(
                        "address must be a 0x-prefixed 20-byte hex string".to_string(),
                    )
                })?;
            let bytes = hex::decode(hex_digits)
                .map_err(|_| AdminError::InvalidArgument("address is not valid hex".to_string()))?;
            ensure_arg(bytes.iter().any(|byte| *byte != 0), || {
                "use native=true for the native token".to_string()
            })?;
            Ok(bytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_invalid_argument<T: std::fmt::Debug>(result: Result<T, AdminError>) -> bool {
        matches!(result, Err(AdminError::InvalidArgument(_)))
    }

    #[test]
    fn admin_validation_icon_url_accepts_https_and_normalizes() {
        assert_eq!(
            validate_icon_url("https://EXAMPLE.com/Icons/a b.png?x=1#f").unwrap(),
            "https://example.com/Icons/a%20b.png?x=1#f"
        );
        assert_eq!(
            validate_icon_url("https://example.com").unwrap(),
            "https://example.com/"
        );
        assert_eq!(
            validate_icon_url("https://example.com:8443/i.svg").unwrap(),
            "https://example.com:8443/i.svg"
        );
    }

    #[test]
    fn admin_validation_icon_url_rejects_non_https_schemes() {
        for url in [
            "http://example.com/i.png",
            "javascript:alert(1)",
            "data:image/png;base64,AAAA",
            "ftp://example.com/i.png",
            "//example.com/i.png",
            "example.com/i.png",
        ] {
            assert!(is_invalid_argument(validate_icon_url(url)), "{url}");
        }
    }

    #[test]
    fn admin_validation_icon_url_rejects_userinfo_missing_host_and_overlong() {
        for url in [
            "https://user@example.com/i.png",
            "https://user:pass@example.com/i.png",
            "https://:pass@example.com/i.png",
            "https://",
            "https:///",
            "https://:8080/i.png",
        ] {
            assert!(is_invalid_argument(validate_icon_url(url)), "{url}");
        }

        let prefix = "https://example.com/";
        let at_limit = format!("{prefix}{}", "a".repeat(MAX_ICON_URL_BYTES - prefix.len()));
        assert_eq!(validate_icon_url(&at_limit).unwrap(), at_limit);
        let over_limit = format!("{at_limit}a");
        assert!(is_invalid_argument(validate_icon_url(&over_limit)));
        // The limit applies to the normalized form, which percent-encoding grows.
        let grows = format!("{prefix}{}", " ".repeat(MAX_ICON_URL_BYTES / 2));
        assert!(is_invalid_argument(validate_icon_url(&grows)));
    }

    #[test]
    fn admin_validation_icon_url_rejects_whitespace_and_control_chars() {
        for url in [
            "",
            " ",
            " https://example.com/i.png",
            "https://example.com/i.png ",
            "https://example.com/i.png\n",
            "\thttps://example.com/i.png",
            "https://exa\tmple.com/i.png",
            "https://example.com/i\n.png",
            "https://example.com/i\u{0}.png",
            "https://example.com/i\u{7f}.png",
            "\u{a0}https://example.com/i.png",
        ] {
            assert!(is_invalid_argument(validate_icon_url(url)), "{url:?}");
        }
    }

    #[test]
    fn admin_validation_reason_bounds() {
        assert!(is_invalid_argument(validate_reason("")));
        assert!(is_invalid_argument(validate_reason("   \n\t ")));
        assert_eq!(validate_reason("  TICKET-1 \n").unwrap(), "TICKET-1");

        let at_limit = "я".repeat(MAX_REASON_CHARS);
        assert!(
            at_limit.len() > MAX_REASON_CHARS,
            "counted in chars, not bytes"
        );
        assert_eq!(validate_reason(&at_limit).unwrap(), at_limit);
        assert_eq!(
            validate_reason(&format!(" {at_limit} ")).unwrap(),
            at_limit,
            "the limit applies after trimming"
        );
        assert!(is_invalid_argument(validate_reason(&format!(
            "{at_limit}я"
        ))));
    }

    #[test]
    fn admin_validation_icon_change_requires_exactly_one_of_url_or_clear() {
        let url = "https://example.com/i.png";
        assert_eq!(
            resolve_icon_change(Some(url), None).unwrap(),
            Some(url.to_string())
        );
        assert_eq!(resolve_icon_change(None, Some(true)).unwrap(), None);
        assert!(is_invalid_argument(resolve_icon_change(
            Some(url),
            Some(true)
        )));
        assert!(is_invalid_argument(resolve_icon_change(
            Some(url),
            Some(false)
        )));
        assert!(is_invalid_argument(resolve_icon_change(None, None)));
        assert!(is_invalid_argument(resolve_icon_change(None, Some(false))));
        assert!(
            is_invalid_argument(resolve_icon_change(Some("http://example.com/i.png"), None)),
            "the URL is validated when it is the chosen field"
        );
    }

    #[test]
    fn admin_validation_token_selector_rules() {
        let address = "0x6b175474e89094c44da98b954eedeac495271d0f";
        let expected = hex::decode(&address[2..]).unwrap();

        // An address, in either case and with either prefix spelling.
        assert_eq!(parse_token_selector(Some(address), None).unwrap(), expected);
        assert_eq!(
            parse_token_selector(Some(&address.to_uppercase().replacen("0X", "0x", 1)), None)
                .unwrap(),
            expected
        );
        assert_eq!(
            parse_token_selector(Some(&format!("0X{}", &address[2..])), None).unwrap(),
            expected
        );

        // The native token is 20 zero bytes.
        assert_eq!(
            parse_token_selector(None, Some(true)).unwrap(),
            vec![0u8; 20]
        );

        // The zero address has one spelling: `native = true`.
        let zero = format!("0x{}", "00".repeat(20));
        match parse_token_selector(Some(&zero), None) {
            Err(AdminError::InvalidArgument(message)) => {
                assert_eq!(message, "use native=true for the native token")
            }
            other => panic!("the zero address must be rejected, got {other:?}"),
        }

        // Exactly one selector.
        assert!(is_invalid_argument(parse_token_selector(
            Some(address),
            Some(true)
        )));
        assert!(
            is_invalid_argument(parse_token_selector(Some(address), Some(false))),
            "a second selector is an error whatever its value"
        );
        assert!(is_invalid_argument(parse_token_selector(None, None)));
        assert!(is_invalid_argument(parse_token_selector(None, Some(false))));

        // Malformed addresses.
        let bad_addresses = [
            "",
            "0x",
            &address[2..],
            &format!("1x{}", &address[2..]),
            &address[..address.len() - 2],
            &format!("{address}00"),
            &format!("0x{}", "zz".repeat(20)),
            &format!("0x {}", &address[3..]),
            &format!(" {address}"),
            &format!("{address} "),
            &format!("0x{}", "ы".repeat(20)),
        ];
        for bad in bad_addresses {
            assert!(
                is_invalid_argument(parse_token_selector(Some(bad), None)),
                "{bad:?}"
            );
        }
    }
}
