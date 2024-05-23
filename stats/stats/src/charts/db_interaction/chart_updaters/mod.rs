mod batch;
pub(crate) mod common_operations;
mod dependent;

pub use batch::RemoteBatchQuery;
pub use dependent::{last_point, parse_and_cumsum, parse_and_sum};
