use anyhow::{Result, bail};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AmbVersion {
    V6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MediatorVersion {
    V6,
    V8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HeaderLayout {
    Modern,
    #[allow(dead_code)]
    Legacy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum AmbSide {
    Foreign,
    Home,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AmbGrammar {
    pub(crate) version: AmbVersion,
    pub(crate) header_layout: HeaderLayout,
    pub(crate) foreign_events: &'static [&'static str],
    pub(crate) home_events: &'static [&'static str],
    /// `(event name, canonical Solidity signature)` for the Foreign side.
    /// The expected `topic0` is `keccak256(signature)` — see
    /// `abi::assert_canonical_topics`. xDai's proxy events share these exact
    /// names, so name-set inference alone cannot tell the two protocols
    /// apart; this is what does.
    pub(crate) foreign_canonical_topics: &'static [(&'static str, &'static str)],
    /// Home-side counterpart of [`Self::foreign_canonical_topics`].
    pub(crate) home_canonical_topics: &'static [(&'static str, &'static str)],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MediatorGrammar {
    pub(crate) version: MediatorVersion,
    pub(crate) events: &'static [&'static str],
}

static AMB_V6_FOREIGN_CANONICAL_TOPICS: &[(&str, &str)] = &[
    (
        "UserRequestForAffirmation",
        "UserRequestForAffirmation(bytes32,bytes)",
    ),
    (
        "RelayedMessage",
        "RelayedMessage(address,address,bytes32,bool)",
    ),
];

static AMB_V6_HOME_CANONICAL_TOPICS: &[(&str, &str)] = &[
    (
        "UserRequestForSignature",
        "UserRequestForSignature(bytes32,bytes)",
    ),
    (
        "AffirmationCompleted",
        "AffirmationCompleted(address,address,bytes32,bool)",
    ),
    (
        "SignedForAffirmation",
        "SignedForAffirmation(address,bytes32)",
    ),
    (
        "SignedForUserRequest",
        "SignedForUserRequest(address,bytes32)",
    ),
    (
        "CollectedSignatures",
        "CollectedSignatures(address,bytes32,uint256)",
    ),
];

static AMB_V6_GRAMMAR: AmbGrammar = AmbGrammar {
    version: AmbVersion::V6,
    header_layout: HeaderLayout::Modern,
    foreign_events: &["UserRequestForAffirmation", "RelayedMessage"],
    home_events: &[
        "UserRequestForSignature",
        "AffirmationCompleted",
        "SignedForAffirmation",
        "SignedForUserRequest",
        "CollectedSignatures",
    ],
    foreign_canonical_topics: AMB_V6_FOREIGN_CANONICAL_TOPICS,
    home_canonical_topics: AMB_V6_HOME_CANONICAL_TOPICS,
};

static MEDIATOR_V6_GRAMMAR: MediatorGrammar = MediatorGrammar {
    version: MediatorVersion::V6,
    events: &[
        "TokensBridgingInitiated",
        "TokensBridged",
        "NewTokenRegistered",
        "FailedMessageFixed",
    ],
};

static MEDIATOR_V8_GRAMMAR: MediatorGrammar = MediatorGrammar {
    version: MediatorVersion::V8,
    events: MEDIATOR_V6_GRAMMAR.events,
};

pub(crate) fn amb_grammar_for(version: i16) -> Result<&'static AmbGrammar> {
    match version {
        6 => Ok(&AMB_V6_GRAMMAR),
        _ => bail!("no AMB grammar registered for version {version}"),
    }
}

pub(crate) fn mediator_grammar_for(version: i16) -> Result<&'static MediatorGrammar> {
    match version {
        6 => Ok(&MEDIATOR_V6_GRAMMAR),
        8 => Ok(&MEDIATOR_V8_GRAMMAR),
        _ => bail!("no Omnibridge mediator grammar registered for version {version}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grammar_lookup_known_versions() {
        assert_eq!(amb_grammar_for(6).unwrap().version, AmbVersion::V6);
        assert_eq!(
            mediator_grammar_for(6).unwrap().version,
            MediatorVersion::V6
        );
        assert_eq!(
            mediator_grammar_for(8).unwrap().version,
            MediatorVersion::V8
        );
    }

    #[test]
    fn test_grammar_lookup_unknown_versions_returns_error() {
        assert!(amb_grammar_for(5).is_err());
        assert!(mediator_grammar_for(7).is_err());
    }
}
