use crate::charts_config::{ChartSettings, Config};
use stats::{cache::Cache, counters, entity::sea_orm_active_enums::ChartType, lines, Chart};
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
        let accounts_cache = Cache::default();
        let new_txns = Arc::new(lines::NewTxns::default());
        let new_native_coin_transfers = Arc::new(lines::NewNativeCoinTransfers::default());
        let native_coin_holders_growth = Arc::new(lines::NativeCoinHoldersGrowth::default());

        let new_verified_contracts = Arc::new(lines::NewVerifiedContracts::default());
        let new_contracts = Arc::new(lines::NewContracts::default());

        vec![
            // finished counters
            Arc::new(counters::TotalBlocks::default()),
            Arc::new(counters::AverageBlockTime::default()),
            Arc::new(counters::TotalTxns::new(new_txns.clone())),
            Arc::new(counters::TotalTokens::default()),
            Arc::new(counters::CompletedTxns::default()),
            Arc::new(counters::TotalAccounts::new(accounts_cache.clone())),
            Arc::new(counters::TotalNativeCoinTransfers::new(
                new_native_coin_transfers.clone(),
            )),
            Arc::new(counters::TotalNativeCoinHolders::new(
                native_coin_holders_growth.clone(),
            )),
            Arc::new(counters::LastNewContracts::new(new_contracts.clone())),
            Arc::new(counters::LastNewVerifiedContracts::new(
                new_verified_contracts.clone(),
            )),
            // finished lines
            Arc::new(lines::NewBlocks::default()),
            Arc::new(lines::AverageGasPrice::default()),
            Arc::new(lines::ActiveAccounts::default()),
            Arc::new(lines::AccountsGrowth::new(accounts_cache.clone())),
            Arc::new(lines::TxnsFee::default()),
            Arc::new(lines::TxnsGrowth::new(new_txns.clone())),
            new_txns,
            Arc::new(lines::AverageBlockSize::default()),
            Arc::new(lines::AverageGasLimit::default()),
            Arc::new(lines::GasUsedGrowth::default()),
            Arc::new(lines::NativeCoinSupply::default()),
            Arc::new(lines::NativeCoinHoldersGrowth::default()),
            Arc::new(lines::AverageTxnFee::default()),
            Arc::new(lines::TxnsSuccessRate::default()),
            Arc::new(lines::AverageBlockRewards::default()),
            Arc::new(lines::NewAccounts::new(accounts_cache)),
            Arc::new(lines::NewNativeCoinHolders::new(
                native_coin_holders_growth.clone(),
            )),
            native_coin_holders_growth,
            new_native_coin_transfers,
            Arc::new(lines::VerifiedContractsGrowth::new(
                new_verified_contracts.clone(),
            )),
            Arc::new(lines::ContractsGrowth::new(new_contracts.clone())),
            new_verified_contracts,
            new_contracts,
        ]
    }
}
