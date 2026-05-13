use anyhow::{Result, bail};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AmbVersion {
    V6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MediatorVersion {
    EthV6,
    GnosisV8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HeaderLayout {
    Modern,
    #[allow(dead_code)]
    Legacy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MediatorGrammar {
    pub(crate) version: MediatorVersion,
    pub(crate) events: &'static [&'static str],
    pub(crate) functions: &'static [&'static str],
}

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
};

static ETH_MEDIATOR_V6_GRAMMAR: MediatorGrammar = MediatorGrammar {
    version: MediatorVersion::EthV6,
    events: &[
        "TokensBridgingInitiated",
        "TokensBridged",
        "NewTokenRegistered",
        "FailedMessageFixed",
    ],
    functions: &[
        "handleNativeTokens",
        "handleNativeTokensAndCall",
        "handleBridgedTokens",
        "handleBridgedTokensAndCall",
        "deployAndHandleBridgedTokens",
        "deployAndHandleBridgedTokensAndCall",
    ],
};

static GNOSIS_MEDIATOR_V8_GRAMMAR: MediatorGrammar = MediatorGrammar {
    version: MediatorVersion::GnosisV8,
    events: ETH_MEDIATOR_V6_GRAMMAR.events,
    functions: ETH_MEDIATOR_V6_GRAMMAR.functions,
};

pub(crate) fn amb_grammar_for(version: i16) -> Result<&'static AmbGrammar> {
    match version {
        6 => Ok(&AMB_V6_GRAMMAR),
        _ => bail!("no AMB grammar registered for version {version}"),
    }
}

pub(crate) fn mediator_grammar_for(
    chain_id: i64,
    version: i16,
) -> Result<&'static MediatorGrammar> {
    match (chain_id, version) {
        (1, 6) => Ok(&ETH_MEDIATOR_V6_GRAMMAR),
        (100, 8) => Ok(&GNOSIS_MEDIATOR_V8_GRAMMAR),
        _ => bail!(
            "no Omnibridge mediator grammar registered for chain {chain_id} version {version}"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grammar_lookup_known_versions() {
        assert_eq!(amb_grammar_for(6).unwrap().version, AmbVersion::V6);
        assert_eq!(
            mediator_grammar_for(1, 6).unwrap().version,
            MediatorVersion::EthV6
        );
        assert_eq!(
            mediator_grammar_for(100, 8).unwrap().version,
            MediatorVersion::GnosisV8
        );
    }

    #[test]
    fn test_grammar_lookup_unknown_versions_returns_error() {
        assert!(amb_grammar_for(5).is_err());
        assert!(mediator_grammar_for(100, 6).is_err());
    }
}
