// SPDX-License-Identifier: LicenseRef-Blockscout

//! Decoding of the cbor blob compilers append to the code they emit.
//!
//! Solidity and vyper use incompatible shapes for it, and the shapes are mutually
//! exclusive - solidity encodes a cbor map, vyper (since 0.4.0) a cbor array of 5.
//! Consumers that read such a blob back out of storage do not necessarily know which
//! compiler produced it, so [`CodeMetadata`] recognizes either.

use semver::Version;
use solidity_metadata::MetadataHash;
use thiserror::Error;
use vyper_cbor_auxdata::Auxdata;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodeMetadata {
    /// <https://docs.soliditylang.org/en/latest/metadata.html#encoding-of-the-metadata-hash-in-the-bytecode>
    Solidity(MetadataHash),
    /// Vyper >=0.4.0 auxdata. Earlier vyper versions emit a cbor array of 4, which
    /// this variant does not accept; those are also never stored as a separate
    /// metadata part, as the compiler emits them deterministically.
    Vyper(Auxdata),
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("cbor blob is neither solidity metadata ({solidity}) nor vyper auxdata ({vyper})")]
pub struct ParseError {
    solidity: String,
    vyper: String,
}

impl CodeMetadata {
    /// Decodes the cbor blob at the beginning of `encoded`.
    ///
    /// Returns the parsed value and the size of the cbor blob itself, **excluding**
    /// the two trailing bytes encoding the auxdata length. Both compilers store a
    /// metadata part as `<cbor blob><2 length bytes>`, so those two bytes always
    /// follow the returned size, even though solidity and vyper disagree on whether
    /// the value they encode counts them.
    pub fn from_cbor(encoded: &[u8]) -> Result<(Self, usize), ParseError> {
        let solidity = match MetadataHash::from_cbor(encoded) {
            Ok((metadata, size)) => return Ok((Self::Solidity(metadata), size)),
            Err(err) => err.to_string(),
        };
        let vyper = match Auxdata::from_cbor(encoded) {
            Ok((auxdata, size)) => return Ok((Self::Vyper(auxdata), size)),
            Err(err) => err.to_string(),
        };

        Err(ParseError { solidity, vyper })
    }

    /// The compiler version recorded inside the blob, if it records one.
    /// Solidity omits it before 0.5.9, vyper >=0.4.0 always stores it.
    pub fn compiler_version(&self) -> Option<&Version> {
        match self {
            Self::Solidity(metadata) => metadata.solc.as_ref(),
            Self::Vyper(auxdata) => Some(auxdata.version()),
        }
    }

    /// Whether both blobs were produced by the same compiler family.
    pub fn is_same_language(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Solidity(_), Self::Solidity(_)) | (Self::Vyper(_), Self::Vyper(_))
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Trailing `0x0033` = 51: solidity does not count the 2 length bytes themselves.
    const SOLIDITY_METADATA: &str = "a2646970667358221220ad5a5e9ea0429c6665dc23af78b0acca8d56235be9dc3573672141811ea4a0da64736f6c63430008070033";
    /// Trailing `0x0054` = 84: vyper does count them.
    const VYPER_AUXDATA: &str = "8558202636c527aac5420370bf53e1d11abbd62b06edbec0bd9eb95f67622a7ecb8cd7195a4a90184b18231831182318460e0e182a18381838183118311831181c151823190180a1657679706572830004030054";

    fn parse(hex_str: &str) -> (CodeMetadata, usize, Vec<u8>) {
        let encoded = hex::decode(hex_str).unwrap();
        let (metadata, size) = CodeMetadata::from_cbor(&encoded).expect("should decode");
        (metadata, size, encoded)
    }

    #[test]
    fn parses_solidity_metadata() {
        let (metadata, size, encoded) = parse(SOLIDITY_METADATA);
        assert!(matches!(metadata, CodeMetadata::Solidity(_)));
        assert_eq!(
            metadata.compiler_version(),
            Some(&Version::from_str("0.8.7").unwrap())
        );
        // the two bytes following the cbor blob encode its length
        assert_eq!(size + 2, encoded.len());
        assert_eq!(&encoded[size..], &[0x00, 0x33]);
    }

    #[test]
    fn parses_vyper_auxdata() {
        let (metadata, size, encoded) = parse(VYPER_AUXDATA);
        assert!(matches!(metadata, CodeMetadata::Vyper(_)));
        assert_eq!(
            metadata.compiler_version(),
            Some(&Version::from_str("0.4.3").unwrap())
        );
        assert_eq!(size + 2, encoded.len());
        assert_eq!(&encoded[size..], &[0x00, 0x54]);

        let CodeMetadata::Vyper(auxdata) = metadata else {
            unreachable!()
        };
        assert_eq!(auxdata.runtime_code_length(), 23114);
        assert_eq!(auxdata.immutables_length(), 384);
    }

    #[test]
    fn rejects_a_blob_of_neither_shape() {
        let error = CodeMetadata::from_cbor(&hex::decode("0000000000000004").unwrap())
            .expect_err("should not decode");
        assert!(
            error.to_string().contains("neither solidity metadata"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn languages_are_distinguished() {
        let (solidity, ..) = parse(SOLIDITY_METADATA);
        let (vyper, ..) = parse(VYPER_AUXDATA);

        assert!(solidity.is_same_language(&solidity));
        assert!(vyper.is_same_language(&vyper));
        assert!(!solidity.is_same_language(&vyper));
        assert!(!vyper.is_same_language(&solidity));
    }
}
