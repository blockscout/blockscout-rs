use super::ChainId;
use crate::error::ParseError;
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
            daily_transactions_number: v.daily_transactions_number.map(|n| n as i64),
            total_transactions_number: v.total_transactions_number.map(|n| n as i64),
            total_addresses_number: v.total_addresses_number.map(|n| n as i64),
            created_at: Default::default(),
            updated_at: v.timestamp,
        }
    }
}

impl TryFrom<GlobalCountersModel> for ChainCounters {
    type Error = ParseError;

    fn try_from(v: GlobalCountersModel) -> Result<Self, Self::Error> {
        Ok(Self {
            chain_id: v.chain_id,
            timestamp: v.updated_at,
            daily_transactions_number: v.daily_transactions_number.map(|n| n as u64),
            total_transactions_number: v.total_transactions_number.map(|n| n as u64),
            total_addresses_number: v.total_addresses_number.map(|n| n as u64),
        })
    }
}

#[derive(Debug, Clone)]
pub struct Counters {
    pub global: Option<ChainCounters>,
}
