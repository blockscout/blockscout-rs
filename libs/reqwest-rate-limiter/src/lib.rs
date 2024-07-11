use std::num::NonZeroU32;
use governor::{middleware, state, clock, NotUntil, RateLimiter};
use reqwest::{Request, Response};
use reqwest_middleware::{Middleware, Next};
use std::sync::Arc;

pub type DefaultRateLimiterMiddleware<
    MW = middleware::NoOpMiddleware<<clock::DefaultClock as clock::Clock>::Instant>,
> = RateLimiterMiddleware<state::direct::NotKeyed, state::InMemoryState, clock::DefaultClock, MW>;

impl DefaultRateLimiterMiddleware {
    pub fn per_second(max_burst: NonZeroU32) -> Self {
        RateLimiterMiddleware::new(RateLimiter::direct(
            governor::Quota::per_second(max_burst),
        ))
    }

    pub fn per_minute(max_burst: NonZeroU32) -> Self {
        RateLimiterMiddleware::new(RateLimiter::direct(
            governor::Quota::per_minute(max_burst),
        ))
    }

    pub fn per_hour(max_burst: NonZeroU32) -> Self {
        RateLimiterMiddleware::new(RateLimiter::direct(
            governor::Quota::per_hour(max_burst),
        ))
    }
}

#[derive(Clone)]
pub struct RateLimiterMiddleware<K, S, C, MW>
where
    S: state::StateStore<Key = K>,
    C: clock::Clock,
    MW: middleware::RateLimitingMiddleware<C::Instant>,
{
    rate_limiter: Arc<RateLimiter<K, S, C, MW>>,
}

impl<K, S, C, MW> RateLimiterMiddleware<K, S, C, MW>
where
    S: state::StateStore<Key = K>,
    C: clock::Clock,
    MW: middleware::RateLimitingMiddleware<C::Instant>,
{
    pub fn new(rate_limiter: RateLimiter<K, S, C, MW>) -> Self {
        Self::new_arc(Arc::new(rate_limiter))
    }

    pub fn new_arc(rate_limiter: Arc<RateLimiter<K, S, C, MW>>) -> Self {
        Self { rate_limiter }
    }
}

#[async_trait::async_trait]
impl<S, C, MW, PO> Middleware for RateLimiterMiddleware<state::direct::NotKeyed, S, C, MW>
where
    S: state::StateStore<Key = state::direct::NotKeyed> + Send + Sync + 'static,
    C: clock::Clock + clock::ReasonablyRealtime + Send + Sync + 'static,
    MW: middleware::RateLimitingMiddleware<
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
        extensions: &mut http::Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<Response> {
        self.rate_limiter.until_ready().await;
        next.run(req, extensions).await
    }
}
