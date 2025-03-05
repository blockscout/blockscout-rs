use super::compiler::SolidityCompiler;
use crate::compiler::Compilers;
use std::sync::Arc;

pub struct Client {
    compilers: Arc<Compilers<SolidityCompiler>>,
}

impl Client {
    /// Convenience method to initialize new solidity client.
    ///
    /// If you need to keep a reference to the compilers after initialization, use [`new_arc`].
    ///
    /// [`new_arc`]: Self::new_arc
    pub fn new(compilers: Compilers<SolidityCompiler>) -> Self {
        Self::new_arc(Arc::new(compilers))
    }

    /// Initialize new solidity client. [`new`] is more ergonomic if you don't need the `Arc`.
    ///
    /// [`new`]: Self::new
    pub fn new_arc(compilers: Arc<Compilers<SolidityCompiler>>) -> Self {
        Self { compilers }
    }

    pub fn compilers(&self) -> &Compilers<SolidityCompiler> {
        self.compilers.as_ref()
    }
}
