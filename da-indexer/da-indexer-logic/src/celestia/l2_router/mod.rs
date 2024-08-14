mod arbitrum;
mod optimism;
pub mod settings;
pub mod types;

use anyhow::Result;
use blockscout_display_bytes::Bytes;
use serde::{Deserialize, Serialize};
use settings::L2RouterSettings;
use std::{collections::HashMap, fs};
use types::{L2BatchMetadata, L2Config, L2Type};

#[derive(Serialize, Deserialize)]
pub struct L2Router {
    pub routes: HashMap<String, L2Config>,
}

impl L2Router {
    pub fn new(routes: HashMap<String, L2Config>) -> Result<Self> {
        Ok(Self { routes })
    }

    pub fn from_settings(settings: L2RouterSettings) -> Result<Self> {
        let routes = fs::read_to_string(&settings.routes_path).map_err(|err| {
            anyhow::anyhow!(
                "failed to read routes file from path {}: {}",
                settings.routes_path,
                err
            )
        })?;
        let router: L2Router = toml::from_str(&routes)?;
        router.routes.iter().for_each(|(namespace, config)| {
            tracing::info!("registered route: {} -> {:?}", namespace, config);
        });
        Ok(router)
    }

    pub async fn get_l2_batch_metadata(
        &self,
        height: u64,
        namespace: &[u8],
        commitment: &[u8],
    ) -> Result<Option<L2BatchMetadata>> {
        let namespace = Bytes::from(namespace.to_vec()).to_string();
        let config = self.routes.get(&namespace);
        if config.is_none() {
            tracing::debug!("unknown namespace: {}", &namespace);
            return Ok(None);
        }
        let config = config.unwrap();

        match config.chain_type {
            L2Type::Optimism => optimism::get_l2_batch(config, height, commitment).await,
            L2Type::Arbitrum => arbitrum::get_l2_batch(config, height, commitment).await,
        }
    }
}
