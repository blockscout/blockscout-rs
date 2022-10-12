use super::compiler::SolidityCompiler;
use crate::compiler::Compilers;
use std::sync::Arc;

pub struct Client {
    compilers: Arc<Compilers<SolidityCompiler>>,
}

impl Client {
    pub fn new(compilers: Compilers<SolidityCompiler>) -> Self {
        Self {
            compilers: Arc::new(compilers),
        }
    }

    pub fn compilers(&self) -> Arc<Compilers<SolidityCompiler>> {
        self.compilers.clone()
    }
}
