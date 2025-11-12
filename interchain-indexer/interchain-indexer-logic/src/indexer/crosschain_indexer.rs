use anyhow::Error;

use crate::{InterchainDatabase, ProviderPool};

use std::{collections::HashMap, sync::Arc};

pub trait CrosschainIndexer: Send + Sync {
    fn new(
        db: Arc<InterchainDatabase>,
        bridge_id: i32,
        providers: HashMap<u64, Arc<ProviderPool>>,
    ) -> Result<Self, Error>
    where
        Self: Sized;
    fn start_indexing(&self) -> Result<(), Error>;
    fn stop_indexing(&self) -> Result<(), Error>;
}
