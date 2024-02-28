mod cache_views;
mod domain;
mod transaction_history;

pub use cache_views::*;
pub use domain::*;
pub use transaction_history::*;

pub fn bind_string_list(list: &[impl AsRef<str>]) -> Vec<String> {
    list.iter()
        .map(|s| s.as_ref().to_string())
        .collect::<Vec<_>>()
}
