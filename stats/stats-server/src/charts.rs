use crate::config::{
    toml_config::{Config, LineChartSection},
    ChartSettings,
};
use stats::{cache::Cache, counters, entity::sea_orm_active_enums::ChartType, lines, Chart};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    hash::Hash,
    sync::Arc,
};

pub type ArcChart = Arc<dyn Chart + Send + Sync + 'static>;

pub struct ChartInfo {
    pub chart: ArcChart,
    pub settings: ChartSettings,
}

pub struct Charts {
    pub config: Config,
    pub charts_info: BTreeMap<String, ChartInfo>,
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
        Self::validated(config)
    }

    fn validated(config: Config) -> Result<Self, anyhow::Error> {
        let config = Self::remove_disabled_charts(config);
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
        let settings = Self::new_settings(&config);
        let charts_info = Self::all_charts()
            .into_iter()
            .filter(|chart| match chart.chart_type() {
                ChartType::Counter => counters_unknown.remove(chart.name()),
                ChartType::Line => lines_unknown.remove(chart.name()),
            })
            .filter_map(|chart| {
                let name = chart.name().to_string();
                settings.get(&name).map(|settings| {
                    let info = ChartInfo {
                        chart,
                        settings: settings.to_owned(),
                    };
                    (name.to_string(), info)
                })
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
            charts_info,
            counters_filter,
            lines_filter,
        })
    }

    fn remove_disabled_charts(config: Config) -> Config {
        let counters = config
            .counters
            .into_iter()
            .filter(|info| info.settings.enabled)
            .collect();
        let lines = config
            .lines
            .sections
            .into_iter()
            .map(|sec| LineChartSection {
                id: sec.id,
                title: sec.title,
                charts: sec
                    .charts
                    .into_iter()
                    .filter(|info| info.settings.enabled)
                    .collect(),
            })
            .filter(|sec| !sec.charts.is_empty())
            .collect::<Vec<_>>()
            .into();
        Config { counters, lines }
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
        let verified_contracts_growth = Arc::new(lines::VerifiedContractsGrowth::new(
            new_verified_contracts.clone(),
        ));
        let new_contracts = Arc::new(lines::NewContracts::default());
        let contracts_growth = Arc::new(lines::ContractsGrowth::new(new_contracts.clone()));

        vec![
            // tier 1
            Arc::new(counters::TotalAddresses::default()),
            Arc::new(lines::AverageBlockRewards::default()),
            Arc::new(counters::TotalTokens::default()),
            Arc::new(lines::NativeCoinSupply::default()),
            native_coin_holders_growth.clone(),
            new_txns.clone(),
            Arc::new(lines::NewAccounts::new(accounts_cache.clone())),
            new_verified_contracts.clone(),
            new_contracts.clone(),
            new_native_coin_transfers.clone(),
            Arc::new(lines::NewBlocks::default()),
            Arc::new(lines::GasUsedGrowth::default()),
            Arc::new(lines::AverageBlockSize::default()),
            Arc::new(counters::TotalBlocks::default()),
            Arc::new(lines::TxnsFee::default()),
            Arc::new(lines::AverageGasLimit::default()),
            Arc::new(counters::AverageBlockTime::default()),
            Arc::new(lines::ActiveAccounts::default()),
            Arc::new(lines::AverageGasPrice::default()),
            Arc::new(lines::AverageTxnFee::default()),
            Arc::new(lines::TxnsSuccessRate::default()),
            Arc::new(counters::CompletedTxns::default()),
            Arc::new(lines::AccountsGrowth::new(accounts_cache.clone())),
            Arc::new(counters::TotalAccounts::new(accounts_cache)),
            // tier 2
            Arc::new(counters::LastNewContracts::new(new_contracts)),
            Arc::new(counters::TotalNativeCoinHolders::new(
                native_coin_holders_growth.clone(),
            )),
            Arc::new(lines::NewNativeCoinHolders::new(native_coin_holders_growth)),
            Arc::new(counters::TotalTxns::new(new_txns.clone())),
            Arc::new(lines::TxnsGrowth::new(new_txns)),
            contracts_growth.clone(),
            verified_contracts_growth.clone(),
            Arc::new(counters::LastNewVerifiedContracts::new(
                new_verified_contracts,
            )),
            Arc::new(counters::TotalNativeCoinTransfers::new(
                new_native_coin_transfers,
            )),
            // tier 3
            Arc::new(counters::TotalContracts::new(contracts_growth)),
            Arc::new(counters::TotalVerifiedContracts::new(
                verified_contracts_growth,
            )),
        ]
    }
}
