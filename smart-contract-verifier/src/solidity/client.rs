use super::compiler::SolidityCompiler;
use crate::{compiler::Compilers, middleware::Middleware, verifier::Success};
use std::sync::Arc;

pub struct ClientBuilder {
    compilers: Compilers<SolidityCompiler>,
    middleware_stack: Vec<Arc<dyn Middleware<Success>>>,
}

impl ClientBuilder {
    pub fn new(compilers: Compilers<SolidityCompiler>) -> Self {
        Self {
            compilers,
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
        M: Middleware<Success>,
    {
        self.with_arc(Arc::new(middleware))
    }

    /// Add middleware to the chain. [`with`] is more ergonomic if you don't need the `Arc`.
    ///
    /// [`with`]: Self::with
    pub fn with_arc(mut self, middleware: Arc<dyn Middleware<Success>>) -> Self {
        self.middleware_stack.push(middleware);
        self
    }

    /// Returns a [`Client`] using this builder configuration.
    pub fn build(self) -> Client {
        Client::new(self.compilers, self.middleware_stack)
    }
}

pub struct Client {
    compilers: Compilers<SolidityCompiler>,
    middleware_stack: Box<[Arc<dyn Middleware<Success>>]>,
}

impl Client {
    /// See [`ClientBuilder`] for a more ergonomic way to build `Client` instances.
    pub fn new<T>(compilers: Compilers<SolidityCompiler>, middleware_stack: T) -> Self
    where
        T: Into<Box<[Arc<dyn Middleware<Success>>]>>,
    {
        Self {
            compilers,
            middleware_stack: middleware_stack.into(),
        }
    }

    pub fn compilers(&self) -> &Compilers<SolidityCompiler> {
        &self.compilers
    }

    pub fn middleware(&self) -> &[Arc<dyn Middleware<Success>>] {
        self.middleware_stack.as_ref()
    }
}
