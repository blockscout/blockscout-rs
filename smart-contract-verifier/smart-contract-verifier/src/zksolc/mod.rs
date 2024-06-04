mod compiler;
pub mod standard_json;

use crate::{CompactVersion, DetailedVersion};
use bytes::Bytes;
pub use compiler::ZkSolcCompiler;

#[derive(Clone, Debug)]
pub struct VerificationRequest<T> {
    pub code: Bytes,
    pub constructor_arguments: Option<Bytes>,
    pub zk_compiler: CompactVersion,
    pub solc_compiler: DetailedVersion,
    pub content: T,
}
