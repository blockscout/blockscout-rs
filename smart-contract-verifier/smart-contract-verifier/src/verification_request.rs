use crate::compiler::Version;
use bytes::Bytes;

/// A request structure that is generic over its content.
/// Is suitable only for verification methods implemented explicitly
/// (currently, all options except Sourcify verification)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerificationRequest<T> {
    /// Deployed bytecode of the contract to be verified.
    /// Is only used if creation bytecode was not provided.
    /// Otherwise it is completely ignored.
    pub deployed_bytecode: Bytes,
    /// Creation transaction input of the contract to be verified.
    /// If present, is used for the actual verification.
    /// Otherwise, deployed bytecode argument is used.
    pub creation_bytecode: Option<Bytes>,
    /// Compiler version the contract being verified should be compiled with.
    pub compiler_version: Version,

    /// Verification method specific field. Contains all data that
    /// the concrete method requires for the verification.
    pub content: T,
}
