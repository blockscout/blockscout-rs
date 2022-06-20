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

    fn is_release(&self) -> bool {
        matches!(self, CompilerVersion::Release(_))
    }

    fn date(&self) -> Option<&NaiveDate> {
        match self {
            CompilerVersion::Nightly(v) => Some(&v.date),
            CompilerVersion::Release(_) => None,
        }
    }

    fn commit(&self) -> &[u8; 4] {
        match self {
            CompilerVersion::Nightly(v) => &v.commit,
            CompilerVersion::Release(v) => &v.commit,
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
        (
            self.version(),
            self.is_release(),
            self.date(),
            self.commit(),
        )
            .cmp(&(
                other.version(),
                other.is_release(),
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
    use rand::{rngs::StdRng, seq::SliceRandom, thread_rng, Rng, SeedableRng};

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
        let ver = check_parsing::<CompilerVersion>;

        // Release only
        assert!(ver("solc-v0.8.10+commit.fc410830") > ver("solc-v0.8.9+commit.e5eed63a"));
        // note: version with different hashes shouldn't be equal to implement sorting
        assert!(ver("solc-v0.8.9+commit.fc410830") > ver("solc-v0.8.9+commit.e5eed63a"));
        assert!(ver("solc-v0.8.9+commit.e5eed63a") == ver("solc-v0.8.9+commit.e5eed63a"));

        // Nighly only
        assert!(
            ver("solc-v0.8.15-nightly.2022.4.4+commit.fd763fa6")
                > ver("solc-v0.8.2-nightly.2022.3.16+commit.10b581b8")
        );
        assert!(
            ver("solc-v0.8.15-nightly.2022.4.4+commit.fd763fa6")
                > ver("solc-v0.8.14-nightly.2022.3.16+commit.10b581b8")
        );
        assert!(
            ver("solc-v0.8.14-nightly.2022.4.4+commit.fd763fa6")
                > ver("solc-v0.8.14-nightly.2022.3.16+commit.10b581b8")
        );
        assert!(
            ver("solc-v0.8.14-nightly.2022.4.4+commit.fd763fa6")
                > ver("solc-v0.8.14-nightly.2022.4.4+commit.10b581b8")
        );
        assert!(
            ver("solc-v0.8.14-nightly.2022.4.4+commit.fd763fa6")
                == ver("solc-v0.8.14-nightly.2022.4.4+commit.fd763fa6")
        );

        // All
        assert!(
            ver("solc-v0.5.2+commit.1df8f40c")
                > ver("solc-v0.5.2-nightly.2018.12.19+commit.88750920")
        );
        assert!(
            ver("solc-v0.5.14+commit.1df8f40c")
                > ver("solc-v0.5.2-nightly.2018.12.19+commit.88750920")
        );
        assert!(
            ver("solc-v0.5.14-nightly.2019.12.9+commit.d6667560")
                > ver("solc-v0.5.2+commit.1df8f40c")
        );
    }

    fn test_shuffle_and_sort(sorted: Vec<&str>, times: usize) {
        let sorted_versions: Vec<CompilerVersion> = sorted
            .iter()
            .map(|s| CompilerVersion::from_str(s).expect("invalid version"))
            .collect();
        let seed = thread_rng().gen();
        let mut r = StdRng::seed_from_u64(seed);
        let mut shuffled_versions = sorted_versions.clone();
        for i in 0..times {
            shuffled_versions.shuffle(&mut r);
            shuffled_versions.sort();
            let shuffled: Vec<String> = shuffled_versions.iter().map(|v| v.to_string()).collect();
            // we compare vec of strings, because in case of wrong order
            // test will show unreadable error (it will print a lot of large structures)
            assert_eq!(shuffled, sorted, "seed={}, i={}", seed, i);
        }
    }

    #[test]
    fn sort_release_versions() {
        test_shuffle_and_sort(
            vec![
                "solc-v0.8.9+commit.e5eed63a",
                "solc-v0.8.10+commit.fc410830",
                "solc-v0.8.11+commit.d7f03943",
                "solc-v0.8.12+commit.f00d7308",
                "solc-v0.8.13+commit.abaa5c0e",
                "solc-v0.8.14+commit.80d49f37",
                "solc-v0.8.15+commit.e14f2714",
            ],
            50,
        );
    }

    #[test]
    fn sort_nightly_versions() {
        test_shuffle_and_sort(
            vec![
                "solc-v0.8.14-nightly.2022.3.16+commit.10b581b8",
                "solc-v0.8.14-nightly.2022.3.17+commit.430ecb6e",
                "solc-v0.8.14-nightly.2022.3.21+commit.43f29c00",
                "solc-v0.8.14-nightly.2022.3.23+commit.b35cda59",
                "solc-v0.8.14-nightly.2022.3.24+commit.c4909e99",
                "solc-v0.8.14-nightly.2022.4.4+commit.fd763fa6",
                "solc-v0.8.14-nightly.2022.4.5+commit.34dd30d7",
                "solc-v0.8.14-nightly.2022.4.6+commit.31b54857",
                "solc-v0.8.14-nightly.2022.4.7+commit.15c2a33e",
                "solc-v0.8.14-nightly.2022.4.8+commit.d9c6ceca",
                "solc-v0.8.14-nightly.2022.4.10+commit.0b811943",
                "solc-v0.8.14-nightly.2022.4.11+commit.9e92c7a4",
                "solc-v0.8.14-nightly.2022.4.13+commit.25923c1f",
                "solc-v0.8.14-nightly.2022.4.14+commit.55917405",
                "solc-v0.8.14-nightly.2022.4.25+commit.fbecdbe7",
                "solc-v0.8.14-nightly.2022.4.28+commit.d55b84ff",
                "solc-v0.8.14-nightly.2022.5.2+commit.3e3e73e3",
                "solc-v0.8.14-nightly.2022.5.4+commit.84c64edf",
                "solc-v0.8.14-nightly.2022.5.5+commit.1dba6aaf",
                "solc-v0.8.14-nightly.2022.5.9+commit.463e4175",
                "solc-v0.8.14-nightly.2022.5.10+commit.9f6d3dea",
                "solc-v0.8.14-nightly.2022.5.11+commit.0c0ff4fc",
                "solc-v0.8.14-nightly.2022.5.12+commit.aafda389",
                "solc-v0.8.14-nightly.2022.5.13+commit.a3bd01d9",
                "solc-v0.8.14-nightly.2022.5.17+commit.80d49f37",
                "solc-v0.8.15-nightly.2022.5.18+commit.de7daaa2",
                "solc-v0.8.15-nightly.2022.5.19+commit.0cb95902",
                "solc-v0.8.15-nightly.2022.5.20+commit.02567fd3",
                "solc-v0.8.15-nightly.2022.5.23+commit.21591531",
                "solc-v0.8.15-nightly.2022.5.25+commit.fdc3c8ee",
                "solc-v0.8.15-nightly.2022.5.27+commit.095cc647",
                "solc-v0.8.15-nightly.2022.5.31+commit.baf56aff",
                "solc-v0.8.15-nightly.2022.6.1+commit.3f84837e",
                "solc-v0.8.15-nightly.2022.6.2+commit.035f6abb",
                "solc-v0.8.15-nightly.2022.6.6+commit.3948391c",
                "solc-v0.8.15-nightly.2022.6.7+commit.8c87f58f",
                "solc-v0.8.15-nightly.2022.6.8+commit.9b220a20",
                "solc-v0.8.15-nightly.2022.6.9+commit.80f6a13d",
                "solc-v0.8.15-nightly.2022.6.10+commit.efcbc79b",
                "solc-v0.8.15-nightly.2022.6.13+commit.82e5339d",
                "solc-v0.8.15-nightly.2022.6.14+commit.dccc06cc",
                "solc-v0.8.16-nightly.2022.6.15+commit.f904bb06",
                "solc-v0.8.16-nightly.2022.6.16+commit.b80f4baa",
                "solc-v0.8.16-nightly.2022.6.17+commit.be470c16",
            ],
            50,
        );
    }

    #[test]
    fn sort_all_versions() {
        test_shuffle_and_sort(
            vec![
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
                "solc-v0.5.2+commit.1df8f40c",
                "solc-v0.5.14-nightly.2019.12.9+commit.d6667560",
                "solc-v0.5.15+commit.6a57276f",
                "solc-v0.5.16+commit.9c3226ce",
                "solc-v0.5.17+commit.d19bba13",
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
                "solc-v0.6.2+commit.bacdbe57",
                "solc-v0.6.3-nightly.2020.1.27+commit.8809d4bb",
                "solc-v0.6.3-nightly.2020.1.28+commit.2d3bd91d",
                "solc-v0.6.3-nightly.2020.1.29+commit.01eb9a5b",
                "solc-v0.6.3-nightly.2020.1.30+commit.ad98bf0f",
                "solc-v0.6.3-nightly.2020.1.31+commit.b6190e06",
                "solc-v0.6.3+commit.8dda9521",
                "solc-v0.6.12+commit.27d51765",
            ],
            100,
        );
    }
}
