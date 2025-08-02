use crate::{
    error::ServiceError,
    repository::{
        address_token_balances::{self, ListAddressTokensPageToken},
        addresses, chains, interop_message_transfers, interop_messages,
    },
    types::{
        ChainId,
        address_token_balances::ExtendedAddressTokenBalance,
        addresses::AddressInfo,
        chains::Chain,
        interop_messages::{InteropMessage, MessageDirection},
        tokens::TokenType,
    },
};
use alloy_primitives::{Address as AddressAlloy, TxHash};
use sea_orm::{DatabaseConnection, prelude::DateTime};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct Cluster {
    chain_ids: HashSet<ChainId>,
}

impl Cluster {
    pub fn new(chain_ids: HashSet<ChainId>) -> Self {
        Self { chain_ids }
    }

    pub fn validate_chain_id(&self, chain_id: ChainId) -> Result<(), ServiceError> {
        if !self.chain_ids.contains(&chain_id) {
            return Err(ServiceError::InvalidClusterChainId(chain_id));
        }
        Ok(())
    }

    pub fn chain_ids(&self) -> Vec<ChainId> {
        self.chain_ids.iter().cloned().collect()
    }

    pub async fn list_chains(&self, db: &DatabaseConnection) -> Result<Vec<Chain>, ServiceError> {
        let chains = chains::list_by_ids(db, self.chain_ids()).await?;
        Ok(chains.into_iter().map(|c| c.into()).collect())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn list_interop_messages(
        &self,
        db: &DatabaseConnection,
        init_chain_id: Option<ChainId>,
        relay_chain_id: Option<ChainId>,
        address: Option<AddressAlloy>,
        direction: Option<MessageDirection>,
        nonce: Option<i64>,
        page_size: u64,
        page_token: Option<(DateTime, TxHash)>,
    ) -> Result<(Vec<InteropMessage>, Option<(DateTime, TxHash)>), ServiceError> {
        if let Some(init_chain_id) = init_chain_id {
            self.validate_chain_id(init_chain_id)?;
        }
        if let Some(relay_chain_id) = relay_chain_id {
            self.validate_chain_id(relay_chain_id)?;
        }

        let cluster_chain_ids = self.chain_ids();
        let (interop_messages, next_page_token) = interop_messages::list(
            db,
            init_chain_id,
            relay_chain_id,
            address,
            direction,
            nonce,
            Some(cluster_chain_ids),
            page_size,
            page_token,
        )
        .await?;

        Ok((
            interop_messages
                .into_iter()
                .map(InteropMessage::try_from)
                .collect::<Result<Vec<_>, _>>()?,
            next_page_token,
        ))
    }

    pub async fn count_interop_messages(
        &self,
        db: &DatabaseConnection,
        chain_id: ChainId,
    ) -> Result<u64, ServiceError> {
        self.validate_chain_id(chain_id)?;

        let cluster_chain_ids = self.chain_ids();
        let count = interop_messages::count(db, chain_id, Some(cluster_chain_ids)).await?;
        Ok(count)
    }

    pub async fn get_address_info(
        &self,
        db: &DatabaseConnection,
        address: AddressAlloy,
    ) -> Result<AddressInfo, ServiceError> {
        let cluster_chain_ids = self.chain_ids();
        let mut address_info =
            addresses::get_address_info(db, address, Some(cluster_chain_ids.clone()))
                .await?
                .unwrap_or_else(|| AddressInfo::default(address.to_vec()));

        let (has_tokens, has_interop_message_transfers) = futures::join!(
            address_token_balances::check_if_tokens_at_address(
                db,
                address,
                cluster_chain_ids.clone()
            ),
            interop_message_transfers::check_if_interop_message_transfers_at_address(
                db,
                address,
                cluster_chain_ids,
            )
        );

        address_info.has_tokens = has_tokens?;
        address_info.has_interop_message_transfers = has_interop_message_transfers?;

        Ok(address_info)
    }

    pub async fn list_address_tokens(
        &self,
        db: &DatabaseConnection,
        address: AddressAlloy,
        token_type: Option<TokenType>,
        page_size: u64,
        page_token: Option<ListAddressTokensPageToken>,
    ) -> Result<
        (
            Vec<ExtendedAddressTokenBalance>,
            Option<ListAddressTokensPageToken>,
        ),
        ServiceError,
    > {
        let cluster_chain_ids = self.chain_ids();

        let res = address_token_balances::list_by_address(
            db,
            address,
            token_type,
            cluster_chain_ids,
            page_size,
            page_token,
        )
        .await?;

        Ok(res)
    }
}
