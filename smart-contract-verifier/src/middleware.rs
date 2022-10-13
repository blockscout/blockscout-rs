#[async_trait::async_trait]
pub trait Middleware<Output>: 'static + Send + Sync {
    /// Invoked with a verification output after successful verification.
    ///
    /// All errors should be handled inside the middleware and should not
    /// be propagated to the client that called it.
    async fn call(&self, output: &Output) -> ();
}
