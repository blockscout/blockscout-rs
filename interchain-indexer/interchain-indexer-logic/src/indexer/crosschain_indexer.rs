use anyhow::Error;

pub trait CrosschainIndexer: Send + Sync {
    fn start_indexing(&self) -> Result<(), Error>;
    fn stop_indexing(&self) -> Result<(), Error>;
}
