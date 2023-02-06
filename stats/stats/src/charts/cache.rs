use std::{future::Future, sync::Arc};
use tokio::sync::Mutex;

#[derive(Default, Debug)]
struct CacheData<T> {
    data: Option<T>,
    version: u64,
}

impl<T> CacheData<T> {
    fn update(&mut self, data: T) {
        self.data = Some(data);
        self.version += 1;
    }
}

#[derive(Default, Debug)]
pub struct Cache<T> {
    data: Arc<Mutex<CacheData<T>>>,
    last_version: u64,
}

impl<T: Clone> Cache<T> {
    pub async fn get_or_update<E, F: Future<Output = Result<T, E>>>(
        &mut self,
        updater: F,
    ) -> Result<T, E> {
        let mut cache = self.data.lock().await;
        let data = match cache.data.as_ref() {
            Some(data) if cache.version > self.last_version => data.clone(),
            _ => {
                let new_data = updater.await?;
                cache.update(new_data.clone());
                new_data
            }
        };
        self.last_version = cache.version;
        Ok(data)
    }
}

impl<T> Clone for Cache<T> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            last_version: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(val: u32) -> Result<u32, ()> {
        Ok(val)
    }

    fn assert_not_called() -> Result<u32, ()> {
        panic!("should use cached version");
    }

    #[tokio::test]
    async fn works() {
        let mut foo: Cache<u32> = Cache::default();
        let mut bar = foo.clone();

        assert_eq!(Ok(1), foo.get_or_update(async move { value(1) }).await);
        assert_eq!(
            Ok(1),
            bar.get_or_update(async move { assert_not_called() }).await
        );

        assert_eq!(Ok(2), bar.get_or_update(async move { value(2) }).await);
        assert_eq!(
            Ok(2),
            foo.get_or_update(async move { assert_not_called() }).await
        );

        assert_eq!(Ok(3), foo.get_or_update(async move { value(3) }).await);
        assert_eq!(Ok(4), foo.get_or_update(async move { value(4) }).await);
        assert_eq!(
            Ok(4),
            bar.get_or_update(async move { assert_not_called() }).await
        );

        let mut baz = bar.clone();
        assert_eq!(
            Ok(4),
            baz.get_or_update(async move { assert_not_called() }).await
        );
        assert_eq!(Ok(5), baz.get_or_update(async move { value(5) }).await);
    }
}
