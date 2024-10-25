use crate::{
    blockscout::BlockscoutClient,
    protocols::{DomainNameOnProtocol, ProtocolError},
};
use alloy::primitives::{Address, B256};
use anyhow::anyhow;
use nonempty::{nonempty, NonEmpty};
use sea_query::{Alias, IntoTableRef, TableRef};
use serde::{Deserialize, Deserializer, Serialize};
use std::{collections::HashMap, sync::Arc};
use url::Url;

#[derive(Debug, Clone)]
pub struct Protocoler {
    networks: HashMap<i64, Network>,
    protocols: HashMap<String, Protocol>,
}

#[derive(Debug, Clone)]
pub struct Network {
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
pub struct ProtocolInfo {
    pub network_id: i64,
    pub slug: String,
    pub tld_list: NonEmpty<Tld>,
    pub subgraph_name: String,
    pub address_resolve_technique: AddressResolveTechnique,
    pub empty_label_hash: Option<B256>,
    pub native_token_contract: Option<Address>,
    pub registry_contract: Option<Address>,
    pub meta: ProtocolMeta,
    pub offchain_strategy: OffchainStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OffchainStrategy {
    #[default]
    None,
    EnsWildcard,
    D3Connect,
}

impl OffchainStrategy {
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    pub fn is_some(&self) -> bool {
        !self.is_none()
    }
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
pub struct Tld(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AddressResolveTechnique {
    #[default]
    ReverseRegistry,
    AllDomains,
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

    pub fn reverse() -> Self {
        Self("reverse".to_string())
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
        networks: HashMap<i64, Network>,
        protocols: HashMap<String, Protocol>,
    ) -> Result<Self, anyhow::Error> {
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

    pub fn protocol_by_slug(&self, slug: &str) -> Option<DeployedProtocol> {
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

    pub fn main_protocol_of_network(
        &self,
        network_id: i64,
        maybe_filter: Option<NonEmpty<String>>,
    ) -> Result<DeployedProtocol<'_>, ProtocolError> {
        self.protocols_of_network(network_id, maybe_filter)
            .map(|p| p.head)
    }

    pub fn protocols_of_network_for_tld(
        &self,
        network_id: i64,
        tld: Tld,
        maybe_filter: Option<NonEmpty<String>>,
    ) -> Result<NonEmpty<DeployedProtocol<'_>>, ProtocolError> {
        let net_protocols = self.protocols_of_network(network_id, maybe_filter)?;
        let protocols = net_protocols
            .iter()
            .filter(|p| p.protocol.info.tld_list.contains(&tld))
            .cloned()
            .collect::<Vec<DeployedProtocol>>();
        let protocols =
            NonEmpty::from_vec(protocols).unwrap_or_else(|| nonempty![net_protocols.head]);
        Ok(protocols)
    }

    pub fn names_options_in_network(
        &self,
        name: &str,
        network_id: i64,
        maybe_filter: Option<NonEmpty<String>>,
    ) -> Result<Vec<DomainNameOnProtocol>, ProtocolError> {
        let tld = Tld::from_domain_name(name).ok_or_else(|| ProtocolError::InvalidName {
            name: name.to_string(),
            reason: "no tld found".to_string(),
        })?;
        let protocols = self.protocols_of_network_for_tld(network_id, tld, maybe_filter)?;
        let names_with_protocols = protocols
            .into_iter()
            .map(|p| DomainNameOnProtocol::from_str(name, p))
            .collect::<Result<_, _>>()?;
        Ok(names_with_protocols)
    }

    pub fn main_name_in_network(
        &self,
        name: &str,
        network_id: i64,
        maybe_filter: Option<NonEmpty<String>>,
    ) -> Result<DomainNameOnProtocol, ProtocolError> {
        let maybe_name = self
            .names_options_in_network(name, network_id, maybe_filter)
            .map(|mut names| names.pop())?;
        let name = maybe_name.ok_or_else(|| ProtocolError::InvalidName {
            name: name.to_string(),
            reason: "no protocol found".to_string(),
        })?;
        Ok(name)
    }

    pub fn name_in_protocol(
        &self,
        name: &str,
        network_id: i64,
        protocol_id: &str,
        maybe_filter: Option<NonEmpty<String>>,
    ) -> Result<DomainNameOnProtocol, ProtocolError> {
        let names = self.names_options_in_network(name, network_id, maybe_filter)?;
        let name = names
            .into_iter()
            .find(|name| name.deployed_protocol.protocol.info.slug == protocol_id)
            .ok_or_else(|| ProtocolError::ProtocolNotFound(protocol_id.to_string()))?;
        Ok(name)
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
