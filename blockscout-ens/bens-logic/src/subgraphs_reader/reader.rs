use super::{
    blockscout::{self, BlockscoutClient},
    schema_selector::schema_names,
    sql,
};
use crate::{
    entity::subgraph::{
        domain::Domain,
        domain_event::{DomainEvent, DomainEventTransaction},
    },
    hash_name::hash_ens_domain_name,
};
use ethers::types::{Address, TxHash};
use sqlx::postgres::PgPool;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct NetworkConfig {
    schema_name: String,
    blockscout_client: Arc<BlockscoutClient>,
}

pub struct SubgraphReader {
    pool: Arc<PgPool>,
    networks: HashMap<i64, NetworkConfig>,
}

impl SubgraphReader {
    pub async fn initialize(
        pool: Arc<PgPool>,
        mut blockscout_clients: HashMap<i64, BlockscoutClient>,
    ) -> Result<Self, anyhow::Error> {
        let schema_names = schema_names(&pool).await?;
        tracing::info!(schema_names =? schema_names, "found subgraph schemas");
        let networks = schema_names
            .into_iter()
            .filter_map(|(id, schema_name)| {
                let maybe_config = blockscout_clients.remove(&id).map(|blockscout_client| {
                    (
                        id,
                        NetworkConfig {
                            schema_name,
                            blockscout_client: Arc::new(blockscout_client),
                        },
                    )
                });
                if maybe_config.is_none() {
                    tracing::warn!("no blockscout url for chain {id}, skip this network")
                }
                maybe_config
            })
            .collect::<HashMap<_, _>>();
        for (id, client) in blockscout_clients {
            tracing::warn!("no chain found for blockscout url with chain_id {id} and url {}, skip this network", client.url())
        }
        tracing::info!(networks =? networks.keys().collect::<Vec<_>>(), "initialized subgraph reader");
        Ok(Self::new(pool, networks))
    }

    pub fn new(pool: Arc<PgPool>, networks: HashMap<i64, NetworkConfig>) -> Self {
        Self { pool, networks }
    }
}

#[derive(Error, Debug)]
pub enum SubgraphReadError {
    #[error("Network with id {0} not found")]
    NetworkNotFound(i64),
    #[error("Db err")]
    DbErr(#[from] sqlx::Error),
    #[error("internal error: {0}")]
    Internal(String),
}

impl SubgraphReader {
    pub async fn resolve_domain(
        &self,
        network_id: i64,
        name: &str,
        only_active: bool,
    ) -> Result<Option<Domain>, SubgraphReadError> {
        self.get_domain(network_id, &domain_id(name), only_active)
            .await
    }

    pub async fn get_domain(
        &self,
        network_id: i64,
        id: &str,
        only_active: bool,
    ) -> Result<Option<Domain>, SubgraphReadError> {
        let network = self
            .networks
            .get(&network_id)
            .ok_or_else(|| SubgraphReadError::NetworkNotFound(network_id))?;
        sql::find_domain(self.pool.as_ref(), &network.schema_name, id, only_active).await
    }

    pub async fn get_domain_history(
        &self,
        network_id: i64,
        id: &str,
    ) -> Result<Vec<DomainEvent>, SubgraphReadError> {
        let network = self
            .networks
            .get(&network_id)
            .ok_or_else(|| SubgraphReadError::NetworkNotFound(network_id))?;
        let domain_txns: Vec<DomainEventTransaction> =
            sql::find_transaction_events(self.pool.as_ref(), &network.schema_name, id).await?;
        let domain_events =
            events_from_transactions(network.blockscout_client.clone(), domain_txns).await?;
        Ok(domain_events)
    }

    pub async fn resolve_address(
        &self,
        network_id: i64,
        address: Address,
        only_active: bool,
    ) -> Result<Vec<Domain>, SubgraphReadError> {
        let network = self
            .networks
            .get(&network_id)
            .ok_or_else(|| SubgraphReadError::NetworkNotFound(network_id))?;
        let address = hex(address);
        sql::find_resolved_addresses(
            self.pool.as_ref(),
            &network.schema_name,
            &address,
            only_active,
        )
        .await
    }

    pub async fn resolve_owned_address(
        &self,
        network_id: i64,
        address: Address,
        only_active: bool,
    ) -> Result<Vec<Domain>, SubgraphReadError> {
        let network = self
            .networks
            .get(&network_id)
            .ok_or_else(|| SubgraphReadError::NetworkNotFound(network_id))?;
        let address = hex(address);
        sql::find_owned_addresses(
            self.pool.as_ref(),
            &network.schema_name,
            &address,
            only_active,
        )
        .await
    }

    pub async fn batch_search_addresses(
        &self,
        network_id: i64,
        addresses: impl IntoIterator<Item = Address>,
    ) -> Result<BTreeMap<String, String>, SubgraphReadError> {
        let network = self
            .networks
            .get(&network_id)
            .ok_or_else(|| SubgraphReadError::NetworkNotFound(network_id))?;
        // remove duplicates
        let addresses: HashSet<String> = addresses.into_iter().map(hex).collect();
        let addreses_str: Vec<&str> = addresses.iter().map(String::as_str).collect::<Vec<_>>();
        let result =
            sql::batch_search_addresses(&self.pool, &network.schema_name, &addreses_str).await?;
        let address_to_name = result
            .into_iter()
            .map(|d| (d.resolved_address, d.domain_name))
            .collect();
        Ok(address_to_name)
    }
}

async fn events_from_transactions(
    client: Arc<BlockscoutClient>,
    txns: Vec<DomainEventTransaction>,
) -> Result<Vec<DomainEvent>, SubgraphReadError> {
    let txns = txns
        .into_iter()
        .map(|t| (TxHash::from_slice(t.transaction_id.as_slice()), t))
        .collect::<HashMap<_, _>>();
    let transactions = client
        .transactions_batch(txns.keys().collect())
        .await
        .map_err(|e| SubgraphReadError::Internal(e.to_string()))?;

    let mut events: Vec<DomainEvent> = transactions
        .into_iter()
        .filter_map(|(tx_hash, t)| match t {
            blockscout::Response::Ok(t) => Some(DomainEvent {
                transaction_hash: t.hash,
                block_number: t.block,
                timestamp: t.timestamp,
                from_address: t.from.hash,
                method: t.method,
                actions: txns
                    .get(&t.hash)
                    .map(|d| d.actions.clone())
                    .unwrap_or_default(),
            }),
            e => {
                tracing::warn!(
                    "invalid response from blockscout transaction '{tx_hash:#x}' api: {e:?}"
                );
                None
            }
        })
        .collect::<Vec<_>>();
    events.sort_by_key(|event| event.block_number);
    Ok(events)
}

fn domain_id(name: &str) -> String {
    hex(hash_ens_domain_name(name))
}

fn hex<T>(data: T) -> String
where
    T: AsRef<[u8]>,
{
    format!("0x{}", hex::encode(data))
}

#[cfg(test)]
mod tests {
    use crate::subgraphs_reader::test_helpers::mocked_blockscout_clients;

    use super::*;
    use ethers::types::Address;
    use pretty_assertions::assert_eq;

    #[sqlx::test(migrations = "tests/migrations")]
    async fn get_domain_works(pool: PgPool) {
        let pool = Arc::new(pool);
        let clients = mocked_blockscout_clients().await;
        let reader = SubgraphReader::initialize(pool.clone(), clients)
            .await
            .expect("failed to init reader");

        // get vitalik domain
        let id = domain_id("vitalik.eth");
        let result = reader
            .get_domain(1, &id, false)
            .await
            .expect("failed to get vitalik domain")
            .expect("domain not found");
        assert_eq!(result.name.as_deref(), Some("vitalik.eth"));
        assert_eq!(
            result.resolved_address.as_deref(),
            Some("0xd8da6bf26964af9d7eed9e03e53415d37aa96045")
        );
        // get expired domain
        let id = domain_id("expired.eth");
        let result = reader
            .get_domain(1, &id, false)
            .await
            .expect("failed to get expired domain")
            .expect("expired domain not found");
        assert!(
            result.is_expired,
            "expired domain has is_expired=false: {:?}",
            result
        );
        // get expired domain with only_active filter
        let id = domain_id("expired.eth");
        let result = reader
            .get_domain(1, &id, true)
            .await
            .expect("failed to get expired domain");
        assert!(
            result.is_none(),
            "expired domain returned with only_active=true: {:?}",
            result
        );
    }

    #[sqlx::test(migrations = "tests/migrations")]
    async fn resolve_works(pool: PgPool) {
        let pool = Arc::new(pool);
        let clients = mocked_blockscout_clients().await;
        let reader = SubgraphReader::initialize(pool.clone(), clients)
            .await
            .expect("failed to init reader");

        let result = reader
            .resolve_address(1, addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045"), false)
            .await
            .expect("failed to get vitalik domains");
        assert_eq!(
            result.iter().map(|d| d.name.as_deref()).collect::<Vec<_>>(),
            vec![Some("vitalik.eth"), Some("sashaxyz.eth")]
        );

        let result = reader
            .resolve_owned_address(1, addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045"), false)
            .await
            .expect("failed to get vitalik domains");
        assert_eq!(
            result.iter().map(|d| d.name.as_deref()).collect::<Vec<_>>(),
            vec![Some("vitalik.eth")]
        );

        // search for expired address
        let result = reader
            .resolve_address(1, addr("0x9f7f7ddbfb8e14d1756580ba8037530da0880b99"), false)
            .await
            .expect("failed to get expired domains");
        // expired domain shoudn't be returned as resolved
        assert_eq!(
            result.iter().map(|d| d.name.as_deref()).collect::<Vec<_>>(),
            vec![Some("expired.eth")]
        );
        // search for expired address with only_active
        let result = reader
            .resolve_address(1, addr("0x9f7f7ddbfb8e14d1756580ba8037530da0880b99"), true)
            .await
            .expect("failed to get expired domains");
        // expired domain shoudn't be returned as resolved
        assert_eq!(
            result.iter().map(|d| d.name.as_deref()).collect::<Vec<_>>(),
            vec![]
        );
    }

    #[sqlx::test(migrations = "tests/migrations")]
    async fn get_domain_history_works(pool: PgPool) {
        let pool = Arc::new(pool);
        let clients = mocked_blockscout_clients().await;
        let reader = SubgraphReader::initialize(pool.clone(), clients)
            .await
            .expect("failed to init reader");
        let id = domain_id("vitalik.eth");
        let history = reader
            .get_domain_history(1, &id)
            .await
            .expect("failed to get history");

        let expected_history = vec![
            DomainEvent {
                transaction_hash: tx_hash(
                    "0xdd16deb1ea750037c3ed1cae5ca20ff9db0e664a5146e5a030137d277a9247f3",
                ),
                timestamp: "2017-06-18T08:39:14.000000Z".into(),
                from_address: addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045"),
                method: Some("finalizeAuction".into()),
                actions: vec!["new_owner".into()],
                block_number: 3891899,
            },
            DomainEvent {
                transaction_hash: tx_hash(
                    "0xea30bda97a7e9afcca208d5a648e8ec1e98b245a8884bf589dec8f4aa332fb14",
                ),
                timestamp: "2019-07-10T05:58:51.000000Z".into(),
                from_address: addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045"),
                method: Some("transferRegistrars".into()),
                actions: vec!["new_owner".into()],
                block_number: 8121770,
            },
            DomainEvent {
                transaction_hash: tx_hash(
                    "0x09922ac0caf1efcc8f68ce004f382b46732258870154d8805707a1d4b098dfd0",
                ),
                timestamp: "2019-10-29T13:47:34.000000Z".into(),
                from_address: addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045"),
                method: Some("setAddr".into()),
                actions: vec!["addr_changed".into()],
                block_number: 8834378,
            },
            DomainEvent {
                transaction_hash: tx_hash(
                    "0xc3f86218c67bee8256b74b9b65d746a40bb5318a8b57948b804dbbbc3d0d7864",
                ),
                timestamp: "2020-02-06T18:23:40.000000Z".into(),
                from_address: addr("0x0904dac3347ea47d208f3fd67402d039a3b99859"),
                method: Some("migrateAll".into()),
                actions: vec!["new_owner".into(), "new_resolver".into()],
                block_number: 9430706,
            },
            DomainEvent {
                transaction_hash: tx_hash(
                    "0x160ef4492c731ac6b59beebe1e234890cd55d4c556f8847624a0b47125fe4f84",
                ),
                timestamp: "2021-02-15T17:19:09.000000Z".into(),
                from_address: addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045"),
                method: Some("multicall".into()),
                actions: vec!["addr_changed".into()],
                block_number: 11862656,
            },
            DomainEvent {
                transaction_hash: tx_hash(
                    "0xbb13efab7f1f798f63814a4d184e903e050b38c38aa407f9294079ee7b3110c9",
                ),
                timestamp: "2021-02-15T17:19:17.000000Z".into(),
                from_address: addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045"),
                method: Some("setResolver".into()),
                actions: vec!["new_resolver".into()],
                block_number: 11862657,
            },
        ];
        assert_eq!(expected_history, history);
    }

    #[sqlx::test(migrations = "tests/migrations")]
    async fn batch_search_works(pool: PgPool) {
        let pool = Arc::new(pool);
        let clients = mocked_blockscout_clients().await;
        let reader = SubgraphReader::initialize(pool.clone(), clients)
            .await
            .expect("failed to init reader");

        let addresses = [
            // test.eth
            "0xeefb13c7d42efcc655e528da6d6f7bbcf9a2251d",
            // is was address of test.eth until 13294741 block
            "0x226159d592e2b063810a10ebf6dcbada94ed68b8",
            // vitalik.eth
            "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
            // vitalik.eth
            "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
            // expired.eth (expired domain)
            "0x9f7f7ddbfb8e14d1756580ba8037530da0880b99",
            // wa🇬🇲i.eth
            "0x9c996076a85b46061d9a70ff81f013853a86b619",
            // not in database
            "0x0000000000000000000000000000000000000000",
        ]
        .into_iter()
        .map(addr);
        let expected_domains = serde_json::from_value(serde_json::json!({
            "0x9c996076a85b46061d9a70ff81f013853a86b619": "wa🇬🇲i.eth",
            "0xd8da6bf26964af9d7eed9e03e53415d37aa96045": "vitalik.eth",
            "0xeefb13c7d42efcc655e528da6d6f7bbcf9a2251d": "test.eth",
        }))
        .unwrap();
        let domains = reader
            .batch_search_addresses(1, addresses)
            .await
            .expect("failed to resolve addresess");
        assert_eq!(domains, expected_domains);
    }

    fn addr(a: &str) -> Address {
        let a = a.trim_start_matches("0x");
        Address::from_slice(
            hex::decode(a)
                .expect("invalid hex provided in addr()")
                .as_slice(),
        )
    }

    fn tx_hash(h: &str) -> TxHash {
        let h = h.trim_start_matches("0x");
        TxHash::from_slice(
            hex::decode(h)
                .expect("invalid hex provided in tx_hash()")
                .as_slice(),
        )
    }
}
