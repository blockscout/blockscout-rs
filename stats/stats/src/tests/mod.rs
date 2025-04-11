#![cfg(any(feature = "test-utils", test))]

use itertools::Itertools;

pub mod init_db;
pub mod mock_blockscout;
pub mod point_construction;
pub mod recorder;
pub mod simple_test;

pub fn normalize_sql(statement: &str) -> String {
    statement.split_whitespace().collect_vec().join(" ")
}
