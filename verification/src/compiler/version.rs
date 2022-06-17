use chrono::NaiveDate;
use semver::Version;
use std::{cmp::Ordering, fmt::Display, str::FromStr};
use thiserror::Error;

const DATE_FORMAT: &str = "%Y.%-m.%-d";

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
    pub date: NaiveDate,
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
        let date = NaiveDate::parse_from_str(&date, DATE_FORMAT)
            .map_err(|e| ParseError::Parse(e.to_string()))?;
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
            self.date.format(DATE_FORMAT),
            hex::encode(self.commit)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CompilerVersion {
    Release(ReleaseVersion),
    Nightly(NightlyVersion),
}

impl CompilerVersion {
    fn version(&self) -> &Version {
        match self {
            CompilerVersion::Nightly(v) => &v.version,
            CompilerVersion::Release(v) => &v.version,
        }
    }

    fn date(&self) -> Option<NaiveDate> {
        match self {
            CompilerVersion::Nightly(v) => Some(v.date.clone()),
            CompilerVersion::Release(_) => None,
        }
    }

    fn commit(&self) -> [u8; 4] {
        match self {
            CompilerVersion::Nightly(v) => v.commit,
            CompilerVersion::Release(v) => v.commit,
        }
    }
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

impl Ord for CompilerVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.version(), self.date(), self.commit()).cmp(&(
            other.version(),
            other.date(),
            other.commit(),
        ))
    }
}

impl PartialOrd for CompilerVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
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
        assert_eq!(ver.date, NaiveDate::from_ymd(2021, 9, 11));
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

    #[test]
    fn order_versions() {
        let versions = vec![
            "solc-v0.5.2-nightly.2018.12.7+commit.52ff3c94",
            "solc-v0.5.2-nightly.2018.12.6+commit.5a08ae5e",
            "solc-v0.5.2-nightly.2018.12.5+commit.6efe2a52",
            "solc-v0.5.2-nightly.2018.12.4+commit.e49f37be",
            "solc-v0.5.2-nightly.2018.12.3+commit.e6a01d26",
            "solc-v0.5.2-nightly.2018.12.19+commit.88750920",
            "solc-v0.5.2-nightly.2018.12.18+commit.4b43aeca",
            "solc-v0.5.2-nightly.2018.12.17+commit.12874029",
            "solc-v0.5.2-nightly.2018.12.13+commit.b3e2ba15",
            "solc-v0.5.2-nightly.2018.12.12+commit.85291bcb",
            "solc-v0.5.2-nightly.2018.12.11+commit.599760b6",
            "solc-v0.5.2-nightly.2018.12.10+commit.6240d9e7",
            "solc-v0.5.2+commit.1df8f40c",
            "solc-v0.5.17+commit.d19bba13",
            "solc-v0.5.16+commit.9c3226ce",
            "solc-v0.5.15+commit.6a57276f",
            "solc-v0.5.14-nightly.2019.12.9+commit.d6667560",
            "solc-v0.6.3-nightly.2020.1.31+commit.b6190e06",
            "solc-v0.6.3-nightly.2020.1.30+commit.ad98bf0f",
            "solc-v0.6.3-nightly.2020.1.29+commit.01eb9a5b",
            "solc-v0.6.3-nightly.2020.1.28+commit.2d3bd91d",
            "solc-v0.6.3-nightly.2020.1.27+commit.8809d4bb",
            "solc-v0.6.3+commit.8dda9521",
            "solc-v0.6.2+commit.bacdbe57",
            "solc-v0.6.12+commit.27d51765",
            "solc-v0.6.2-nightly.2020.1.9+commit.17158995",
            "solc-v0.6.2-nightly.2020.1.8+commit.12b52ae6",
            "solc-v0.6.2-nightly.2020.1.27+commit.1bdb409b",
            "solc-v0.6.2-nightly.2020.1.23+commit.3add37a2",
            "solc-v0.6.2-nightly.2020.1.22+commit.641bb815",
            "solc-v0.6.2-nightly.2020.1.20+commit.470c19eb",
            "solc-v0.6.2-nightly.2020.1.17+commit.92908f52",
            "solc-v0.6.2-nightly.2020.1.16+commit.3d4a2219",
            "solc-v0.6.2-nightly.2020.1.15+commit.9d9a7ebe",
            "solc-v0.6.2-nightly.2020.1.14+commit.6dbadf69",
            "solc-v0.6.2-nightly.2020.1.13+commit.408458b7",
        ];
        let mut versions: Vec<CompilerVersion> = versions
            .iter()
            .map(|s| CompilerVersion::from_str(s).expect("invalid version"))
            .collect();
        versions.sort();
        let versions: Vec<String> = versions.iter().map(|v| v.to_string()).collect();
        assert_eq!(
            versions,
            vec![
                "solc-v0.5.2+commit.1df8f40c",
                "solc-v0.5.2-nightly.2018.12.3+commit.e6a01d26",
                "solc-v0.5.2-nightly.2018.12.4+commit.e49f37be",
                "solc-v0.5.2-nightly.2018.12.5+commit.6efe2a52",
                "solc-v0.5.2-nightly.2018.12.6+commit.5a08ae5e",
                "solc-v0.5.2-nightly.2018.12.7+commit.52ff3c94",
                "solc-v0.5.2-nightly.2018.12.10+commit.6240d9e7",
                "solc-v0.5.2-nightly.2018.12.11+commit.599760b6",
                "solc-v0.5.2-nightly.2018.12.12+commit.85291bcb",
                "solc-v0.5.2-nightly.2018.12.13+commit.b3e2ba15",
                "solc-v0.5.2-nightly.2018.12.17+commit.12874029",
                "solc-v0.5.2-nightly.2018.12.18+commit.4b43aeca",
                "solc-v0.5.2-nightly.2018.12.19+commit.88750920",
                "solc-v0.5.14-nightly.2019.12.9+commit.d6667560",
                "solc-v0.5.15+commit.6a57276f",
                "solc-v0.5.16+commit.9c3226ce",
                "solc-v0.5.17+commit.d19bba13",
                "solc-v0.6.2+commit.bacdbe57",
                "solc-v0.6.2-nightly.2020.1.8+commit.12b52ae6",
                "solc-v0.6.2-nightly.2020.1.9+commit.17158995",
                "solc-v0.6.2-nightly.2020.1.13+commit.408458b7",
                "solc-v0.6.2-nightly.2020.1.14+commit.6dbadf69",
                "solc-v0.6.2-nightly.2020.1.15+commit.9d9a7ebe",
                "solc-v0.6.2-nightly.2020.1.16+commit.3d4a2219",
                "solc-v0.6.2-nightly.2020.1.17+commit.92908f52",
                "solc-v0.6.2-nightly.2020.1.20+commit.470c19eb",
                "solc-v0.6.2-nightly.2020.1.22+commit.641bb815",
                "solc-v0.6.2-nightly.2020.1.23+commit.3add37a2",
                "solc-v0.6.2-nightly.2020.1.27+commit.1bdb409b",
                "solc-v0.6.3+commit.8dda9521",
                "solc-v0.6.3-nightly.2020.1.27+commit.8809d4bb",
                "solc-v0.6.3-nightly.2020.1.28+commit.2d3bd91d",
                "solc-v0.6.3-nightly.2020.1.29+commit.01eb9a5b",
                "solc-v0.6.3-nightly.2020.1.30+commit.ad98bf0f",
                "solc-v0.6.3-nightly.2020.1.31+commit.b6190e06",
                "solc-v0.6.12+commit.27d51765",
            ]
        )
    }
}
