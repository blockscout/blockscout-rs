use crate::charts_config::{ChartSettings, Config};
use stats::{counters, entity::sea_orm_active_enums::ChartType, lines, Chart};
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    sync::Arc,
};

pub type ArcChart = Arc<dyn Chart + Send + Sync + 'static>;

pub struct Charts {
    pub config: Config,
    pub charts: Vec<ArcChart>,
    pub counters_filter: HashSet<String>,
    pub lines_filter: HashSet<String>,
    pub settings: HashMap<String, ChartSettings>,
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

struct ValidatedConfig {
    charts: Vec<ArcChart>,
    counters_filter: HashSet<String>,
    lines_filter: HashSet<String>,
}

impl Charts {
    pub fn new(config: Config) -> Result<Self, anyhow::Error> {
        let ValidatedConfig {
            charts,
            counters_filter,
            lines_filter,
        } = Self::validate_config(&config)?;
        let settings = Self::new_settings(&config);
        Ok(Self {
            config,
            charts,
            counters_filter,
            lines_filter,
            settings,
        })
    }

    fn validate_config(config: &Config) -> Result<ValidatedConfig, anyhow::Error> {
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

        Ok(ValidatedConfig {
            charts,
            counters_filter,
            lines_filter,
        })
    }

    // assumes that config is valid
    fn new_settings(config: &Config) -> HashMap<String, ChartSettings> {
        config
            .counters
            .iter()
            .map(|counter| (counter.id.clone(), counter.settings.clone()))
            .chain(config.lines.sections.iter().flat_map(|section| {
                section
                    .charts
                    .iter()
                    .map(|chart| (chart.id.clone(), chart.settings.clone()))
            }))
            .collect()
    }

    fn all_charts() -> Vec<ArcChart> {
        vec![
            // finished counters
            Arc::new(counters::TotalBlocks::default()),
            Arc::new(counters::AverageBlockTime::default()),
            Arc::new(counters::TotalTxns::default()),
            Arc::new(counters::TotalTokens::default()),
            Arc::new(counters::CompletedTxns::default()),
            // finished lines
            Arc::new(lines::NewBlocks::default()),
            Arc::new(lines::AverageGasPrice::default()),
            Arc::new(lines::ActiveAccounts::default()),
            Arc::new(lines::AccountsGrowth::default()),
            Arc::new(lines::TxnsFee::default()),
            Arc::new(lines::NewTxns::default()),
            Arc::new(lines::AverageBlockSize::default()),
            Arc::new(lines::AverageGasLimit::default()),
            // mock counters
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
            Arc::new(lines::MockLine::new("txnsGrowth".into(), 1000..10_000_000)),
        ]
    }
}
