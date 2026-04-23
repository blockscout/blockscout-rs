use crate::{
    clients::stats::{
        counters::GetCounters,
        lines::{GetLineChart, GetLineChartParams, LineChart, Resolution},
    },
    error::{ParseError, ServiceError},
    types::{
        ChainId,
        chain_metrics::{ChainMetrics, WeeklyMetric},
    },
};
use api_client_framework::HttpApiClient;
use chrono::{Duration, Utc};
use std::{collections::BTreeMap, sync::Arc};

const SECONDS_IN_DAY: f64 = 86_400.0;
const DAILY_TXS_COUNTER_ID: &str = "newTxns24h";
const NEW_ACCOUNTS_CHART_ID: &str = "newAccounts";
const NEW_TXS_CHART_ID: &str = "newTxns";
const ACTIVE_ACCOUNTS_CHART_ID: &str = "activeAccounts";

pub async fn fetch_chain_metrics(
    blockscout_clients: &Arc<BTreeMap<ChainId, Arc<HttpApiClient>>>,
    chain_ids: &[ChainId],
) -> Vec<ChainMetrics> {
    let futures = chain_ids.iter().map(|chain_id| {
        let client = blockscout_clients.get(chain_id).cloned();
        async move {
            if let Some(client) = client {
                fetch_single_chain_metrics(*chain_id, &client).await
            } else {
                ChainMetrics {
                    chain_id: *chain_id,
                    tps: None,
                    new_addresses: None,
                    daily_transactions: None,
                    active_accounts: None,
                }
            }
        }
    });

    futures::future::join_all(futures).await
}

#[tracing::instrument(skip(client))]
async fn fetch_single_chain_metrics(chain_id: ChainId, client: &HttpApiClient) -> ChainMetrics {
    let (tps, new_addresses, daily_transactions, active_accounts) = futures::join!(
        fetch_tps(client),
        fetch_weekly_metric(client, NEW_ACCOUNTS_CHART_ID),
        fetch_weekly_metric(client, NEW_TXS_CHART_ID),
        fetch_weekly_metric(client, ACTIVE_ACCOUNTS_CHART_ID),
    );

    ChainMetrics {
        chain_id,
        tps: tps.ok(),
        new_addresses: new_addresses.ok(),
        daily_transactions: daily_transactions.ok(),
        active_accounts: active_accounts.ok(),
    }
}

#[tracing::instrument(skip(client), err)]
async fn fetch_tps(client: &HttpApiClient) -> Result<f64, ServiceError> {
    let counters = client
        .request(&GetCounters {})
        .await
        .map_err(ServiceError::from)?;

    let new_txns_24h = counters
        .counters
        .iter()
        .find(|c| c.id == DAILY_TXS_COUNTER_ID)
        .ok_or(ServiceError::Internal(anyhow::anyhow!("counter not found")))?;

    let txns_24h = new_txns_24h
        .value
        .parse::<i64>()
        .map_err(ParseError::from)?;

    let tps = txns_24h as f64 / SECONDS_IN_DAY;
    Ok(tps)
}

#[tracing::instrument(skip(client), err)]
async fn fetch_weekly_metric(
    client: &HttpApiClient,
    chart_name: &str,
) -> Result<WeeklyMetric, ServiceError> {
    let now = Utc::now().date_naive();
    // We need to fetch at least 2 points which cover whole weeks.
    // In worst case, that is 7+7+6=20 days.
    let from = now - Duration::days(21);

    let params = GetLineChartParams {
        from: from.format("%Y-%m-%d").to_string(),
        to: now.format("%Y-%m-%d").to_string(),
        resolution: Resolution::Week,
    };

    let line_chart = client
        .request(&GetLineChart {
            name: chart_name.to_string(),
            params,
        })
        .await
        .map_err(|e| {
            ServiceError::Internal(anyhow::anyhow!(
                "failed to fetch line chart {}: {}",
                chart_name,
                e
            ))
        })?;

    calculate_weekly_metric(&line_chart)
}

fn calculate_weekly_metric(line_chart: &LineChart) -> Result<WeeklyMetric, ServiceError> {
    let full_week_points = &line_chart
        .chart
        .iter()
        .filter(|p| !p.is_approximate.unwrap_or(false))
        .collect::<Vec<_>>();

    if full_week_points.is_empty() {
        return Ok(WeeklyMetric::default());
    }

    let (previous_full_week, current_full_week) = match full_week_points.as_slice() {
        [.., previous, current] => (
            previous.value.parse().map_err(ParseError::from)?,
            current.value.parse().map_err(ParseError::from)?,
        ),
        _ => {
            return Err(ServiceError::Internal(anyhow::anyhow!(
                "expected at least 2 data points, got {}",
                full_week_points.len()
            )));
        }
    };

    let wow_diff_percent = calculate_wow_diff(current_full_week, previous_full_week);

    Ok(WeeklyMetric {
        current_full_week,
        previous_full_week,
        wow_diff_percent,
    })
}

fn calculate_wow_diff(current: i64, previous: i64) -> f64 {
    if previous == 0 {
        if current > 0 {
            return 100.0;
        }
        return 0.0;
    }

    ((current - previous) as f64 / previous as f64) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(100, 50, 100.0)]
    #[case(50, 100, -50.0)]
    #[case(100, 100, 0.0)]
    #[case(100, 0, 100.0)]
    #[case(0, 0, 0.0)]
    #[case(1000, 500, 100.0)]
    fn test_calculate_wow_diff(#[case] current: i64, #[case] previous: i64, #[case] expected: f64) {
        assert_eq!(calculate_wow_diff(current, previous), expected);
    }
}
