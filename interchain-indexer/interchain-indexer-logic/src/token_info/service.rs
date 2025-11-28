use std::{collections::HashMap, sync::Arc};

use alloy::{
    network::Ethereum,
    primitives::Address,
    providers::DynProvider,
    sol,
};
use interchain_indexer_entity::tokens::{self, Model as TokenInfoModel};
use parking_lot::RwLock;
use sea_orm::ActiveValue::Set;

use crate::InterchainDatabase;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    ERC20,
    "src/token_info/abi/ERC20.json"
);

pub struct TokenInfoService {
    db: Arc<InterchainDatabase>,
    providers: HashMap<u64, DynProvider<Ethereum>>,

    // Local token info cache: map of (chain_id, address) to token info DB model
    // Wrapped into async RwLock to allow concurrent access.
    token_info_cache: RwLock<HashMap<(u64, Vec<u8>), TokenInfoModel>>,
}

impl TokenInfoService {
    pub fn new(db: Arc<InterchainDatabase>, providers: HashMap<u64, DynProvider<Ethereum>>) -> Self {
        Self {
            db,
            providers,
            token_info_cache: RwLock::new(HashMap::new()),
        }
    }

    pub async fn get_token_info(
        &self,
        chain_id: u64,
        address: Vec<u8>,
    ) -> anyhow::Result<TokenInfoModel> {
        let key = (chain_id, address.clone());

        // Fast path: try to read from cache
        if let Some(model) = {
            let cache = self.token_info_cache.read();
            cache.get(&key).cloned()
        } {
            return Ok(model);
        }

        // Not in cache: load from DB
        if let Some(model) = self.db.get_token_info(chain_id, key.1.clone()).await? {
            // Insert into cache
            let mut cache = self.token_info_cache.write();
            cache.entry(key.clone()).or_insert_with(|| model.clone());

            return Ok(model);
        }

        // Not in DB: fetch on-chain via RPC provider
        let provider = self
            .providers
            .get(&chain_id)
            .ok_or_else(|| anyhow::anyhow!("Provider not found for chain_id={}", chain_id))?;

        let token_contract = ERC20::new(Address::from_slice(&address), provider.clone());
        let name_call = token_contract.name();
        let symbol_call = token_contract.symbol();
        let decimals_call = token_contract.decimals();

        let (name, symbol, decimals) =
            tokio::try_join!(name_call.call(), symbol_call.call(), decimals_call.call())?;

        let model = TokenInfoModel {
            chain_id: chain_id as i64,
            address: address,
            name: Some(name),
            symbol: Some(symbol),
            token_icon: None,
            decimals: Some(decimals as i16),
            created_at: None,
            updated_at: None,
        };

        let active_model = tokens::ActiveModel {
            chain_id: Set(model.chain_id),
            address: Set(model.address.clone()),
            symbol: Set(model.symbol.clone()),
            name: Set(model.name.clone()),
            token_icon: Set(model.token_icon.clone()),
            decimals: Set(model.decimals),
            ..Default::default()
        };

        self.db.upsert_token_info(active_model).await?;

        let mut cache = self.token_info_cache.write();
        cache.entry(key).or_insert_with(|| model.clone());

        Ok(model)
    }
}
