use semver::Version;
use std::{fmt::Display, str::FromStr};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("error parsing the string: {0}")]
    Parse(String),
    #[error("could not parse the semver: {0}")]
    SemVer(semver::Error),
    #[error("wrong version format, expected maj.min.patch, got: {0}")]
    VersionFormat(String),
    #[error("couldn't parse commit hash")]
    CommitHash(hex::FromHexError),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReleaseVersion {
    pub version: Version,
    pub commit: [u8; 4],
}

impl FromStr for ReleaseVersion {
    type Err = ParseError;

    /// Parses release version from string formated as
    /// `solc-v*VERSION*+commit.*COMMITHASH*`, example
    /// `solc-v0.8.9+commit.e5eed63a`
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (version_str, commit_hash) = sscanf::scanf!(s, "solc-v{}+commit.{}", String, String)
            .map_err(|e| ParseError::Parse(format!("{:?}", e)))?;
        let version = Version::from_str(&version_str).map_err(ParseError::SemVer)?;
        if !version.pre.is_empty() || !version.build.is_empty() {
            return Err(ParseError::VersionFormat(version_str));
        }
        let mut commit = [0; 4];
        hex::decode_to_slice(&commit_hash, &mut commit).map_err(ParseError::CommitHash)?;
        Ok(Self { version, commit })
    }
}

impl Display for ReleaseVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "solc-v{}+commit.{}",
            self.version,
            hex::encode(self.commit)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NightlyVersion {
    pub version: Version,
    pub date: String,
    pub commit: [u8; 4],
}

impl FromStr for NightlyVersion {
    type Err = ParseError;

    /// Parses nigthly version from string formated as
    /// `solc-v*VERSION*-nightly.*DATE*+commit.*COMMITHASH*`, example
    /// `solc-v0.8.8-nightly.2021.9.9+commit.dea1b9ec`
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (version_str, date, commit_hash) =
            sscanf::scanf!(s, "solc-v{}-nightly.{}+commit.{}", String, String, String)
                .map_err(|e| ParseError::Parse(format!("{:?}", e)))?;
        let version = Version::from_str(&version_str).map_err(ParseError::SemVer)?;
        if !version.pre.is_empty() || !version.build.is_empty() {
            return Err(ParseError::VersionFormat(version_str));
        }
        let mut commit = [0; 4];
        hex::decode_to_slice(&commit_hash, &mut commit).map_err(ParseError::CommitHash)?;
        Ok(Self {
            version,
            date,
            commit,
        })
    }
}

impl Display for NightlyVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "solc-v{}-nightly.{}+commit.{}",
            self.version,
            self.date,
            hex::encode(self.commit)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CompilerVersion {
    Release(ReleaseVersion),
    Nightly(NightlyVersion),
}

impl FromStr for CompilerVersion {
    type Err = ParseError;

    /// Parses compiler version
    /// If version contains "nightly", tries to parse it as a nightly version
    /// Else tries to parse it as a release version
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains("nightly") {
            Ok(Self::Nightly(NightlyVersion::from_str(s)?))
        } else {
            Ok(Self::Release(ReleaseVersion::from_str(s)?))
        }
    }
}

impl Display for CompilerVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompilerVersion::Release(v) => v.fmt(f),
            CompilerVersion::Nightly(v) => v.fmt(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_parsing<T: FromStr + ToString>(ver_str: &str) -> T
    where
        <T as std::str::FromStr>::Err: std::fmt::Debug,
    {
        let ver = T::from_str(ver_str).unwrap();
        assert_eq!(ver_str, ver.to_string());
        ver
    }

    #[test]
    fn parse_release() {
        let ver = check_parsing::<ReleaseVersion>("solc-v0.8.9+commit.e5eed63a");
        assert_eq!(ver.version, Version::new(0, 8, 9));
        assert_eq!(ver.commit, [229, 238, 214, 58]);
        check_parsing::<ReleaseVersion>("solc-v0.0.0+commit.00000000");
        check_parsing::<ReleaseVersion>("solc-v123456789.987654321.0+commit.ffffffff");
        check_parsing::<ReleaseVersion>("solc-v1.2.3+commit.01234567");
        check_parsing::<ReleaseVersion>("solc-v3.2.1+commit.89abcdef");
    }

    #[test]
    fn parse_invalid_release() {
        ReleaseVersion::from_str("").unwrap_err();
        ReleaseVersion::from_str("sometext").unwrap_err();
        ReleaseVersion::from_str("solc-0.8.9+commit.deadbeef").unwrap_err();
        ReleaseVersion::from_str("solc-v0.8+commit.deadbeef").unwrap_err();
        ReleaseVersion::from_str("solc-v0.8.9commit.deadbeef").unwrap_err();
        ReleaseVersion::from_str("solcv0.8.9+commit.deadbeef").unwrap_err();
        ReleaseVersion::from_str("solc-v0.8.9+commitdeadbeef").unwrap_err();
        ReleaseVersion::from_str("solc-v+commit.deadbeef").unwrap_err();
        ReleaseVersion::from_str("solc-v0.8.9+commit.").unwrap_err();
        ReleaseVersion::from_str("-v0.8.9+commit.deadbeef").unwrap_err();
        ReleaseVersion::from_str("solc-v0.8.9+commit.deadbe").unwrap_err();
        ReleaseVersion::from_str("solc-v0.8.9+commit.alivebee").unwrap_err();
        ReleaseVersion::from_str("solc-v0.8.9-pre+commit.deadbeef").unwrap_err();
        ReleaseVersion::from_str("solc-v0.8.9-nightly.2021.9.11+commit.e5eed63a").unwrap_err();
    }

    #[test]
    fn parse_nightly() {
        let ver = check_parsing::<NightlyVersion>("solc-v0.8.9-nightly.2021.9.11+commit.e5eed63a");
        assert_eq!(ver.version, Version::new(0, 8, 9));
        assert_eq!(ver.date, "2021.9.11");
        assert_eq!(ver.commit, [229, 238, 214, 58]);
        check_parsing::<NightlyVersion>("solc-v0.0.0-nightly.1990.1.1+commit.00000000");
        check_parsing::<NightlyVersion>(
            "solc-v123456789.987654321.0-nightly.2100.12.30+commit.ffffffff",
        );
    }

    #[test]
    fn parse_invalid_nightly() {
        NightlyVersion::from_str("").unwrap_err();
        NightlyVersion::from_str("sometext").unwrap_err();
        NightlyVersion::from_str("solc-0.8.9-nightly.2021.9.11+commit.e5eed63a").unwrap_err();
        NightlyVersion::from_str("solc-v0.8-nightly.2021.9.11+commit.e5eed63a").unwrap_err();
        NightlyVersion::from_str("solc-v0.8.9.2021.9.11+commit.e5eed63a").unwrap_err();
        NightlyVersion::from_str("solc-v0.8.9-nightly.+commit.e5eed63a").unwrap_err();
        NightlyVersion::from_str("solc-v-nightly.2021.9.11+commit.e5eed63a").unwrap_err();
        NightlyVersion::from_str("solc-v0.8.9-nightly.2021.9.11+commit.").unwrap_err();
        NightlyVersion::from_str("solc-v0.8.9-nightly.2021.9.11+commit.alivebee").unwrap_err();
        NightlyVersion::from_str("solc-v0.8.9-nighly.2021.9.11+commit.e5eed63a").unwrap_err();
        NightlyVersion::from_str("-v0.8.9-nightly.2021.9.11+commit.e5eed63a").unwrap_err();
        NightlyVersion::from_str("solc-v0.8.9+commit.deadbeef").unwrap_err();
        NightlyVersion::from_str("solc-v0.8.9-pre-nightly.2021.9.11+commit.e5eed63a").unwrap_err();
    }

    #[test]
    fn parse_version() {
        assert_eq!(
            check_parsing::<CompilerVersion>("solc-v0.8.9+commit.e5eed63a"),
            CompilerVersion::Release(
                ReleaseVersion::from_str("solc-v0.8.9+commit.e5eed63a").unwrap()
            )
        );
        assert_eq!(
            check_parsing::<CompilerVersion>("solc-v0.8.9-nightly.2021.9.11+commit.e5eed63a"),
            CompilerVersion::Nightly(
                NightlyVersion::from_str("solc-v0.8.9-nightly.2021.9.11+commit.e5eed63a").unwrap()
            )
        );
    }
}
