use governor::{
    clock::{Clock, ReasonablyRealtime},
    middleware::RateLimitingMiddleware,
    state::{NotKeyed, StateStore},
    NotUntil, RateLimiter,
};
use reqwest::{Request, Response};
use reqwest_middleware::{Middleware, Next};
use std::sync::Arc;

pub struct RateLimiterMiddleware<K, S, C, MW>
where
    S: StateStore<Key = K>,
    C: Clock,
    MW: RateLimitingMiddleware<C::Instant>,
{
    rate_limiter: Arc<RateLimiter<K, S, C, MW>>,
}

impl<K, S, C, MW> RateLimiterMiddleware<K, S, C, MW>
where
    S: StateStore<Key = K>,
    C: Clock,
    MW: RateLimitingMiddleware<C::Instant>,
{
    pub fn new(rate_limiter: RateLimiter<K, S, C, MW>) -> Self {
        Self::new_arc(Arc::new(rate_limiter))
    }

    pub fn new_arc(rate_limiter: Arc<RateLimiter<K, S, C, MW>>) -> Self {
        Self { rate_limiter }
    }
}

#[async_trait::async_trait]
impl<S, C, MW, PO> Middleware for RateLimiterMiddleware<NotKeyed, S, C, MW>
where
    S: StateStore<Key = NotKeyed> + Send + Sync + 'static,
    C: Clock + ReasonablyRealtime + Send + Sync + 'static,
    MW: RateLimitingMiddleware<
            C::Instant,
            NegativeOutcome = NotUntil<C::Instant>,
            PositiveOutcome = PO,
        > + Send
        + Sync
        + 'static,
    PO: Send,
{
    async fn handle(
        &self,
        req: Request,
        extensions: &mut task_local_extensions::Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<Response> {
        self.rate_limiter.until_ready().await;
        next.run(req, extensions).await
    }
}
