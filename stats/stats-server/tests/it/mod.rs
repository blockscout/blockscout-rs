//! Tests are combined into single "rust tests"
//! to reuse slowly-initialized parts, such as blockscout database
//! or stats service

pub mod common;
mod mock_blockscout_reindex;
mod mock_blockscout_simple;
