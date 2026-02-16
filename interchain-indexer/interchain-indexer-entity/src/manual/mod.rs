use crate::{bridge_contracts, indexer_checkpoints};

impl indexer_checkpoints::Model {
    pub fn validated_realtime_cursor(&self) -> u64 {
        self.realtime_cursor.max(0) as u64
    }

    pub fn validated_catchup_cursor(&self) -> u64 {
        self.catchup_max_cursor.max(0) as u64
    }
}

impl bridge_contracts::Model {
    pub fn validated_started_at_block(&self) -> u64 {
        self.started_at_block.unwrap_or(0).max(0) as u64
    }
}
