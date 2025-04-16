use crate::verify_new::VyperCompiler;
use std::sync::Arc;

type EvmCompilersPool = crate::EvmCompilersPool<VyperCompiler>;

pub struct Client {
    compilers: Arc<EvmCompilersPool>,
}

impl Client {
    /// Convenience method to initialize new vyper client.
    ///
    /// If you need to keep a reference to the compilers after initialization, use [`new_arc`].
    ///
    /// [`new_arc`]: Self::new_arc
    pub fn new(compilers: EvmCompilersPool) -> Self {
        Self::new_arc(Arc::new(compilers))
    }

    /// Initialize new vyper client. [`new`] is more ergonomic if you don't need the `Arc`.
    ///
    /// [`new`]: Self::new
    pub fn new_arc(compilers: Arc<EvmCompilersPool>) -> Self {
        Self { compilers }
    }

    pub fn compilers(&self) -> &EvmCompilersPool {
        self.compilers.as_ref()
    }
}
