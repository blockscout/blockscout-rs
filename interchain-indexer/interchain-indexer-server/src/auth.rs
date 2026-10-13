// SPDX-License-Identifier: LicenseRef-Blockscout

//! API-key authentication for the operator write API.
//!
//! Keys are configured as named SHA-256 digests; the key name is the actor
//! recorded in the audit log. The key itself, its digest and the request
//! metadata never reach a log line, an error message or a `Debug` impl.

use crate::settings::WriteApiSettings;
use anyhow::{Context, bail, ensure};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fmt, sync::Arc};

const API_KEY_HEADER: &str = "x-api-key";

/// Returned for every authentication failure; the reason goes to the trace
/// event only.
const UNAUTHENTICATED_MESSAGE: &str = "missing or invalid x-api-key";

type KeyDigest = [u8; 32];

/// Catalogue of write-API keys, validated at startup.
pub(crate) struct WriteApiAuth {
    keys: Vec<(String, KeyDigest)>,
}

// Names only: never print digests.
impl fmt::Debug for WriteApiAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WriteApiAuth")
            .field(
                "key_names",
                &self.keys.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            )
            .finish()
    }
}

/// An authenticated operator. Only [`WriteApiAuth::authenticate`] creates one,
/// so holding an `Actor` proves a key was presented and matched.
#[derive(Debug)]
pub(crate) struct Actor {
    name: Arc<str>,
}

impl Actor {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for Actor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

#[derive(Clone, Copy)]
enum AuthFailure {
    NoKeysConfigured,
    MissingKey,
    UnknownKey,
}

impl AuthFailure {
    fn as_str(self) -> &'static str {
        match self {
            Self::NoKeysConfigured => "no_keys_configured",
            Self::MissingKey => "missing_key",
            Self::UnknownKey => "unknown_key",
        }
    }
}

impl WriteApiAuth {
    /// Validates the configured catalogue. Any error is a startup error.
    ///
    /// Error messages carry key names only: a value that is not a digest may
    /// be the key itself.
    pub(crate) fn from_settings(settings: &WriteApiSettings) -> anyhow::Result<Self> {
        let mut keys: Vec<(String, KeyDigest)> = Vec::with_capacity(settings.keys_sha256.len());
        let mut owners: HashMap<KeyDigest, &str> = HashMap::new();

        for (name, value) in &settings.keys_sha256 {
            ensure!(
                !name.is_empty(),
                "write_api.keys_sha256 contains an empty key name"
            );
            let digest = parse_digest(value).with_context(|| {
                format!(
                    "write_api.keys_sha256.{name} must be a 64-character hex SHA-256 digest of the key (did you put the key itself?)"
                )
            })?;
            if let Some(first) = owners.insert(digest, name) {
                bail!(
                    "write_api.keys_sha256.{first} and write_api.keys_sha256.{name} share one digest; every key needs its own so the actor is unambiguous"
                );
            }
            keys.push((name.clone(), digest));
        }

        match keys.is_empty() {
            true => tracing::warn!(
                "write api has no keys configured; every write method will reject requests"
            ),
            false => tracing::info!(
                key_names = ?keys.iter().map(|(name, _)| name).collect::<Vec<_>>(),
                "write api keys loaded"
            ),
        }
        Ok(Self { keys })
    }

    /// Resolves the actor behind the request's `x-api-key` header.
    ///
    /// Every failure yields the same `UNAUTHENTICATED` status; the reason is
    /// only traced.
    pub(crate) fn authenticate<T>(
        &self,
        method: &'static str,
        request: &tonic::Request<T>,
    ) -> Result<Actor, tonic::Status> {
        let presented = request
            .metadata()
            .get(API_KEY_HEADER)
            .map(|value| value.as_bytes());

        self.resolve(presented)
            .map(|name| Actor {
                name: Arc::from(name),
            })
            .map_err(|failure| {
                tracing::warn!(
                    method,
                    auth_failure = failure.as_str(),
                    "write api authentication failed"
                );
                tonic::Status::unauthenticated(UNAUTHENTICATED_MESSAGE)
            })
    }

    fn resolve(&self, presented: Option<&[u8]>) -> Result<&str, AuthFailure> {
        if self.keys.is_empty() {
            Err(AuthFailure::NoKeysConfigured)?;
        }
        let presented = presented
            .filter(|value| !value.is_empty())
            .ok_or(AuthFailure::MissingKey)?;

        // Raw bytes, not `to_str()`: a non-ASCII value must not panic, it just
        // does not match. Every entry is compared, with no early exit; digests
        // are unique (checked at startup), so at most one entry matches.
        let digest: KeyDigest = Sha256::digest(presented).into();
        let mut matched: Option<&str> = None;
        for (name, stored) in &self.keys {
            if *stored == digest {
                matched.get_or_insert(name.as_str());
            }
        }
        matched.ok_or(AuthFailure::UnknownKey)
    }
}

/// Decodes a 64-character hex string (either case) into a SHA-256 digest.
///
/// Returns `None` rather than the decoder's error: that error names the
/// offending character of a value that may be the key itself.
fn parse_digest(value: &str) -> Option<KeyDigest> {
    KeyDigest::try_from(hex::decode(value).ok()?).ok()
}

/// Helpers for tests of the code that sits behind authentication: they get an
/// [`Actor`] the same way production does, by presenting a configured key.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::collections::BTreeMap;

    pub(crate) fn sha256_hex(key: &str) -> String {
        hex::encode(Sha256::digest(key.as_bytes()))
    }

    pub(crate) fn settings_with_digests(keys: &[(&str, String)]) -> WriteApiSettings {
        WriteApiSettings {
            keys_sha256: keys
                .iter()
                .map(|(name, digest)| (name.to_string(), digest.clone()))
                .collect::<BTreeMap<_, _>>(),
        }
    }

    /// Catalogue holding one key: `key_name` authenticates `key`.
    pub(crate) fn auth_with_key(key_name: &str, key: &str) -> WriteApiAuth {
        auth_with_keys(&[(key_name, key)])
    }

    pub(crate) fn auth_with_keys(keys: &[(&str, &str)]) -> WriteApiAuth {
        let digests: Vec<_> = keys
            .iter()
            .map(|(name, key)| (*name, sha256_hex(key)))
            .collect();
        WriteApiAuth::from_settings(&settings_with_digests(&digests)).expect("valid catalogue")
    }

    pub(crate) fn request_with_key<T>(message: T, key: &str) -> tonic::Request<T> {
        let mut request = tonic::Request::new(message);
        request
            .metadata_mut()
            .insert(API_KEY_HEADER, key.parse().expect("ascii key"));
        request
    }
}

#[cfg(test)]
mod tests {
    use super::{test_support::*, *};
    use tonic::{Code, metadata::AsciiMetadataValue};

    fn assert_unauthenticated(result: Result<Actor, tonic::Status>) {
        let status = result.expect_err("must be rejected");
        assert_eq!(status.code(), Code::Unauthenticated);
        assert_eq!(status.message(), UNAUTHENTICATED_MESSAGE);
    }

    #[test]
    fn write_api_auth_accepts_lowercase_and_uppercase_digests() {
        let lower = sha256_hex("lower-key");
        let upper = sha256_hex("upper-key").to_uppercase();
        let auth = WriteApiAuth::from_settings(&settings_with_digests(&[
            ("lower", lower),
            ("upper", upper),
        ]))
        .expect("both cases are accepted");

        let actor = auth
            .authenticate("Test", &request_with_key((), "upper-key"))
            .expect("uppercase digest must match");
        assert_eq!(actor.name(), "upper");
        let actor = auth
            .authenticate("Test", &request_with_key((), "lower-key"))
            .expect("lowercase digest must match");
        assert_eq!(actor.name(), "lower");
    }

    #[test]
    fn write_api_auth_rejects_a_value_that_is_not_a_64_hex_digest_without_echoing_it() {
        let bad_values = [
            "my-secret-key".to_string(),
            String::new(),
            "0x".to_string(),
            "ab".repeat(31),
            "ab".repeat(33),
            format!("my-secret-key{}", "a".repeat(51)),
        ];
        for bad in &bad_values {
            let err = WriteApiAuth::from_settings(&settings_with_digests(&[("ops", bad.clone())]))
                .expect_err("not a digest");
            let rendered = format!("{err:#}");
            assert!(rendered.contains("write_api.keys_sha256.ops"), "{rendered}");
            assert!(rendered.contains("64-character hex SHA-256"), "{rendered}");
            assert!(
                bad.is_empty() || !rendered.contains(bad.as_str()),
                "the value must not be echoed: {rendered}"
            );
            assert!(!rendered.contains("my-secret-key"), "{rendered}");
        }
    }

    #[test]
    fn write_api_auth_rejects_an_empty_key_name() {
        let err = WriteApiAuth::from_settings(&settings_with_digests(&[("", sha256_hex("k"))]))
            .expect_err("empty name");
        assert!(format!("{err:#}").contains("empty key name"));
    }

    #[test]
    fn write_api_auth_rejects_a_digest_shared_by_two_names() {
        let digest = sha256_hex("shared-key");
        let err = WriteApiAuth::from_settings(&settings_with_digests(&[
            ("alice", digest.clone()),
            ("bob", digest.to_uppercase()),
        ]))
        .expect_err("shared digest");
        let rendered = format!("{err:#}");
        assert!(rendered.contains("alice") && rendered.contains("bob"));
        assert!(
            !rendered.to_lowercase().contains(&digest),
            "the digest must not be echoed: {rendered}"
        );
    }

    #[test]
    fn write_api_auth_empty_catalogue_is_ok_and_rejects_everything() {
        let auth = WriteApiAuth::from_settings(&WriteApiSettings::default())
            .expect("an empty catalogue is valid (fail-closed)");
        assert_unauthenticated(auth.authenticate("Test", &request_with_key((), "any-key")));
        assert_unauthenticated(auth.authenticate("Test", &tonic::Request::new(())));
    }

    #[test]
    fn write_api_auth_missing_and_empty_header_are_rejected() {
        let auth = auth_with_key("ops", "test-key");
        assert_unauthenticated(auth.authenticate("Test", &tonic::Request::new(())));
        assert_unauthenticated(auth.authenticate("Test", &request_with_key((), "")));
    }

    #[test]
    fn write_api_auth_unknown_key_is_rejected() {
        let auth = auth_with_key("ops", "test-key");
        assert_unauthenticated(auth.authenticate("Test", &request_with_key((), "other-key")));
    }

    #[test]
    fn write_api_auth_non_ascii_value_does_not_panic() {
        let auth = auth_with_key("ops", "test-key");
        let mut request = tonic::Request::new(());
        let value = AsciiMetadataValue::try_from(&[0xff_u8, b'k'][..]).expect("opaque bytes");
        request.metadata_mut().insert(API_KEY_HEADER, value);
        assert_unauthenticated(auth.authenticate("Test", &request));
    }

    #[test]
    fn write_api_auth_valid_key_of_any_length_yields_its_actor_name() {
        let long_key = "k".repeat(4096);
        let auth = auth_with_keys(&[("short", "k"), ("long", &long_key), ("mid", "test-key")]);
        for (key, name) in [
            ("k", "short"),
            (long_key.as_str(), "long"),
            ("test-key", "mid"),
        ] {
            let actor = auth
                .authenticate("Test", &request_with_key((), key))
                .expect("configured key must authenticate");
            assert_eq!(actor.name(), name);
            assert_eq!(actor.to_string(), name);
        }
    }

    #[test]
    fn write_api_auth_debug_prints_names_only() {
        let digest = sha256_hex("test-key");
        let auth = WriteApiAuth::from_settings(&settings_with_digests(&[("ops", digest.clone())]))
            .expect("valid catalogue");
        let rendered = format!("{auth:?}");
        assert!(rendered.contains("ops"), "{rendered}");
        assert!(!rendered.contains(&digest), "{rendered}");
        assert!(!rendered.contains("test-key"), "{rendered}");
    }
}
