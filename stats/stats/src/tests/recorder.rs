use std::{
    marker::PhantomData,
    ops::Deref,
    sync::{Arc, Mutex},
};

use crate::data_source::types::Get;

pub trait Recorder {
    type Data;
    fn record(next: Self::Data);
    fn get_records() -> Vec<Self::Data>;
}

pub struct InMemoryRecorder<Data, InMemoryStorage>(PhantomData<(Data, InMemoryStorage)>)
where
    InMemoryStorage: Get<Arc<Mutex<Vec<Data>>>>;

impl<Data, InMemoryStorage> Recorder for InMemoryRecorder<Data, InMemoryStorage>
where
    Data: Clone,
    InMemoryStorage: Get<Arc<Mutex<Vec<Data>>>>,
{
    type Data = Data;
    fn record(next: Data) {
        InMemoryStorage::get().lock().unwrap().push(next)
    }

    fn get_records() -> Vec<Data> {
        InMemoryStorage::get().lock().unwrap().deref().to_vec()
    }
}
