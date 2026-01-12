use super::ChainId;
use crate::proto;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WeeklyMetric {
    pub current_week: i64,
    pub previous_week: i64,
    /// Week-over-week difference as a percentage (e.g., "42.2" for 42.2% increase).
    pub wow_diff_percent: f64,
}

impl From<WeeklyMetric> for proto::WeeklyMetric {
    fn from(v: WeeklyMetric) -> Self {
        Self {
            current_week: v.current_week.to_string(),
            previous_week: v.previous_week.to_string(),
            wow_diff_percent: format!("{:.2}", v.wow_diff_percent),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainMetrics {
    pub chain_id: ChainId,
    pub tps: Option<f64>,
    pub new_addresses: Option<WeeklyMetric>,
    pub daily_transactions: Option<WeeklyMetric>,
    pub active_accounts: Option<WeeklyMetric>,
}

impl From<ChainMetrics> for proto::ChainMetrics {
    fn from(v: ChainMetrics) -> Self {
        Self {
            chain_id: v.chain_id.to_string(),
            tps: v.tps.map(|tps| format!("{:.2}", tps)),
            new_addresses: v.new_addresses.map(|m| m.into()),
            daily_transactions: v.daily_transactions.map(|m| m.into()),
            active_accounts: v.active_accounts.map(|m| m.into()),
        }
    }
}
