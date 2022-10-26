use crate::SignatureSource;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use tokio::sync::Mutex;

type Call<Q, R> = Mutex<Option<(Q, R)>>;

#[derive(Default, Debug)]
pub struct Source {
    create: Call<String, Result<(), anyhow::Error>>,
    function: Call<String, Result<Vec<String>, anyhow::Error>>,
    event: Call<String, Result<Vec<String>, anyhow::Error>>,
}

impl Source {
    pub fn with_create(mut self, abi: String, result: Result<(), anyhow::Error>) -> Self {
        *self.create.get_mut() = Some((abi, result));
        self
    }

    pub fn with_function(
        mut self,
        hex: String,
        result: Result<Vec<String>, anyhow::Error>,
    ) -> Self {
        *self.function.get_mut() = Some((hex, result));
        self
    }

    pub fn with_event(mut self, hex: String, result: Result<Vec<String>, anyhow::Error>) -> Self {
        *self.event.get_mut() = Some((hex, result));
        self
    }

    pub fn build(self) -> Arc<Self> {
        Arc::new(self)
    }
}

#[async_trait::async_trait]
impl SignatureSource for Source {
    async fn create_signatures(&self, abi: &str) -> Result<(), anyhow::Error> {
        let mut call = self.create.lock().await;
        match call.take() {
            Some((request, response)) => {
                assert_eq!(&request, abi);
                response
            }
            None => {
                panic!("unexpected call of create_signature");
            }
        }
    }

    async fn get_function_signatures(&self, hex: &str) -> Result<Vec<String>, anyhow::Error> {
        let mut call = self.function.lock().await;
        match call.take() {
            Some((request, response)) => {
                assert_eq!(&request, hex);
                response
            }
            None => {
                panic!("unexpected call of get_function_signatures");
            }
        }
    }

    async fn get_event_signatures(&self, hex: &str) -> Result<Vec<String>, anyhow::Error> {
        let mut call = self.event.lock().await;
        match call.take() {
            Some((request, response)) => {
                assert_eq!(&request, hex);
                response
            }
            None => {
                panic!("unexpected call of get_event_signatures");
            }
        }
    }

    fn source(&self) -> String {
        "mock".into()
    }
}
