use semver::Version;
use std::{fmt::Display, str::FromStr};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReleaseVersion {
    pub version: Version,
    pub commit: String,
}

impl ReleaseVersion {
    pub fn new(version: Version, commit: String) -> Self {
        Self { version, commit }
    }
}

/// Parses release version from string formated as
/// `solc-v*VERSION*+commit.*COMMITHASH*`, example
/// `solc-v0.8.9+commit.e5eed63a`
impl FromStr for ReleaseVersion {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parsed = sscanf::scanf!(s, "solc-v{String}+commit.{String}")
            .map_err(|e| anyhow::anyhow!("{:?}", e))?;
        let version = Version::from_str(&parsed.0)?;
        let commit = parsed.1;
        if commit.len() != 8 {
            anyhow::bail!("expected commit hash of length 8, got {}", commit);
        }
        Ok(Self { version, commit })
    }
}

impl Display for ReleaseVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "solc-v{}+commit.{}", self.version, self.commit)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NightlyVersion {
    pub version: Version,
    pub date: String,
    pub commit: String,
}

impl NightlyVersion {
    pub fn new(version: Version, date: String, commit: String) -> Self {
        Self {
            version,
            date,
            commit,
        }
    }
}

/// Parses nigthly version from string formated as
/// `solc-v*VERSION*-nightly.*DATE*+commit.*COMMITHASH*`, example
/// `solc-v0.8.8-nightly.2021.9.9+commit.dea1b9ec`
impl FromStr for NightlyVersion {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parsed = sscanf::scanf!(s, "solc-v{String}-nightly.{String}+commit.{String}")
            .map_err(|e| anyhow::anyhow!("{:?}", e))?;
        let version = Version::from_str(&parsed.0)?;
        let date = parsed.1;
        let commit = parsed.2;
        if commit.len() != 8 {
            anyhow::bail!("expected commit hash of length 8, got {}", commit);
        }
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
            self.version, self.date, self.commit
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CompilerVersion {
    Release(ReleaseVersion),
    Nightly(NightlyVersion),
}

/// Parses compiler version
/// If version contains "nightly", tries to parse it as a nightly version
/// Else tries to parse it as a release version
impl FromStr for CompilerVersion {
    type Err = anyhow::Error;
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
