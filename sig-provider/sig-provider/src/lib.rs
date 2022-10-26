mod aggregator;
mod sources;

pub use aggregator::SourceAggregator;
pub use sources::{fourbyte, sigeth, SignatureSource};

#[cfg(test)]
pub use sources::mock;
