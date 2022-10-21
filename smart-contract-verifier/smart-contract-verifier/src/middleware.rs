use std::sync::Arc;

/// When implemented the struct could be added as a verification success
/// post-processing step.
//
// Output type implemented as a generic and not as an associated type,
// so that the same middleware provider could process different output types.
#[async_trait::async_trait]
pub trait Middleware<Output>: 'static + Send + Sync {
    /// Invoked with a verification output after successful verification.
    ///
    /// All errors should be handled inside the middleware and should not
    /// be propagated to the client that called it.
    async fn call(&self, output: &Output) -> ();
}

#[derive(Default, Clone)]
pub struct Composition<Output> {
    middleware_stack: Vec<Arc<dyn Middleware<Output>>>,
}

impl<Output> Composition<Output> {
    /// Initialize empty composition.
    pub fn new() -> Self {
        Self {
            middleware_stack: vec![],
        }
    }

    /// Convenience method to attach middleware.
    ///
    /// If you need to keep a reference to the middleware after attaching, use [`with_arc`].
    ///
    /// [`with_arc`]: Self::with_arc
    pub fn with<M>(self, middleware: M) -> Self
    where
        M: Middleware<Output>,
    {
        self.with_arc(Arc::new(middleware))
    }

    /// Add middleware to the composition. [`with`] is more ergonomic if you don't need the `Arc`.
    ///
    /// [`with`]: Self::with
    pub fn with_arc(mut self, middleware: Arc<dyn Middleware<Output>>) -> Self {
        self.middleware_stack.push(middleware);
        self
    }
}

#[async_trait::async_trait]
impl<Output: 'static + Sync> Middleware<Output> for Composition<Output> {
    async fn call(&self, output: &Output) -> () {
        for middleware in &self.middleware_stack {
            middleware.call(output).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;

    mock! {
        Middleware<Output: 'static + Send + Sync> {}

        #[async_trait::async_trait]
        impl<Output: 'static + Send + Sync> super::Middleware<Output> for Middleware<Output> {
            async fn call(&self, output: &Output) -> ();
        }
    }

    #[tokio::test]
    async fn composition() {
        let mut middleware1 = MockMiddleware::<()>::new();
        let mut middleware2 = MockMiddleware::<()>::new();

        middleware1.expect_call().times(1).return_const(());
        middleware2.expect_call().times(1).return_const(());

        let composition = Composition::new()
            .with(middleware1)
            .with_arc(Arc::new(middleware2));
        composition.call(&()).await;
    }
}
