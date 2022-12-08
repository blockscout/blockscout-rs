/// The enum representing how provided bytecode corresponds
/// to the local result of source codes compilation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchType {
    Partial,
    Full,
}
