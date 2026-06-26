use std::collections::HashMap;

use alloy::{primitives::B256, rpc::types::Log};
use itertools::Itertools;

pub(crate) fn group_logs_by_transaction(logs: &[Log]) -> HashMap<B256, Vec<&Log>> {
    logs.iter()
        .filter_map(|log| log.transaction_hash.map(|hash| (hash, log)))
        .into_group_map()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_logs_by_transaction_preserves_input_order() {
        let tx = B256::with_last_byte(1);
        let first = Log {
            transaction_hash: Some(tx),
            log_index: Some(2),
            ..Default::default()
        };
        let second = Log {
            transaction_hash: Some(tx),
            log_index: Some(3),
            ..Default::default()
        };

        let logs_input = [first, second];
        let grouped = group_logs_by_transaction(&logs_input);
        let logs = grouped.get(&tx).expect("tx group exists");

        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].log_index, Some(2));
        assert_eq!(logs[1].log_index, Some(3));
    }
}
