use serde::Deserialize;
use stats::{counters, entity::sea_orm_active_enums::ChartType, lines, Chart};
use stats_proto::blockscout::stats::v1::LineCharts;
use std::{collections::HashSet, hash::Hash, sync::Arc};

#[derive(Debug, Clone, Deserialize)]
pub struct CounterInfo {
    pub id: String,
    pub title: String,
    pub units: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub counters: Vec<CounterInfo>,
    pub lines: LineCharts,
}

pub type ArcChart = Arc<dyn Chart + Send + Sync + 'static>;

pub struct Charts {
    pub config: Config,
    pub charts: Vec<ArcChart>,
    pub counters_filter: HashSet<String>,
    pub lines_filter: HashSet<String>,
}

fn new_hashset_check_duplicates<T: Hash + Eq, I: IntoIterator<Item = T>>(
    iter: I,
) -> Result<HashSet<T>, T> {
    let mut result = HashSet::default();
    for item in iter.into_iter() {
        let prev_value = result.replace(item);
        if let Some(prev_value) = prev_value {
            return Err(prev_value);
        }
    }
    Ok(result)
}

impl Charts {
    pub fn new(config: Config) -> Result<Self, anyhow::Error> {
        let counters_filter = config.counters.iter().map(|counter| counter.id.clone());
        let counters_filter = new_hashset_check_duplicates(counters_filter)
            .map_err(|id| anyhow::anyhow!("encountered same id twice: {}", id))?;

        let lines_filter = config
            .lines
            .sections
            .iter()
            .flat_map(|section| section.charts.iter().map(|chart| chart.id.clone()));
        let lines_filter = new_hashset_check_duplicates(lines_filter)
            .map_err(|id| anyhow::anyhow!("encountered same id twice: {}", id))?;

        let mut counters_unknown = counters_filter.clone();
        let mut lines_unknown = lines_filter.clone();
        let charts = Self::all_charts()
            .into_iter()
            .filter(|chart| match chart.chart_type() {
                ChartType::Counter => counters_unknown.remove(chart.name()),
                ChartType::Line => lines_unknown.remove(chart.name()),
            })
            .collect();

        if !counters_unknown.is_empty() || !lines_unknown.is_empty() {
            return Err(anyhow::anyhow!(
                "found unknown chart ids: {:?}",
                counters_unknown.union(&lines_unknown)
            ));
        }

        Ok(Self {
            config,
            charts,
            counters_filter,
            lines_filter,
        })
    }

    fn all_charts() -> Vec<ArcChart> {
        vec![
            // finished counters
            Arc::new(counters::TotalBlocks::default()),
            Arc::new(counters::AverageBlockTime::default()),
            Arc::new(counters::TotalTxns::default()),
            Arc::new(counters::TotalTokens::default()),
            // finished lines
            Arc::new(lines::NewBlocks::default()),
            Arc::new(lines::AverageGasPrice::default()),
            Arc::new(lines::ActiveAccounts::default()),
            Arc::new(lines::AccountsGrowth::default()),
            // mock counters
            Arc::new(counters::MockCounter::new(
                "completedTxns".into(),
                "956276037263".into(),
            )),
            Arc::new(counters::MockCounter::new(
                "totalAccounts".into(),
                "765543".into(),
            )),
            Arc::new(counters::MockCounter::new(
                "totalNativeCoinHolders".into(),
                "409559".into(),
            )),
            Arc::new(counters::MockCounter::new(
                "totalNativeCoinTransfers".into(),
                "32528".into(),
            )),
            // mock lines
            Arc::new(lines::MockLine::new(
                "averageBlockSize".into(),
                90_000..100_000,
            )),
            Arc::new(lines::MockLine::new(
                "averageGasLimit".into(),
                8_000_000..30_000_000,
            )),
            Arc::new(lines::MockLine::new("averageTxnFee".into(), 0.0001..0.01)),
            Arc::new(lines::MockLine::new(
                "gasUsedGrowth".into(),
                1_000_000..100_000_000,
            )),
            Arc::new(lines::MockLine::new(
                "nativeCoinHoldersGrowth".into(),
                1000..5000,
            )),
            Arc::new(lines::MockLine::new(
                "nativeCoinSupply".into(),
                1_000_000..100_000_000,
            )),
            Arc::new(lines::MockLine::new(
                "newNativeCoinTransfers".into(),
                100..10_000,
            )),
            Arc::new(lines::MockLine::new("newTxns".into(), 200..20_000)),
            Arc::new(lines::MockLine::new("txnsFee".into(), 0.0001..0.01)),
            Arc::new(lines::MockLine::new("txnsGrowth".into(), 1000..10_000_000)),
        ]
    }
}
