use crate::{
    blockscout::BlockscoutClient,
    protocols::{CleanName, DomainNameOnProtocol, ProtocolError},
};
use alloy::primitives::{Address, B256};
use anyhow::anyhow;
use nonempty::{nonempty, NonEmpty};
use sea_query::{Alias, IntoTableRef, TableRef};
use serde::{Deserialize, Deserializer, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    sync::Arc,
};
use url::Url;

const MAX_NETWORKS_LIMIT: usize = 5;
const MAX_PROTOCOLS_FROM_USER_INPUT: usize = 5;

#[derive(Debug, Clone)]
pub struct Protocoler {
    networks: BTreeMap<i64, Network>,
    protocols: BTreeMap<String, Protocol>,
}

#[derive(Debug, Clone)]
pub struct Network {
    pub network_id: i64,
    pub blockscout_client: Arc<BlockscoutClient>,
    pub use_protocols: Vec<String>,
    pub rpc_url: Option<Url>,
}

impl Network {
    pub fn rpc_url(&self) -> Url {
        self.rpc_url.as_ref().cloned().unwrap_or_else(|| {
            self.blockscout_client
                .as_ref()
                .url()
                .join("/api/eth-rpc")
                .expect("valid url")
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Protocol {
    pub info: ProtocolInfo,
    pub subgraph_schema: String,
}

#[derive(Debug, Clone, Copy)]
pub struct DeployedProtocol<'a> {
    pub protocol: &'a Protocol,
    pub deployment_network: &'a Network,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ProtocolInfo {
    pub network_id: i64,
    pub slug: String,
    pub tld_list: NonEmpty<Tld>,
    pub subgraph_name: String,
    pub address_resolve_technique: AddressResolveTechnique,
    pub meta: ProtocolMeta,
    pub protocol_specific: ProtocolSpecific,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type")]
pub enum ProtocolSpecific {
    EnsLike(EnsLikeProtocol),
    D3Connect(D3ConnectProtocol),
    InfinityName(InfinityNameProtocol),
}

impl Default for ProtocolSpecific {
    fn default() -> Self {
        Self::EnsLike(Default::default())
    }
}

impl ProtocolSpecific {
    pub fn try_offchain_resolve(&self) -> bool {
        match self {
            ProtocolSpecific::EnsLike(ens) => ens.try_offchain_resolve,
            ProtocolSpecific::D3Connect(d3) => !d3.disable_offchain_resolve,
            ProtocolSpecific::InfinityName(_) => false,
        }
    }

    pub fn empty_label_hash(&self) -> Option<B256> {
        match self {
            ProtocolSpecific::EnsLike(ens_like) => ens_like.empty_label_hash,
            ProtocolSpecific::D3Connect(_) => None,
            ProtocolSpecific::InfinityName(_) => None,
        }
    }

    pub fn native_token_contract(&self) -> Option<Address> {
        match self {
            ProtocolSpecific::EnsLike(ens) => ens.native_token_contract,
            ProtocolSpecific::D3Connect(d3) => Some(d3.native_token_contract),
            ProtocolSpecific::InfinityName(infinity) => Some(infinity.main_contract),
        }
    }

    // pub fn registry_contract(&self) -> Option<Address> {
    //     match self {
    //         ProtocolSpecific::EnsLike(ens) => ens.registry_contract,
    //         ProtocolSpecific::D3Connect(_) => None,
    //         ProtocolSpecific::InfinityName(_) => None,
    //     }
    // }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct EnsLikeProtocol {
    pub registry_contract: Option<Address>,
    pub empty_label_hash: Option<B256>,
    pub native_token_contract: Option<Address>,
    #[serde(default)]
    pub try_offchain_resolve: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct D3ConnectProtocol {
    pub resolver_contract: Address,
    pub native_token_contract: Address,
    #[serde(default)]
    pub disable_offchain_resolve: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct InfinityNameProtocol {
    pub main_contract: Address,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ProtocolMeta {
    pub short_name: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub docs_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialOrd, Ord, PartialEq, Eq, Default, Hash)]
pub struct Tld(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AddressResolveTechnique {
    #[default]
    ReverseRegistry,
    AllDomains,
    #[serde(rename = "addr2name")]
    Addr2Name,
    #[serde(rename = "primary_name_record")]
    PrimaryNameRecord,
}

impl Tld {
    pub fn new(tld: &str) -> Tld {
        Self(tld.trim_start_matches('.').to_string())
    }

    pub fn from_domain_name(name: &str) -> Option<Tld> {
        name.rsplit('.')
            .next()
            .filter(|c| !c.is_empty())
            .map(Self::new)
    }
}

impl<'de> Deserialize<'de> for Tld {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer).map(|s| Self::new(&s))
    }
}

impl Protocoler {
    pub fn initialize(
        networks: impl IntoIterator<Item = (i64, Network)>,
        protocols: impl IntoIterator<Item = (String, Protocol)>,
    ) -> Result<Self, anyhow::Error> {
        let networks = networks.into_iter().collect::<BTreeMap<_, _>>();
        let protocols = protocols.into_iter().collect::<BTreeMap<_, _>>();

        for (id, network) in networks.iter() {
            if let Some(name) = network
                .use_protocols
                .iter()
                .find(|&name| !protocols.contains_key(name))
            {
                return Err(anyhow!("unknown protocol '{name}' in network '{id}'",));
            }
        }

        for protocol in protocols.values() {
            let network_id = protocol.info.network_id;
            if !networks.contains_key(&network_id) {
                return Err(anyhow!(
                    "unknown network id '{network_id}' for protocol '{}'",
                    protocol.info.slug
                ));
            }
        }

        Ok(Self {
            networks,
            protocols,
        })
    }

    pub fn iter_protocols(&self) -> impl Iterator<Item = &Protocol> {
        self.protocols.values()
    }

    pub fn iter_deployed_protocols(&self) -> impl Iterator<Item = DeployedProtocol<'_>> {
        self.iter_protocols()
            .filter_map(|protocol| protocol.deployed_on_network(self))
    }

    pub fn protocol_by_slug(&'_ self, slug: &str) -> Option<DeployedProtocol<'_>> {
        self.protocols.get(slug).map(|protocol| {
            protocol
                .deployed_on_network(self)
                .expect("protocoler should be correctly initialized")
        })
    }

    pub fn protocols_of_network(
        &self,
        network_id: i64,
        maybe_filter: Option<NonEmpty<String>>,
    ) -> Result<NonEmpty<DeployedProtocol<'_>>, ProtocolError> {
        let network = self
            .networks
            .get(&network_id)
            .ok_or_else(|| ProtocolError::NetworkNotFound(network_id))?;
        let net_protocols = network
            .use_protocols
            .iter()
            .map(|name| {
                let protocol = self
                    .protocols
                    .get(name)
                    .expect("protocol should be in the map");
                protocol
                    .deployed_on_network(self)
                    .expect("protocoler should be correctly initialized")
            })
            .collect::<Vec<_>>();
        let net_protocols = if let Some(filter) = maybe_filter {
            filter
                .into_iter()
                .map(|f| {
                    net_protocols
                        .iter()
                        .find(|&p| p.protocol.info.slug == f)
                        .copied()
                        .ok_or_else(|| ProtocolError::ProtocolNotFound(f))
                })
                .collect::<Result<Vec<_>, _>>()?
        } else {
            net_protocols
        };
        let net_protocols = NonEmpty::from_vec(net_protocols).ok_or_else(|| {
            ProtocolError::ProtocolNotFound("no protocols found for network".to_string())
        })?;
        Ok(net_protocols)
    }

    pub fn choose_protocol_for_network_by_domain_name(
        &self,
        network_id: i64,
        clean: &CleanName,
        maybe_filter: Option<NonEmpty<String>>,
    ) -> Result<DeployedProtocol<'_>, ProtocolError> {
        let protocols =
            self.protocols_of_network_for_tld_with_filter(network_id, clean.tld(), maybe_filter)?;
        // choose protocol with highest priority
        Ok(protocols.head)
    }

    pub fn protocols_of_network_for_tld_with_filter(
        &self,
        network_id: i64,
        tld: &Tld,
        maybe_filter: Option<NonEmpty<String>>,
    ) -> Result<NonEmpty<DeployedProtocol<'_>>, ProtocolError> {
        let net_protocols = self.protocols_of_network(network_id, maybe_filter)?;
        let protocols = net_protocols
            .iter()
            .filter(|p| p.protocol.info.tld_list.contains(tld))
            .cloned()
            .collect::<Vec<DeployedProtocol>>();
        let protocols =
            NonEmpty::from_vec(protocols).unwrap_or_else(|| nonempty![net_protocols.head]);
        Ok(protocols)
    }

    pub fn deployed_protocols_from_user_input(
        &self,
        network_id: Option<i64>,
        protocols: Option<NonEmpty<String>>,
    ) -> Result<NonEmpty<DeployedProtocol<'_>>, ProtocolError> {
        if let Some(network_id) = network_id {
            // If network id is provided, this is a network-specific request and provided protocols are filters
            let maybe_filter = protocols;
            return self.protocols_of_network(network_id, maybe_filter);
        }
        if let Some(protocols) = protocols {
            // If protocols are provided, this is a protocol-specific request and we just need to search for matching protocols
            let deduped = protocols.into_iter().collect::<HashSet<_>>();
            let protocols = self.protocols_by_slugs(deduped)?;
            if let Some(protocols) = NonEmpty::from_vec(protocols) {
                if protocols.len() > MAX_PROTOCOLS_FROM_USER_INPUT {
                    return Err(ProtocolError::TooManyProtocols {
                        specified: protocols.len(),
                        max: MAX_PROTOCOLS_FROM_USER_INPUT,
                    });
                }
                return Ok(protocols);
            }
        };

        // otherwise, this is request without network or protocols, so we need to search for protocols on mainnet
        if let Some(protocol) = self.protocol_on_mainnet() {
            return Ok(nonempty![protocol]);
        }

        Err(ProtocolError::ProtocolNotFound(
            "no protocols found for network or protocols".to_string(),
        ))
    }

    pub fn protocols_from_user_input(
        &self,
        network_id: Option<i64>,
        protocols: Option<NonEmpty<String>>,
    ) -> Result<NonEmpty<&Protocol>, ProtocolError> {
        self.deployed_protocols_from_user_input(network_id, protocols)
            .map(|nonempty| nonempty.map(|p| p.protocol))
    }

    pub fn deployed_protocol_from_user_input(
        &self,
        network_id: Option<i64>,
        protocol_id: Option<String>,
    ) -> Result<DeployedProtocol<'_>, ProtocolError> {
        match (network_id, protocol_id) {
            (Some(network_id), Some(protocol_id)) => self
                .protocols_of_network(network_id, Some(nonempty![protocol_id]))
                .map(|p| p.head),
            (Some(network_id), None) => self.protocols_of_network(network_id, None).map(|p| p.head),
            (None, Some(protocol_id)) => self
                .protocol_by_slug(&protocol_id)
                .ok_or_else(|| ProtocolError::ProtocolNotFound(protocol_id)),
            (None, None) => self.protocol_on_mainnet().ok_or_else(|| {
                ProtocolError::ProtocolNotFound(
                    "no default protocol found, either specify network_id or protocol_id"
                        .to_string(),
                )
            }),
        }
    }

    pub fn protocol_on_mainnet(&self) -> Option<DeployedProtocol<'_>> {
        self.iter_deployed_protocols()
            .find(|p| p.protocol.info.network_id == 1)
    }

    fn protocols_by_slugs(
        &self,
        protocols: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Vec<DeployedProtocol<'_>>, ProtocolError> {
        protocols
            .into_iter()
            .map(|name| {
                let name = name.as_ref();
                self.protocol_by_slug(name)
                    .ok_or_else(|| ProtocolError::ProtocolNotFound(name.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()
    }

    pub fn get_domain_options<'a>(
        &'_ self,
        clean: &CleanName,
        protocols: NonEmpty<DeployedProtocol<'a>>,
    ) -> Result<NonEmpty<DomainNameOnProtocol<'a>>, ProtocolError> {
        let supported_tlds: BTreeSet<Tld> = protocols
            .iter()
            .flat_map(|p| p.protocol.info.tld_list.iter().cloned())
            .collect();

        match clean.level() {
            1 => {
                // level is 1, so this is just string without .
                self.get_domain_options_append_tld(clean, protocols, supported_tlds)
            }
            level if level > 1 => {
                let mut options =
                    self.get_domain_options_for_name_with_tld(clean, protocols.clone())?;
                // if domain.tld is not in supported_tlds, then this is a subdomain, so we need to append the TLD to the name
                if !supported_tlds.contains(clean.tld()) {
                    let additional_options =
                        self.get_domain_options_append_tld(clean, protocols, supported_tlds)?;
                    options.extend(additional_options);
                }
                Ok(options)
            }
            _ => {
                // should be unreachable
                tracing::warn!(
                    "unexpected domain level ({}) for name '{}'",
                    clean.level(),
                    clean.name()
                );
                Err(ProtocolError::InvalidName {
                    name: clean.name().to_string(),
                    reason: "Invalid domain level".to_string(),
                })
            }
        }
    }

    fn get_domain_options_append_tld<'a>(
        &'_ self,
        clean: &CleanName,
        protocols: NonEmpty<DeployedProtocol<'a>>,
        supported_tlds: impl IntoIterator<Item = Tld>,
    ) -> Result<NonEmpty<DomainNameOnProtocol<'a>>, ProtocolError> {
        let domain_options = supported_tlds
            .into_iter()
            .filter_map(|tld| {
                let name_with_tld = clean.clone().append_tld(tld);
                self.get_domain_options_for_name_with_tld(&name_with_tld, protocols.clone())
                    .ok()
            })
            .flat_map(|non_empty| non_empty.into_iter())
            .take(MAX_NETWORKS_LIMIT);

        NonEmpty::collect(domain_options).ok_or_else(|| ProtocolError::InvalidName {
            name: clean.name().to_string(),
            reason: "No valid TLDs".to_string(),
        })
    }

    fn get_domain_options_for_name_with_tld<'a>(
        &'_ self,
        clean: &CleanName,
        protocols: NonEmpty<DeployedProtocol<'a>>,
    ) -> Result<NonEmpty<DomainNameOnProtocol<'a>>, ProtocolError> {
        let domain_names = protocols
            .into_iter()
            .map(|p| DomainNameOnProtocol::from_str(clean.name(), p))
            .collect::<Result<Vec<_>, _>>()?;

        NonEmpty::from_vec(domain_names).ok_or_else(|| ProtocolError::InvalidName {
            name: clean.name().to_string(),
            reason: "no matching protocols for given TLD".to_string(),
        })
    }
}

impl Protocol {
    pub fn subgraph_table(&self, table: &str) -> TableRef {
        (Alias::new(&self.subgraph_schema), Alias::new(table)).into_table_ref()
    }

    pub fn deployed_on_network<'a>(
        &'a self,
        protocoler: &'a Protocoler,
    ) -> Option<DeployedProtocol<'a>> {
        protocoler
            .networks
            .get(&self.info.network_id)
            .map(|network| DeployedProtocol {
                protocol: self,
                deployment_network: network,
            })
    }
}

#[cfg(test)]
mod tld_tests {
    use super::Tld;

    #[test]
    fn tld_new_trims_dot() {
        let tld = Tld::new(".eth");
        assert_eq!(tld.0, "eth");
    }

    #[test]
    fn tld_new_no_dot() {
        let tld = Tld::new("eth");
        assert_eq!(tld.0, "eth");
    }

    #[test]
    fn from_domain_name_works() {
        let domain = "vitalik.eth";
        let tld = Tld::from_domain_name(domain).unwrap();
        assert_eq!(tld.0, "eth");

        let domain = "abcnews.gno";
        let tld = Tld::from_domain_name(domain).unwrap();
        assert_eq!(tld.0, "gno");
    }

    #[test]
    fn from_domain_name_empty() {
        let domain = ".";
        let tld = Tld::from_domain_name(domain);
        assert!(tld.is_none());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockscout::BlockscoutClient;
    use pretty_assertions::assert_eq;
    use rstest::*;

    #[fixture]
    fn blockscout_client() -> Arc<BlockscoutClient> {
        Arc::new(BlockscoutClient::new(
            "http://localhost:8545".parse().unwrap(),
            1,
            30,
        ))
    }

    #[fixture]
    fn protocoler(blockscout_client: Arc<BlockscoutClient>) -> Protocoler {
        let networks = [
            (
                1,
                Network {
                    network_id: 1,
                    blockscout_client: blockscout_client.clone(),
                    use_protocols: vec!["ens".to_string()],
                    rpc_url: None,
                },
            ),
            (
                100,
                Network {
                    network_id: 100,
                    blockscout_client: blockscout_client.clone(),
                    use_protocols: vec!["gnosis".to_string()],
                    rpc_url: None,
                },
            ),
            (
                1337,
                Network {
                    network_id: 1337,
                    blockscout_client: blockscout_client.clone(),
                    use_protocols: vec!["ens".to_string(), "gnosis".to_string()],
                    rpc_url: None,
                },
            ),
        ];

        let protocols = [
            (
                "ens".to_string(),
                Protocol {
                    info: ProtocolInfo {
                        network_id: 1,
                        slug: "ens".to_string(),
                        tld_list: nonempty![Tld::new("eth")],
                        subgraph_name: "ens-subgraph".to_string(),
                        ..Default::default()
                    },
                    subgraph_schema: "ens".to_string(),
                },
            ),
            (
                "gnosis".to_string(),
                Protocol {
                    info: ProtocolInfo {
                        network_id: 100,
                        slug: "gnosis".to_string(),
                        tld_list: nonempty![Tld::new("gno")],
                        subgraph_name: "gnosis-subgraph".to_string(),
                        ..Default::default()
                    },
                    subgraph_schema: "gnosis".to_string(),
                },
            ),
        ];

        Protocoler::initialize(networks, protocols).expect("should initialize successfully")
    }

    #[rstest]
    #[case(1, None, 1, "ens")]
    #[case(1337, Some(nonempty!["ens".to_string()]), 1, "ens")]
    fn test_protocols_of_network_success(
        protocoler: Protocoler,
        #[case] network_id: i64,
        #[case] filter: Option<NonEmpty<String>>,
        #[case] expected_len: usize,
        #[case] expected_slug: &str,
    ) {
        let result = protocoler.protocols_of_network(network_id, filter);
        assert!(result.is_ok());
        let protocols = result.unwrap();
        assert_eq!(protocols.len(), expected_len);
        assert_eq!(protocols.head.protocol.info.slug, expected_slug);
    }

    #[rstest]
    #[case(999, None, "NetworkNotFound", 999)]
    #[case(1, Some(nonempty!["nonexistent".to_string()]), "ProtocolNotFound", 0)]
    fn test_protocols_of_network_errors(
        protocoler: Protocoler,
        #[case] network_id: i64,
        #[case] filter: Option<NonEmpty<String>>,
        #[case] error_type: &str,
        #[case] error_value: i64,
    ) {
        let result = protocoler.protocols_of_network(network_id, filter);
        assert!(result.is_err());
        match (result.unwrap_err(), error_type) {
            (ProtocolError::NetworkNotFound(id), "NetworkNotFound") => {
                assert_eq!(id, error_value);
            }
            (ProtocolError::ProtocolNotFound(name), "ProtocolNotFound") => {
                assert_eq!(name, "nonexistent");
            }
            _ => panic!("Unexpected error type"),
        }
    }

    // #[rstest]
    // #[case(vec![1, 100], 2, vec!["ens", "gnosis"])]
    // #[case(vec![], 0, vec![])]
    // fn test_protocols_of_networks_success(
    //     protocoler: Protocoler,
    //     #[case] network_ids: Vec<i64>,
    //     #[case] expected_len: usize,
    //     #[case] expected_slugs: Vec<&str>,
    // ) {
    //     let result = protocoler.protocols_of_networks(network_ids);
    //     assert!(result.is_ok());
    //     let protocols = result.unwrap();
    //     assert_eq!(protocols.len(), expected_len);
    //     let slugs: Vec<&str> = protocols
    //         .iter()
    //         .map(|p| p.protocol.info.slug.as_str())
    //         .collect();
    //     for expected_slug in expected_slugs {
    //         assert!(slugs.contains(&expected_slug));
    //     }
    // }

    // #[rstest]
    // fn test_protocols_of_networks_with_invalid_network(protocoler: Protocoler) {
    //     let result = protocoler.protocols_of_networks(vec![1, 999]);
    //     assert!(result.is_err());
    //     match result.unwrap_err() {
    //         ProtocolError::NetworkNotFound(id) => assert_eq!(id, 999),
    //         _ => panic!("Expected NetworkNotFound error"),
    //     }
    // }

    // #[rstest]
    // #[case("eth", 1, "ens")]
    // #[case("gno", 1, "gnosis")]
    // #[case("nonexistent", 0, "")]
    // fn test_protocols_for_tld(
    //     protocoler: Protocoler,
    //     #[case] tld_str: &str,
    //     #[case] expected_len: usize,
    //     #[case] expected_slug: &str,
    // ) {
    //     let tld = Tld::new(tld_str);
    //     let protocols = protocoler.protocols_for_tld(tld);
    //     assert_eq!(protocols.len(), expected_len);
    //     if expected_len > 0 {
    //         assert_eq!(protocols[0].protocol.info.slug, expected_slug);
    //     }
    // }

    #[rstest]
    #[case(Some(1), None, vec!["ens"])]
    #[case(None, Some(vec!["ens".to_string()]), vec!["ens"])]
    #[case(Some(1337), None, vec!["ens", "gnosis"])]
    #[case(Some(1337), Some(vec!["ens".to_string()]), vec!["ens"])]
    #[case(None, None, vec!["ens"])] // fallback to mainnet
    fn test_protocols_of_user_input_success(
        protocoler: Protocoler,
        #[case] network_id: Option<i64>,
        #[case] protocols_input: Option<Vec<String>>,
        #[case] expected_slugs: Vec<&str>,
    ) {
        let protocols_input = protocols_input.map(|slugs| NonEmpty::from_vec(slugs).unwrap());
        let result = protocoler.deployed_protocols_from_user_input(network_id, protocols_input);
        assert!(result.is_ok());
        let protocols = result.unwrap();
        let actual_slugs: Vec<&str> = protocols
            .iter()
            .map(|p| p.protocol.info.slug.as_str())
            .collect();
        assert_eq!(actual_slugs, expected_slugs);
    }

    #[rstest]
    fn test_protocols_of_user_input_invalid_protocol(protocoler: Protocoler) {
        let protocols_input = nonempty!["nonexistent".to_string()];
        let result = protocoler.deployed_protocols_from_user_input(None, Some(protocols_input));
        assert!(result.is_err());
        match result.unwrap_err() {
            ProtocolError::ProtocolNotFound(name) => assert_eq!(name, "nonexistent"),
            _ => panic!("Expected ProtocolNotFound error"),
        }
    }

    #[rstest]
    #[case(vec!["ens", "gnosis"], vec!["ens", "gnosis"])]
    #[case(vec!["ens"], vec!["ens"])]
    #[case(vec![], vec![])]
    fn test_protocols_by_slugs_success(
        protocoler: Protocoler,
        #[case] slugs: Vec<&str>,
        #[case] expected_slugs: Vec<&str>,
    ) {
        let result = protocoler.protocols_by_slugs(slugs);
        assert!(result.is_ok());
        let protocols = result.unwrap();
        assert_eq!(protocols.len(), expected_slugs.len());
        let actual_slugs: Vec<&str> = protocols
            .iter()
            .map(|p| p.protocol.info.slug.as_str())
            .collect();
        assert_eq!(actual_slugs, expected_slugs);
    }

    #[rstest]
    fn test_protocols_by_slugs_not_found(protocoler: Protocoler) {
        let result = protocoler.protocols_by_slugs(vec!["nonexistent"]);
        assert!(result.is_err());
        match result.unwrap_err() {
            ProtocolError::ProtocolNotFound(name) => assert_eq!(name, "nonexistent"),
            _ => panic!("Expected ProtocolNotFound error"),
        }
    }

    #[rstest]
    #[case("vitalik.eth", 1, Ok(vec![("vitalik.eth", "ens")]))]
    #[case("vitalik", 1, Ok(vec![("vitalik.eth", "ens")]))]
    #[case("subdomain.vitalik", 1, Ok(vec![("subdomain.vitalik", "ens"), ("subdomain.vitalik.eth", "ens")]))]
    #[case("a.b", 1337, Ok(vec![
        ("a.b", "ens"),
        ("a.b", "gnosis"),
        ("a.b.eth", "ens"),
        ("a.b.eth", "gnosis"),
        ("a.b.gno", "ens"),
        ("a.b.gno", "gnosis"),
    ]))]
    fn test_get_names_options(
        protocoler: Protocoler,
        #[case] name: &str,
        #[case] network_id: i64,
        #[case] expected: Result<Vec<(&str, &str)>, String>,
    ) {
        let protocols = protocoler
            .protocols_of_network(network_id, None)
            .expect("should get protocols");
        let name = CleanName::new(name).expect("should be valid");
        let result = protocoler.get_domain_options(&name, protocols);
        match expected {
            Ok(expected) => {
                let options =
                    result.unwrap_or_else(|error| panic!("expected Ok, got Err: {error:?}"));
                let expected = expected
                    .into_iter()
                    .map(|(name, slug)| (name.to_string(), slug.to_string()))
                    .collect::<Vec<_>>();
                assert_eq!(options.len(), expected.len());
                assert_eq!(
                    options
                        .into_iter()
                        .map(|o| (
                            o.inner.name().to_string(),
                            o.deployed_protocol.protocol.info.slug.clone()
                        ))
                        .collect::<Vec<_>>(),
                    expected
                );
            }
            Err(s) => {
                let error = result.unwrap_err();
                assert!(
                    error.to_string().contains(&s),
                    "expected error to contain '{s}', got '{error}'"
                );
            }
        }
    }
}
