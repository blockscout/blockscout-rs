#[macro_export]
macro_rules! impl_compiler_input {
    ($target:path) => {
        impl crate::verifier::CompilerInput for $target {
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
    };
}
pub use impl_compiler_input;

pub trait CompilerInput {
    /// Modifies input so that the corresponding bytecode
    /// should have modified metadata hash, if any.
    fn modify(self) -> Self;
}

impl_compiler_input!(foundry_compilers::CompilerInput);
