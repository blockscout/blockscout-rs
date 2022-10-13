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
