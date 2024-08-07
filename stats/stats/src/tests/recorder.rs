use std::{marker::PhantomData, ops::Deref, sync::Arc};

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::data_source::types::Get;

#[async_trait]
pub trait Recorder {
    type Data;
    async fn record(next: Self::Data);
    async fn get_records() -> Vec<Self::Data>;
}

pub struct InMemoryRecorder<InMemoryStorage>(PhantomData<InMemoryStorage>);

#[async_trait]
impl<InMemoryStorage, Data> Recorder for InMemoryRecorder<InMemoryStorage>
where
    Data: Clone + Send + 'static,
    InMemoryStorage: Get<Value = Arc<Mutex<Vec<Data>>>>,
{
    type Data = Data;
    async fn record(next: Self::Data) {
        InMemoryStorage::get().lock().await.push(next)
    }

    async fn get_records() -> Vec<Data> {
        InMemoryStorage::get().lock().await.deref().to_vec()
    }
}
