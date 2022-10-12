use super::compiler::VyperCompiler;
use crate::compiler::Compilers;
use std::sync::Arc;

pub struct Client {
    compilers: Arc<Compilers<VyperCompiler>>,
}

impl Client {
    pub fn new(compilers: Compilers<VyperCompiler>) -> Self {
        Self {
            compilers: Arc::new(compilers),
        }
    }

    pub fn compilers(&self) -> Arc<Compilers<VyperCompiler>> {
        self.compilers.clone()
    }
}
