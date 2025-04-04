use super::compiler::VyperCompiler;
use crate::{compiler::Compilers, verify_new};
use std::sync::Arc;

pub struct Client {
    compilers: Arc<Compilers<VyperCompiler>>,
    new_compilers: Arc<verify_new::EvmCompilersPool<verify_new::VyperCompiler>>,
}

impl Client {
    /// Convenience method to initialize new vyper client.
    ///
    /// If you need to keep a reference to the compilers after initialization, use [`new_arc`].
    ///
    /// [`new_arc`]: Self::new_arc
    pub fn new(compilers: Compilers<VyperCompiler>) -> Self {
        Self::new_arc(Arc::new(compilers))
    }

    /// Initialize new vyper client. [`new`] is more ergonomic if you don't need the `Arc`.
    ///
    /// [`new`]: Self::new
    pub fn new_arc(compilers: Arc<Compilers<VyperCompiler>>) -> Self {
        let new_compilers = verify_new::EvmCompilersPool::new(
            compilers.fetcher.clone(),
            compilers.threads_semaphore.clone(),
        );
        Self {
            compilers,
            new_compilers: Arc::new(new_compilers),
        }
    }

    pub fn compilers(&self) -> &Compilers<VyperCompiler> {
        self.compilers.as_ref()
    }

    pub fn new_compilers(&self) -> &verify_new::EvmCompilersPool<verify_new::VyperCompiler> {
        self.new_compilers.as_ref()
    }
}
