use super::ChainId;
use chrono::NaiveDateTime;
use entity::counters_global_imported::Model as GlobalCountersModel;

#[derive(Debug, Clone)]
pub struct ChainCounters {
    pub chain_id: ChainId,
    pub timestamp: NaiveDateTime,
    pub daily_transactions_number: Option<u64>,
    pub total_transactions_number: Option<u64>,
    pub total_addresses_number: Option<u64>,
}

impl From<ChainCounters> for GlobalCountersModel {
    fn from(v: ChainCounters) -> Self {
        Self {
            id: Default::default(),
            chain_id: v.chain_id,
            date: v.timestamp.date(),
            daily_transactions_number: v
                .daily_transactions_number
                .and_then(|n| i64::try_from(n).ok()),
            total_transactions_number: v
                .total_transactions_number
                .and_then(|n| i64::try_from(n).ok()),
            total_addresses_number: v.total_addresses_number.and_then(|n| i64::try_from(n).ok()),
            created_at: Default::default(),
            updated_at: Default::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Counters {
    pub global: Option<ChainCounters>,
}
