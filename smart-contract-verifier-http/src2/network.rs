use futures::Future;
use std::{num::NonZeroUsize, time::Duration};

pub async fn make_retrying_request<F, Fut, Response, Error>(
    attempts: NonZeroUsize,
    sleep_between: Option<Duration>,
    request: F,
) -> Result<Response, Error>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<Response, Error>>,
{
    for _ in 0..attempts.get() - 1 {
        let resp = request().await;
        if resp.is_ok() {
            return resp;
        }
        if let Some(duration) = sleep_between {
            tokio::time::sleep(duration).await;
        }
    }
    request().await
}
