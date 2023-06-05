use super::compiler::SolidityCompiler;
use crate::{compiler::Compilers, middleware::Middleware, solidity::Success};
use std::sync::Arc;

pub struct Client {
    compilers: Arc<Compilers<SolidityCompiler>>,
    middleware: Option<Arc<dyn Middleware<Success>>>,
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
        Self {
            compilers,
            middleware: None,
        }
    }

    /// Convenience method to attach middleware.
    ///
    /// If you need to keep a reference to the middleware after attaching, use [`with_middleware_arc`].
    ///
    /// [`with_middleware_arc`]: Self::with_middleware_arc
    pub fn with_middleware(self, middleware: impl Middleware<Success>) -> Self {
        self.with_middleware_arc(Arc::new(middleware))
    }

    /// Add middleware to the client. [`with_middleware`] is more ergonomic if you don't need the `Arc`.
    ///
    /// [`with_middleware`]: Self::with_middleware
    pub fn with_middleware_arc(mut self, middleware: Arc<impl Middleware<Success>>) -> Self {
        self.middleware = Some(middleware);
        self
    }

    pub fn compilers(&self) -> &Compilers<SolidityCompiler> {
        self.compilers.as_ref()
    }

    /// Provides a reference to the middleware, if there is any.
    pub fn middleware(&self) -> Option<&dyn Middleware<Success>> {
        self.middleware.as_ref().map(|m| m.as_ref())
    }
}
