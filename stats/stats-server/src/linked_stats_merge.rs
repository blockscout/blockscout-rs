use std::collections::HashSet;

use stats_proto::blockscout::stats::v1 as proto_v1;

pub fn merge_counters(
    primary: proto_v1::Counters,
    secondary: proto_v1::Counters,
) -> proto_v1::Counters {
    let mut seen_ids: HashSet<_> = primary
        .counters
        .iter()
        .map(|counter| counter.id.clone())
        .collect();
    let mut counters = primary.counters;
    counters.extend(
        secondary
            .counters
            .into_iter()
            .filter(|counter| seen_ids.insert(counter.id.clone())),
    );
    proto_v1::Counters { counters }
}

pub fn merge_line_chart_sections(
    primary: Vec<proto_v1::LineChartSection>,
    secondary: Vec<proto_v1::LineChartSection>,
) -> Vec<proto_v1::LineChartSection> {
    let mut sections = primary;
    for secondary_section in secondary {
        if let Some(primary_section) = sections
            .iter_mut()
            .find(|section| section.id == secondary_section.id)
        {
            let mut seen_chart_ids: HashSet<_> = primary_section
                .charts
                .iter()
                .map(|chart| chart.id.clone())
                .collect();
            primary_section.charts.extend(
                secondary_section
                    .charts
                    .into_iter()
                    .filter(|chart| seen_chart_ids.insert(chart.id.clone())),
            );
        } else {
            sections.push(secondary_section);
        }
    }
    sections
}

pub fn merge_main_page_stats(
    primary: proto_v1::MainPageStats,
    secondary: proto_v1::MainPageStats,
) -> proto_v1::MainPageStats {
    proto_v1::MainPageStats {
        average_block_time: primary.average_block_time.or(secondary.average_block_time),
        total_addresses: primary.total_addresses.or(secondary.total_addresses),
        total_blocks: primary.total_blocks.or(secondary.total_blocks),
        total_transactions: primary.total_transactions.or(secondary.total_transactions),
        yesterday_transactions: primary
            .yesterday_transactions
            .or(secondary.yesterday_transactions),
        total_operational_transactions: primary
            .total_operational_transactions
            .or(secondary.total_operational_transactions),
        yesterday_operational_transactions: primary
            .yesterday_operational_transactions
            .or(secondary.yesterday_operational_transactions),
        op_stack_total_operational_transactions: primary
            .op_stack_total_operational_transactions
            .or(secondary.op_stack_total_operational_transactions),
        op_stack_yesterday_operational_transactions: primary
            .op_stack_yesterday_operational_transactions
            .or(secondary.op_stack_yesterday_operational_transactions),
        daily_new_transactions: primary
            .daily_new_transactions
            .or(secondary.daily_new_transactions),
        daily_new_operational_transactions: primary
            .daily_new_operational_transactions
            .or(secondary.daily_new_operational_transactions),
        op_stack_daily_new_operational_transactions: primary
            .op_stack_daily_new_operational_transactions
            .or(secondary.op_stack_daily_new_operational_transactions),
    }
}

pub fn merge_transactions_page_stats(
    primary: proto_v1::TransactionsPageStats,
    secondary: proto_v1::TransactionsPageStats,
) -> proto_v1::TransactionsPageStats {
    proto_v1::TransactionsPageStats {
        pending_transactions_30m: primary
            .pending_transactions_30m
            .or(secondary.pending_transactions_30m),
        transactions_fee_24h: primary
            .transactions_fee_24h
            .or(secondary.transactions_fee_24h),
        average_transactions_fee_24h: primary
            .average_transactions_fee_24h
            .or(secondary.average_transactions_fee_24h),
        transactions_24h: primary.transactions_24h.or(secondary.transactions_24h),
        operational_transactions_24h: primary
            .operational_transactions_24h
            .or(secondary.operational_transactions_24h),
        op_stack_operational_transactions_24h: primary
            .op_stack_operational_transactions_24h
            .or(secondary.op_stack_operational_transactions_24h),
        total_zetachain_cross_chain_txns: primary
            .total_zetachain_cross_chain_txns
            .or(secondary.total_zetachain_cross_chain_txns),
        new_zetachain_cross_chain_txns_24h: primary
            .new_zetachain_cross_chain_txns_24h
            .or(secondary.new_zetachain_cross_chain_txns_24h),
        pending_zetachain_cross_chain_txns: primary
            .pending_zetachain_cross_chain_txns
            .or(secondary.pending_zetachain_cross_chain_txns),
    }
}

pub fn merge_contracts_page_stats(
    primary: proto_v1::ContractsPageStats,
    secondary: proto_v1::ContractsPageStats,
) -> proto_v1::ContractsPageStats {
    proto_v1::ContractsPageStats {
        total_contracts: primary.total_contracts.or(secondary.total_contracts),
        new_contracts_24h: primary.new_contracts_24h.or(secondary.new_contracts_24h),
        total_verified_contracts: primary
            .total_verified_contracts
            .or(secondary.total_verified_contracts),
        new_verified_contracts_24h: primary
            .new_verified_contracts_24h
            .or(secondary.new_verified_contracts_24h),
    }
}

pub fn merge_main_page_multichain_stats(
    primary: proto_v1::MainPageMultichainStats,
    secondary: proto_v1::MainPageMultichainStats,
) -> proto_v1::MainPageMultichainStats {
    proto_v1::MainPageMultichainStats {
        total_multichain_txns: primary
            .total_multichain_txns
            .or(secondary.total_multichain_txns),
        total_multichain_addresses: primary
            .total_multichain_addresses
            .or(secondary.total_multichain_addresses),
        yesterday_txns_multichain: primary
            .yesterday_txns_multichain
            .or(secondary.yesterday_txns_multichain),
        new_txns_multichain_window: primary
            .new_txns_multichain_window
            .or(secondary.new_txns_multichain_window),
    }
}

pub fn merge_main_page_interchain_stats(
    primary: proto_v1::MainPageInterchainStats,
    secondary: proto_v1::MainPageInterchainStats,
) -> proto_v1::MainPageInterchainStats {
    proto_v1::MainPageInterchainStats {
        total_interchain_messages: primary
            .total_interchain_messages
            .or(secondary.total_interchain_messages),
        total_interchain_messages_sent: primary
            .total_interchain_messages_sent
            .or(secondary.total_interchain_messages_sent),
        total_interchain_messages_received: primary
            .total_interchain_messages_received
            .or(secondary.total_interchain_messages_received),
    }
}

pub fn merge_update_statuses(
    primary: proto_v1::UpdateStatus,
    secondary: proto_v1::UpdateStatus,
) -> proto_v1::UpdateStatus {
    proto_v1::UpdateStatus {
        all_status: min_status(primary.all_status, secondary.all_status),
        independent_status: min_status(primary.independent_status, secondary.independent_status),
        blocks_dependent_status: min_status(
            primary.blocks_dependent_status,
            secondary.blocks_dependent_status,
        ),
        internal_transactions_dependent_status: min_status(
            primary.internal_transactions_dependent_status,
            secondary.internal_transactions_dependent_status,
        ),
        user_ops_dependent_status: min_status(
            primary.user_ops_dependent_status,
            secondary.user_ops_dependent_status,
        ),
        zetachain_cctx_dependent_status: min_status(
            primary.zetachain_cctx_dependent_status,
            secondary.zetachain_cctx_dependent_status,
        ),
    }
}

fn min_status(primary: i32, secondary: i32) -> i32 {
    let primary = proto_v1::ChartSubsetUpdateStatus::try_from(primary)
        .unwrap_or(proto_v1::ChartSubsetUpdateStatus::Pending);
    let secondary = proto_v1::ChartSubsetUpdateStatus::try_from(secondary)
        .unwrap_or(proto_v1::ChartSubsetUpdateStatus::Pending);
    if status_rank(primary) <= status_rank(secondary) {
        primary.into()
    } else {
        secondary.into()
    }
}

fn status_rank(status: proto_v1::ChartSubsetUpdateStatus) -> u8 {
    match status {
        proto_v1::ChartSubsetUpdateStatus::Pending => 0,
        proto_v1::ChartSubsetUpdateStatus::WaitingForStartingCondition => 1,
        proto_v1::ChartSubsetUpdateStatus::QueuedForInitialUpdate => 2,
        proto_v1::ChartSubsetUpdateStatus::RunningInitialUpdate => 3,
        proto_v1::ChartSubsetUpdateStatus::CompletedInitialUpdate => 4,
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn counter(id: &str, value: &str) -> proto_v1::Counter {
        proto_v1::Counter {
            id: id.to_string(),
            value: value.to_string(),
            title: id.to_string(),
            units: None,
            description: id.to_string(),
        }
    }

    fn line_info(id: &str) -> proto_v1::LineChartInfo {
        proto_v1::LineChartInfo {
            id: id.to_string(),
            title: id.to_string(),
            description: id.to_string(),
            units: None,
            resolutions: vec!["DAY".to_string()],
        }
    }

    #[test]
    fn merge_counters_keeps_primary_order_and_values() {
        let merged = merge_counters(
            proto_v1::Counters {
                counters: vec![counter("a", "1"), counter("b", "2")],
            },
            proto_v1::Counters {
                counters: vec![counter("b", "20"), counter("c", "3")],
            },
        );

        assert_eq!(
            merged
                .counters
                .into_iter()
                .map(|counter| counter.id)
                .collect::<Vec<_>>(),
            vec!["a", "b", "c"]
        );
    }

    #[test]
    fn merge_line_chart_sections_keeps_primary_section_title_and_chart_order() {
        let merged = merge_line_chart_sections(
            vec![proto_v1::LineChartSection {
                id: "transactions".to_string(),
                title: "Primary".to_string(),
                charts: vec![line_info("a"), line_info("b")],
            }],
            vec![
                proto_v1::LineChartSection {
                    id: "transactions".to_string(),
                    title: "Secondary".to_string(),
                    charts: vec![line_info("b"), line_info("c")],
                },
                proto_v1::LineChartSection {
                    id: "other".to_string(),
                    title: "Other".to_string(),
                    charts: vec![line_info("d")],
                },
            ],
        );

        assert_eq!(merged[0].title, "Primary");
        assert_eq!(
            merged[0]
                .charts
                .iter()
                .map(|chart| chart.id.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b", "c"]
        );
        assert_eq!(merged[1].id, "other");
    }

    #[test]
    fn merge_update_statuses_uses_explicit_status_order() {
        let merged = merge_update_statuses(
            proto_v1::UpdateStatus {
                all_status: proto_v1::ChartSubsetUpdateStatus::CompletedInitialUpdate.into(),
                independent_status: proto_v1::ChartSubsetUpdateStatus::QueuedForInitialUpdate
                    .into(),
                blocks_dependent_status: proto_v1::ChartSubsetUpdateStatus::RunningInitialUpdate
                    .into(),
                internal_transactions_dependent_status:
                    proto_v1::ChartSubsetUpdateStatus::CompletedInitialUpdate.into(),
                user_ops_dependent_status:
                    proto_v1::ChartSubsetUpdateStatus::CompletedInitialUpdate.into(),
                zetachain_cctx_dependent_status:
                    proto_v1::ChartSubsetUpdateStatus::CompletedInitialUpdate.into(),
            },
            proto_v1::UpdateStatus {
                all_status: proto_v1::ChartSubsetUpdateStatus::RunningInitialUpdate.into(),
                independent_status: proto_v1::ChartSubsetUpdateStatus::CompletedInitialUpdate
                    .into(),
                blocks_dependent_status:
                    proto_v1::ChartSubsetUpdateStatus::WaitingForStartingCondition.into(),
                internal_transactions_dependent_status:
                    proto_v1::ChartSubsetUpdateStatus::CompletedInitialUpdate.into(),
                user_ops_dependent_status: proto_v1::ChartSubsetUpdateStatus::Pending.into(),
                zetachain_cctx_dependent_status:
                    proto_v1::ChartSubsetUpdateStatus::CompletedInitialUpdate.into(),
            },
        );

        assert_eq!(
            proto_v1::ChartSubsetUpdateStatus::try_from(merged.all_status).unwrap(),
            proto_v1::ChartSubsetUpdateStatus::RunningInitialUpdate
        );
        assert_eq!(
            proto_v1::ChartSubsetUpdateStatus::try_from(merged.independent_status).unwrap(),
            proto_v1::ChartSubsetUpdateStatus::QueuedForInitialUpdate
        );
        assert_eq!(
            proto_v1::ChartSubsetUpdateStatus::try_from(merged.blocks_dependent_status).unwrap(),
            proto_v1::ChartSubsetUpdateStatus::WaitingForStartingCondition
        );
        assert_eq!(
            proto_v1::ChartSubsetUpdateStatus::try_from(merged.user_ops_dependent_status).unwrap(),
            proto_v1::ChartSubsetUpdateStatus::Pending
        );
    }
}
