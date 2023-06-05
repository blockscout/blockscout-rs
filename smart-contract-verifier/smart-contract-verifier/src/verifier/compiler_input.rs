pub trait CompilerInput {
    /// Modifies input so that the corresponding bytecode
    /// should have modified metadata hash, if any.
    fn modify(self) -> Self;
}

impl CompilerInput for ethers_solc::CompilerInput {
    fn modify(mut self) -> Self {
        // TODO: could we update some other field to avoid copying strings?
        self.sources.iter_mut().for_each(|(_file, source)| {
            let mut modified_content = source.content.as_ref().clone();
            modified_content.push(' ');
            source.content = std::sync::Arc::new(modified_content);
        });
        self
    }
}
