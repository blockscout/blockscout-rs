// SPDX-License-Identifier: LicenseRef-Blockscout

/// A value that must never be rendered.
///
/// Deliberately implements neither `Display`, `Serialize`, `Deserialize`, nor a
/// derived `Debug`: those are the four ways a secret escapes. `expose` is the
/// single accessor, so every read is greppable.
pub struct Secret<T>(T);

impl<T> Secret<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }

    pub fn expose(&self) -> &T {
        &self.0
    }
}

impl<T> std::fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

impl<T: Clone> Clone for Secret<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

/// Reduce every URL in `input` to `scheme://host[:port]/<redacted>`, dropping
/// path, query, fragment and userinfo.
///
/// Errors from the RPC transport embed the full request URL (`reqwest::Error`
/// renders it in both `Display` and `Debug`), and that text reaches the failure
/// ledger in Postgres and the public status API. Since an API key can sit in the
/// path, the query, or userinfo, the only safe rule is to keep scheme+host and
/// drop everything after it.
pub fn redact_urls(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0usize;

    while let Some(rel_hit) = input[cursor..].find("://") {
        let hit = cursor + rel_hit;

        // Walk backwards from `hit` while the byte is a legal scheme
        // character, to find where the scheme starts.
        let mut scheme_start = hit;
        while scheme_start > cursor {
            let prev = scheme_start - 1;
            let b = bytes[prev];
            let is_scheme_byte = b.is_ascii_alphanumeric() || b == b'+' || b == b'-' || b == b'.';
            if !is_scheme_byte {
                break;
            }
            scheme_start = prev;
        }

        // `scheme_start == hit` means there were no scheme characters before
        // `://` (a bare `://host/...`, or one preceded by a non-ASCII byte).
        // Such a string is not a well-formed URL, but it is still shaped like
        // one, so it is redacted the same way with an empty scheme rather than
        // passed through: emitting it verbatim would leak the authority and
        // path, and this function's job is to fail closed.

        // Everything between `cursor` and `scheme_start` is plain text.
        output.push_str(&input[cursor..scheme_start]);

        // Authority end: the first byte that cannot appear in an authority —
        // `/`, `?`, `#` (start of path/query/fragment), or a byte that plainly
        // cannot appear in a URL at all. Every terminator checked here is
        // ASCII, so a multi-byte character is never split: non-ASCII bytes
        // simply fail every check and are treated as ordinary authority
        // bytes.
        let authority_start = hit + 3;
        let mut authority_end = authority_start;
        while authority_end < bytes.len() {
            let b = bytes[authority_end];
            let is_authority_terminator = matches!(
                b,
                b'/' | b'?' | b'#' | b'"' | b'\'' | b')' | b',' | b'}' | b'>' | b'`'
            ) || b.is_ascii_whitespace();
            if is_authority_terminator {
                break;
            }
            authority_end += 1;
        }

        // The whole URL can extend past the authority with a path, query, or
        // fragment — which legitimately contain `/`, `?`, `#`, so those are
        // no longer terminators here. Only bytes that plainly cannot appear
        // in a URL end it. This decides how much of the input is *consumed*
        // (and so dropped from the output): stopping at `authority_end`
        // instead would leave the path/query/fragment as unredacted "plain
        // text before the next match" on the following loop iteration.
        let mut url_end = authority_end;
        while url_end < bytes.len() {
            let b = bytes[url_end];
            let is_url_terminator = matches!(b, b'"' | b'\'' | b')' | b',' | b'}' | b'>' | b'`')
                || b.is_ascii_whitespace();
            if is_url_terminator {
                break;
            }
            url_end += 1;
        }

        let scheme = &input[scheme_start..hit];
        let authority = &input[authority_start..authority_end];
        // Drop `user:pass@`: keep only what follows the last `@`.
        let authority = match authority.rfind('@') {
            Some(at) => &authority[at + 1..],
            None => authority,
        };

        output.push_str(scheme);
        output.push_str("://");
        output.push_str(authority);
        // Anything beyond the bare authority — path, query, fragment, or
        // trailing garbage — was dropped; mark that explicitly instead of
        // silently emitting `scheme://host` for a URL that had more.
        if url_end > authority_end {
            output.push_str("/<redacted>");
        }

        cursor = url_end;
    }

    output.push_str(&input[cursor..]);
    output
}

/// Convenience wrapper for transport errors: renders with `{:?}` and redacts.
pub fn sanitize_transport_error<E: std::fmt::Debug>(err: &E) -> String {
    redact_urls(&format!("{err:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_urls_strips_a_path_embedded_secret() {
        let redacted = redact_urls("http://host/v1/SECRET123");

        assert!(redacted.contains("http://host"));
        assert!(!redacted.contains("SECRET123"));
    }

    #[test]
    fn redact_urls_strips_a_query_embedded_secret() {
        let redacted = redact_urls("https://host:8545/rpc?apikey=SECRET123");

        assert!(!redacted.contains("SECRET123"));
    }

    #[test]
    fn redact_urls_strips_userinfo() {
        let redacted = redact_urls("https://user:SECRET123@host/x");

        assert!(!redacted.contains("SECRET123"));
    }

    #[test]
    fn redact_urls_handles_two_urls_in_one_string() {
        let redacted =
            redact_urls("first https://a.example.org/SECRET_A then https://b.example.org/SECRET_B");

        assert!(!redacted.contains("SECRET_A"));
        assert!(!redacted.contains("SECRET_B"));
        assert!(redacted.contains("https://a.example.org"));
        assert!(redacted.contains("https://b.example.org"));
    }

    #[test]
    fn redact_urls_leaves_a_string_with_no_url_unchanged() {
        assert_eq!(redact_urls("no url here at all"), "no url here at all");
    }

    #[test]
    fn redact_urls_handles_a_realistic_transport_error_rendering() {
        let rendered = r#"Transport(Custom(reqwest::Error { kind: Request, url: "https://eth.example.org/v1/SECRET123", source: Timeout }))"#;

        let redacted = redact_urls(rendered);

        assert!(!redacted.contains("SECRET123"));
    }

    #[test]
    fn redact_urls_does_not_panic_on_multi_byte_input() {
        let input = "prefix é é https://host/pathé?q=SECRET123 end é";

        let redacted = redact_urls(input);

        // Reaching this line at all is the panic guard — a slice landing off a
        // char boundary would have aborted above. These assertions additionally
        // pin the *result*, so the test cannot silently degrade into checking
        // nothing: multi-byte text outside the URL survives, and multi-byte text
        // inside it is redacted along with the secret.
        assert_eq!(redacted, "prefix é é https://host/<redacted> end é");
        assert!(!redacted.contains("SECRET123"));
    }

    #[test]
    fn redact_urls_redacts_a_url_with_no_scheme_before_the_separator() {
        // Not a well-formed URL, but shaped like one. Failing closed matters
        // more than fidelity here: passing it through would emit the authority
        // and path verbatim.
        assert!(!redact_urls("://host/SECRET123").contains("SECRET123"));
        assert!(!redact_urls("é://host/SECRET123").contains("SECRET123"));
    }

    #[test]
    fn sanitize_transport_error_redacts_the_url_from_a_debug_rendering() {
        // Stands in for a boxed `reqwest::Error`, whose `Debug` carries a `url`
        // field verbatim. A plain `Debug` type is enough: the helper's contract
        // is "render with `{:?}`, then redact".
        #[derive(Debug)]
        #[allow(dead_code)]
        struct FakeTransportError {
            url: String,
        }

        let err = FakeTransportError {
            url: "https://eth.example.org/v1/SECRET123".to_string(),
        };

        let sanitized = sanitize_transport_error(&err);

        assert!(!sanitized.contains("SECRET123"));
        assert!(sanitized.contains("https://eth.example.org"));
    }

    #[test]
    fn secret_debug_never_renders_the_value() {
        let secret = Secret::new("SECRET123".to_string());

        let rendered = format!("{secret:?}");

        assert!(!rendered.contains("SECRET123"));
        assert!(!rendered.contains("Secret(SECRET123)"));
    }

    #[test]
    fn secret_debug_stays_redacted_inside_a_wrapper_struct() {
        #[derive(Debug)]
        struct W {
            s: Secret<String>,
        }

        let w = W {
            s: Secret::new("SECRET123".to_string()),
        };

        let rendered = format!("{w:?}");
        // Field is genuinely used, not only present for the derive, so
        // `dead_code` cannot flag it away and silently weaken the guard.
        assert_eq!(w.s.expose(), "SECRET123");

        assert!(!rendered.contains("SECRET123"));
    }
}
