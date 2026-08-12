# Graph Report - interchain-indexer  (2026-08-12)

## Corpus Check
- 239 files · ~231,366 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 3341 nodes · 7920 edges · 203 communities (176 shown, 27 thin omitted)
- Extraction: 93% EXTRACTED · 7% INFERRED · 0% AMBIGUOUS · INFERRED: 554 edges (avg confidence: 0.83)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `154a9195`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- Key
- provider_layers.rs
- StatsService
- fill_mock_interchain_database
- persistence.rs
- indexed_chains.rs
- InterchainDatabase
- init_db
- amb/abi.rs
- .new
- Result
- ExampleIndexer
- projection.rs
- env_merge.rs
- config.rs
- log_stream.rs
- ChainInfoService
- BlockscoutTokenInfoClient
- bridged_tokens_query.rs
- list_stats_chains
- .new
- ADR-007: The Scan Floor Is Reconciled Against The Checkpoint, Not Against bridge_contracts (Accepted, with stated expiry)
- filters.rs
- InterchainServiceImpl
- TestRangeProcessor
- indexers.rs
- Avalanche Bridge Filtering
- avalanche/consolidation.rs
- Settings
- cursor.rs
- avalanche/mod.rs
- Interchain Indexer Service README
- .new
- Status
- AmbIndexer
- ictt_payload.rs
- TokenTransfer
- TokenInfoService
- services/utils.rs
- Codex Skill: implementation-plan
- avalanche_e2e.rs
- Codex Skill: gh-issue-publish
- Glossary
- BridgeConfig
- InterchainStatisticsService
- package.json
- .get_crosschain_transfers
- bridge_contracts Is a Proxy, Not the Membership Set
- spawn_configured_indexers
- helpers/mod.rs
- BridgeCounts
- amb/indexer.rs
- .new
- FailureLedger
- progress.rs
- TokenInfoService and Token Metadata Enrichment Flow
- InterchainService
- MessageBuffer (Tiered Storage)
- .fetch_token_info
- Rust Style Rules
- range_driver.rs
- BatchError
- Stats Projection
- try_reconstruct_transfer
- SourceData
- AvalancheIndexer
- failure_ledger/settings.rs
- Expected Skips Inside a Shared Transaction
- BlockRange
- policy.rs
- run
- BufferItem
- workflows/ Tool-Agnostic Task Procedures
- Memory Bank
- interchain-indexer service
- Layer 1: Generic Pipeline
- AvalancheDataApiClient
- AmbDispatchMockService
- prelude.rs
- Checkpoint
- Database Subsystem: Schema and DB Interaction Layer
- Project Context
- Architectural Decision Records Index
- BridgeType
- fetch_receipts_for_transactions
- Codex Task Analysis Skill
- compile
- bridge_model_to_proto
- MockLogsService
- BridgeContractConfig
- stats.rs
- .check
- Model
- BridgedTokensSortField
- Interchain Indexer
- from_sql
- ADR-004: Observability Horizon and Asset Union-Find
- Event-Derived AMB Transfer Reconstruction
- FailureLedger
- BlockchainIdResolver
- AmbIndexerSettings
- src/utils.rs
- Indexing Gaps, Retries, and Checkpoint Safety
- Model
- Layer 2: Avalanche Reference Realization
- AvalancheRangeProcessor
- Testing Rules
- Model
- .record_indexer_failures
- AvalancheIndexerSettings
- BlockscoutTokenInfoClientSettings
- Migration
- Migration
- Migration
- Migration
- Tiered Message Buffer Storage
- MessageStatus
- AbiRegistry
- BufferItem<T>
- Model
- Model
- MaintenancePlan
- unindexed_chain_filter.rs
- maintenance.rs
- group_logs_by_transaction
- v1ChainIndexingProgress
- CrosschainIndexer Worker
- Model
- Model
- Model
- Model
- Model
- Asset Identity as Union-Find
- ApiError
- buffer_item.rs
- stats_asset_tokens.rs
- Gotcha: PostgreSQL Bind Parameter Limit (65535 per statement)
- Gotcha: Token Info Is Eventually Consistent and Reads Can Write Back
- chain_model_to_proto
- is_tmp_mkdir_command
- is_tmp_path
- Entity
- Entity
- Entity
- Entity
- Entity
- Entity
- Entity
- Entity
- Entity
- Entity
- Entity
- Entity
- .recompute_stats_chains
- Entity
- Entity
- Entity
- indexer_checkpoints::Model
- bigdecimal_rename.sh
- Gotcha: Config Env Overrides — Null Replaces, JSON Quoting, Zero-Padded Numbers
- Gotcha: Filter Params Must Not Reuse Pagination Cursor Field Names
- ActiveModel
- ActiveModel
- ActiveModel
- ActiveModel
- ActiveModel
- ActiveModel
- ActiveModel
- ActiveModel
- ActiveModel
- ActiveModel
- ActiveModel
- ActiveModel
- ActiveModel
- ActiveModel
- ActiveModel
- ActiveModel
- ActiveModel
- ActiveModel
- ActiveModel
- bridge_contracts::Model
- gh-issue-publish.sh
- update_migration.sh
- Gotcha: Indexer Cleanup Guard Runs on Panic (IndexerCleanupGuard Drop)

## God Nodes (most connected - your core abstractions)
1. `init_db()` - 231 edges
2. `InterchainDatabase` - 112 edges
3. `fill_mock_interchain_database()` - 83 edges
4. `seed_minimal_bridge()` - 51 edges
5. `Key` - 43 edges
6. `IndexedChains` - 39 edges
7. `TokenInfoService` - 37 edges
8. `list_bridged_token_stats_for_chain()` - 36 edges
9. `seed_bridge_row()` - 34 edges
10. `list_stats_chains()` - 34 edges

## Surprising Connections (you probably didn't know these)
- `Cursor Task Analysis Skill` --semantically_similar_to--> `Codex Task Analysis Skill`  [INFERRED] [semantically similar]
  .cursor/skills/task-analysis/SKILL.md → .codex/skills/task-analysis/SKILL.md
- `Claude Skill: implementation-plan` --semantically_similar_to--> `Codex Skill: implementation-plan`  [INFERRED] [semantically similar]
  .claude/skills/implementation-plan/SKILL.md → .codex/skills/implementation-plan/SKILL.md
- `Claude Skill: gh-issue-bug` --semantically_similar_to--> `Codex Skill: gh-issue-bug`  [INFERRED] [semantically similar]
  .claude/skills/gh-issue-bug/SKILL.md → .codex/skills/gh-issue-bug/SKILL.md
- `Claude Skill: gh-issue-publish` --semantically_similar_to--> `Codex Skill: gh-issue-publish`  [INFERRED] [semantically similar]
  .claude/skills/gh-issue-publish/SKILL.md → .codex/skills/gh-issue-publish/SKILL.md
- `Claude Skill: research-scope` --semantically_similar_to--> `Codex Skill: research-scope`  [INFERRED] [semantically similar]
  .claude/skills/research-scope/SKILL.md → .codex/skills/research-scope/SKILL.md

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **Dual-Harness Thin Wrappers over One Canonical Workflow Set** — _memory_bank_workflows, _claude_skills_implementation_plan_skill, _codex_skills_implementation_plan_skill, _codex_skills_implementation_plan_agents_openai, _codex_skills_gh_issue_bug_canonical_workflow_delegation, _claude_claude_claude_code_overrides [INFERRED 0.95]
- **Scan-floor and checkpoint semantics: what a checkpoint certifies, what catchup_min_cursor is, how a floor change behaves, and how ADR-007 reconciles it** — _memory_bank_gotchas_checkpoint_certifies_scanning_not_correctness, _memory_bank_gotchas_catchup_min_cursor_is_stored_floor, _memory_bank_gotchas_lowering_scan_floor_no_rescan, _memory_bank_gotchas_one_scanner_per_bridge_chain, _memory_bank_gotchas_amb_scan_floor_is_amb_proxy_started_at_block, _memory_bank_adr_007_scan_floor_reconciled_against_the_checkpoint_adr_007, _memory_bank_glossary_checkpoint [EXTRACTED 1.00]
- **tmp/tasks Lifecycle Pipeline: analysis to plan to code to review to PR** — _claude_skills_task_analysis_skill, _claude_skills_implementation_plan_skill, _claude_skills_task_to_code_skill, _claude_skills_solution_review_skill, _claude_skills_pr_description_skill, _claude_skills_task_analysis_task_folder_artifacts [EXTRACTED 1.00]
- **docker-compose Full Stack Topology** — docker_docker_compose_db, docker_docker_compose_backend, docker_docker_compose_interchain_indexer, docker_docker_compose_stats_interchain, docker_docker_compose_stats, docker_docker_compose_frontend, docker_docker_compose_caddy [EXTRACTED 1.00]
- **Maintenance Commit Transaction (offload, flush, project, checkpoint)** — _memory_bank_research_message_lifecycle_maintenance_transaction, _memory_bank_research_message_lifecycle_buffer_alter, _memory_bank_research_message_lifecycle_consolidate_contract, _memory_bank_research_stats_projection_apply_stats_for_flushed_batch, _memory_bank_research_message_lifecycle_cursor_semantics, _memory_bank_research_db_schema_and_layer_canonical_write_path [EXTRACTED 1.00]
- **GitHub Issue Draft-then-Publish Pipeline via tmp/gh-issues** — _codex_skills_gh_issue_bug_skill, _codex_skills_gh_issue_improvement_skill, _codex_skills_gh_issue_publish_skill, _codex_skills_gh_issue_bug_tmp_gh_issues_convention, _memory_bank_workflows_scripts_gh_issue_publish_sh [EXTRACTED 1.00]
- **Indexing Completeness: batch, ledger, driver, checkpoint** — _memory_bank_adr_005_failed_range_ledger_and_checkpoint_independence_log_batch, _memory_bank_adr_005_failed_range_ledger_and_checkpoint_independence_failure_ledger, _memory_bank_adr_005_failed_range_ledger_and_checkpoint_independence_range_driver, _memory_bank_adr_005_failed_range_ledger_and_checkpoint_independence_range_processor, _memory_bank_adr_005_failed_range_ledger_and_checkpoint_independence_checkpoint_independence [EXTRACTED 1.00]
- **Partial Data Handling: nullable sides, horizon eligibility, union-find identity** — _memory_bank_adr_003_amb_event_based_transfers_nullable_transfer_sides, _memory_bank_adr_004_stats_observability_horizon_and_asset_union_find_observability_horizon, _memory_bank_adr_004_stats_observability_horizon_and_asset_union_find_asset_union_find, _memory_bank_adr_004_stats_observability_horizon_and_asset_union_find_indexer_contract, _memory_bank_adr_002_primary_chain_filtering_process_unknown_chains [INFERRED 0.85]
- **Stats Projection Integrity Verification Suite** — _memory_bank_runbooks_runtime_verification_query_c_hard_invariants, _memory_bank_runbooks_runtime_verification_query_a_split_asset_detector, _memory_bank_runbooks_runtime_verification_query_b_refusal_legitimacy, _memory_bank_runbooks_runtime_verification_query_d_deferred_transfers, _memory_bank_runbooks_runtime_verification_query_e_unindexed_chain_edges, _memory_bank_runbooks_runtime_verification_observability_horizon, _memory_bank_runbooks_runtime_verification_asset_union_find_merge, readme_stats_processed_markers [EXTRACTED 1.00]
- **Observability Horizon Eligibility (projection, backfill, read filter)** — _memory_bank_research_stats_subsystem_indexed_chains, _memory_bank_research_stats_subsystem_observability_horizon_rule, _memory_bank_research_stats_projection_message_countable_condition, _memory_bank_research_stats_subsystem_unindexed_chain_read_filter, _memory_bank_adr_readme_adr_004 [EXTRACTED 1.00]
- **AMB out-of-order stream merge semantics: per-side persistence, nullable columns, replacement on collision, and correct chain/sender attribution** — _memory_bank_gotchas_amb_events_out_of_order, _memory_bank_gotchas_amb_transfer_sides_nullable_never_mirrored, _memory_bank_gotchas_amb_collision_replacement_delete_before_insert, _memory_bank_gotchas_amb_queued_events_preserve_emitting_chain, _memory_bank_gotchas_amb_header_sender_not_source_initiator [EXTRACTED 1.00]
- **Stats projection: one eligibility predicate, observability-based deferral, merge-based asset identity, sticky edge side, and ICTT-address token identity** — _memory_bank_gotchas_stats_eligibility_observability_not_terminality, _memory_bank_gotchas_stats_asset_mapping_conflicts_merge, _memory_bank_gotchas_stats_transfer_backfill_failed_amb_eligibility_resolved, _memory_bank_gotchas_stats_edge_amount_side_follows_source_presence, _memory_bank_gotchas_token_identity_stats_asset_tokens_ictt_address, _memory_bank_gotchas_indexed_chains_may_observe [EXTRACTED 1.00]
- **Agent Task Lifecycle: analysis to plan to code to review to PR** — _cursor_skills_task_analysis_skill_task_analysis, _cursor_skills_implementation_plan_skill_implementation_plan, _cursor_skills_task_to_code_skill_task_to_code, _cursor_skills_solution_review_skill_solution_review, _cursor_skills_pr_description_skill_pr_description, _codex_skills_solution_review_skill_task_folder_artifacts [EXTRACTED 1.00]

## Communities (203 total, 27 thin omitted)

### Community 0 - "Key"
Cohesion: 0.05
Nodes (115): DynSolValue, addr(), amount_to_decimal(), build_destination_only(), build_destination_only_transfer(), build_source_led(), build_transfer(), destination() (+107 more)

### Community 1 - "provider_layers.rs"
Cohesion: 0.06
Nodes (61): Context, DefaultClock, InMemoryState, base_node(), block_number_packet(), build_layered_http_provider(), build_layered_provider_from_services(), build_mock_error_response() (+53 more)

### Community 2 - "StatsService"
Cohesion: 0.12
Nodes (17): MessagePathStatsRow, OutputPagination, P, BridgedTokenListRow, kickoff_enrichment_no_token_service_is_noop(), Arc, DatabaseTransaction, DbErr (+9 more)

### Community 3 - "fill_mock_interchain_database"
Cohesion: 0.10
Nodes (45): default_filter(), indexer_failures_and_mark_catchup_complete_are_independent_records(), list_indexer_checkpoints_filters_and_orders_deterministically(), lower_catchup_floor_is_idempotent_and_never_raises_or_touches_max_cursor(), mark_catchup_complete_upserts_empty_range_checkpoint(), mark_catchup_complete_without_safe_realtime_cursor_does_not_insert(), message_ids(), mock_indexed_chains() (+37 more)

### Community 4 - "persistence.rs"
Cohesion: 0.07
Nodes (70): A, batch_size_for_width(), batched_upsert(), ConnectionTrait, DbErr, F, OnConflict, Result (+62 more)

### Community 5 - "indexed_chains.rs"
Cohesion: 0.06
Nodes (48): Column, Entity, chain_unindexed_condition(), cols(), IndexedChains, message_countable_condition(), render(), Clone (+40 more)

### Community 6 - "InterchainDatabase"
Cohesion: 0.07
Nodes (37): Fn, BackfillStatsReport, build_all_time_message_paths_query(), build_bounded_message_paths_query(), CrosschainMessageLookup, expect_found(), get_crosschain_message_native_collision_is_ambiguous_until_qualified(), get_crosschain_message_numeric_collision_is_ambiguous_until_qualified() (+29 more)

### Community 7 - "init_db"
Cohesion: 0.10
Nodes (65): completed_message(), completed_message_at(), completed_message_without_indexed_source(), insert_already_processed_bridging_transfer(), message_paths_invalid_or_empty_range_returns_empty(), ActiveModel, DatabaseConnection, RwLock (+57 more)

### Community 8 - "amb/abi.rs"
Cohesion: 0.07
Nodes (47): AbiRegistry, amb_side_for_abi(), amb_side_for_abi_infers_side_from_configured_event_set(), ContractAbi, ContractKind, ContractVersion, event_abi(), filter_for_chain_unions_topics_across_versions() (+39 more)

### Community 9 - ".new"
Cohesion: 0.06
Nodes (51): ChainIndexingProgress, FailureTotalsResult, FullStatus, GaugeValue, GetFullStatusRequest, GetIndexingProgressRequest, GetIndexingProgressResponse, GetStatusRequest (+43 more)

### Community 10 - "Result"
Cohesion: 0.10
Nodes (22): BridgedTokensListPagination, build_pagination_from_messages(), BridgedTokensPaginationLogic, ListMarker, MessagesPaginationLogic, OutputPagination<P>, PaginationDirection, Default (+14 more)

### Community 11 - "ExampleIndexer"
Cohesion: 0.05
Nodes (38): CleanupGuard, Arc, AtomicBool, Drop, JoinHandle, Option, RwLock, CrosschainIndexerState (+30 more)

### Community 12 - "projection.rs"
Cohesion: 0.12
Nodes (50): EdgeKey, EdgeAmountSide, Model, Relation, BigDecimal, DateTime, Option, asset_has_token_on_chain() (+42 more)

### Community 13 - "env_merge.rs"
Cohesion: 0.13
Nodes (51): AppliedOverride, apply(), apply_env_overrides(), apply_patch(), apply_to_keyed_array(), apply_to_named_map_array(), ArrayRule, ArrayRules (+43 more)

### Community 14 - "config.rs"
Cohesion: 0.08
Nodes (33): D, collect_json_files(), deserialize_abi(), deserialize_address(), deserialize_bridge_type(), fixture_vars(), load_bridges_from_file(), load_bridges_impl() (+25 more)

### Community 15 - "log_stream.rs"
Cohesion: 0.07
Nodes (34): build_log_stream_for_chain(), BoxStream, Duration, DynProvider, Ethereum, Filter, Result, fetch_logs() (+26 more)

### Community 16 - "ChainInfoService"
Cohesion: 0.08
Nodes (31): Entity, Model, Relation, DateTime, Json, Option, Related, RelationDef (+23 more)

### Community 17 - "BlockscoutTokenInfoClient"
Cohesion: 0.16
Nodes (17): Client, BlockscoutTokenInfo, BlockscoutTokenInfoClient, BlockscoutTokenInfoError, CachedIconResult, Arc, Error, HashMap (+9 more)

### Community 18 - "bridged_tokens_query.rs"
Cohesion: 0.16
Nodes (41): add_asset_edges_on_bridge(), bridged_tokens_aggregation_input_output_total(), bridged_tokens_default_excludes_edge_unindexed_for_its_bridge(), bridged_tokens_empty_configured_pairs_restricts_nothing(), bridged_tokens_last_page(), bridged_tokens_name_sort_nulls_and_empty_last(), bridged_tokens_opt_in_returns_same_rows_until_projection_widens(), bridged_tokens_pagination_after_bridge_collapse() (+33 more)

### Community 19 - "list_stats_chains"
Cohesion: 0.13
Nodes (35): build_pagination(), cursor_where_next(), cursor_where_prev(), forward_order_clause(), inverse_order_clause(), list_stats_chains(), ConnectionTrait, DatabaseConnection (+27 more)

### Community 20 - ".new"
Cohesion: 0.14
Nodes (42): load_native_id_map_filters_missing_native_ids(), message_paths_bounded_queries_apply_open_and_half_open_ranges(), message_paths_bounded_queries_sum_daily_rows_and_order_deterministically(), message_paths_default_excludes_pair_unindexed_for_its_bridge(), message_paths_empty_configured_pairs_restricts_nothing(), message_paths_include_zero_bounded_counterparty_expands_requested_known_rows_only(), message_paths_include_zero_bounded_queries_expand_known_chains(), message_paths_include_zero_counterparty_expands_requested_known_rows_only() (+34 more)

### Community 21 - "ADR-007: The Scan Floor Is Reconciled Against The Checkpoint, Not Against bridge_contracts (Accepted, with stated expiry)"
Cohesion: 0.08
Nodes (38): ADR-007: The Scan Floor Is Reconciled Against The Checkpoint, Not Against bridge_contracts (Accepted, with stated expiry), Alternative 1 (rejected): Withhold the contracts upsert when the previous floor is unknown, Alternative 2 (rejected for now, the correct long-term shape): persist the pair's floor in its own column, Alternative 3 (rejected): also lower catchup_max_cursor to the old floor, Neutral consequence: bridge_contracts returns to being purely diagnostic, bridges_pending_contracts_upsert — REMOVED by ADR-007 (evidence-preserving withhold mechanism and its startup coupling), catchup_max_cursor is deliberately untouched — lowering a floor causes no rescan in the current design, ChainPlan::floor_contracts — survives with one consumer, start_block() (+30 more)

### Community 22 - "filters.rs"
Cohesion: 0.10
Nodes (31): messages_where(), Condition, String, sql_messages(), sql_transfers(), test_messages_condition_both_directions_no_focal(), test_messages_condition_bridge_only(), test_messages_condition_counterparties_only() (+23 more)

### Community 23 - "InterchainServiceImpl"
Cohesion: 0.13
Nodes (23): AddressInfo, BridgeInfo, CrosschainMessageModel, CrosschainTransferModel, DbMessageStatus, GetMessageDetailsRequest, hex_string_opt(), Option (+15 more)

### Community 24 - "TestRangeProcessor"
Cohesion: 0.12
Nodes (24): AtomicUsize, empty_batch(), escalates_and_stops_consuming_when_record_keeps_failing(), healthy_path_issues_no_ledger_write_statement(), mock_provider(), RangeProcessor, records_a_failed_batch_and_resolves_it_on_a_later_success(), replaying_an_already_persisted_interval_leaves_the_message_row_intact() (+16 more)

### Community 25 - "indexers.rs"
Cohesion: 0.19
Nodes (24): checkpoint_floor(), enumerate_indexing_targets(), init_db(), omnibridge_config_path(), progress_for(), reconcile_catchup_floors(), reconcile_catchup_floors_amb_pair_only_resets_on_amb_proxy_lowered(), reconcile_catchup_floors_disabled_bridge_lowered_then_reenabled_ends_up_lowered() (+16 more)

### Community 26 - "Avalanche Bridge Filtering"
Cohesion: 0.10
Nodes (28): Codebase Review, Complexity Hotspots, Onboarding Friction, Recommended Research Priorities, Home Chain, Unknown Chain, Avalanche Data API, External Systems (+20 more)

### Community 27 - "avalanche/consolidation.rs"
Cohesion: 0.26
Nodes (30): Bytes, addr(), call_outcome_transfer(), encode_transferrer(), execution_failed(), execution_succeeded(), key(), message_id() (+22 more)

### Community 28 - "Settings"
Cohesion: 0.05
Nodes (52): ActiveValue, DatabaseSettings, Deserialize, FnOnce, T, set_value(), DummyMessage, MessageBuffer<T> (+44 more)

### Community 29 - "cursor.rs"
Cohesion: 0.16
Nodes (24): FnMut, block_sets_bootstrap_delegates(), block_sets_extend_delegates(), BlockSets, bootstrap(), BootstrapCase, Cursor, cursor_blocks_builder_keys_iterator() (+16 more)

### Community 30 - "avalanche/mod.rs"
Cohesion: 0.22
Nodes (25): gate_receiver_ictt_arm(), handle_log(), handle_message_executed(), handle_message_execution_failed(), handle_receive_cross_chain_message(), handle_send_cross_chain_message(), LogHandleContext, parse_execution_outcome_log() (+17 more)

### Community 31 - "Interchain Indexer Service README"
Cohesion: 0.10
Nodes (25): Config Structs Use deny_unknown_fields, Functional Style for Boolean Logic, pending_messages Cold Storage Retention Is Load-Bearing, Query F — Incoming ICTT Reconstruction Diagnostic, Query G — pending_messages Backlog Trend, reconstruct_incoming_ictt_transfers Kill Switch, database service (UBI stack), interchain-indexer service (UBI stack, built from Dockerfile) (+17 more)

### Community 32 - ".new"
Cohesion: 0.15
Nodes (19): GetBridgesRequest, GetBridgesResponse, GetChainsRequest, GetChainsResponse, GetMessagesByAddressRequest, GetMessagesByTransactionRequest, GetMessagesRequest, GetMessagesResponse (+11 more)

### Community 33 - "Status"
Cohesion: 0.16
Nodes (21): GetBridgedTokensRequest, GetBridgedTokensResponse, GetChainsStatsRequest, GetChainsStatsResponse, GetCommonStatisticsRequest, GetCommonStatisticsResponse, GetDailyStatisticsRequest, GetDailyStatisticsResponse (+13 more)

### Community 34 - "AmbIndexer"
Cohesion: 0.14
Nodes (14): AmbIndexer, Arc, AtomicBool, AtomicU64, Filter, Message, NaiveDateTime, Result (+6 more)

### Community 35 - "ictt_payload.rs"
Cohesion: 0.11
Nodes (21): ictt_completeness(), CreditExpectation, decode_inner(), decode_transferrer_message(), IcttPayload, mainnet_bytes(), PayloadRejection, Result (+13 more)

### Community 36 - "TokenTransfer"
Cohesion: 0.10
Nodes (26): CallFailed, CallSucceeded, AnnotatedEvent, AnnotatedICTTSource, CallOutcome, Message, MessageExecutionOutcome, Address (+18 more)

### Community 37 - "TokenInfoService"
Cohesion: 0.18
Nodes (17): Arc, Box, DateTime, DynProvider, Ethereum, HashMap, HashSet, Mutex (+9 more)

### Community 38 - "services/utils.rs"
Cohesion: 0.12
Nodes (22): build_chain_bridge_filter(), build_chain_bridge_filter_all_indexed_is_none_even_without_opt_in(), build_chain_bridge_filter_default_sets_sorted_pairs(), build_chain_bridge_filter_include_unindexed_true_clears_restriction(), build_chain_bridge_filter_prunes_pairs_to_requested_bridge_ids(), checked_bridge_id(), checked_bridge_id_rejects_above_i32_max(), db_datetime_to_string() (+14 more)

### Community 39 - "Codex Skill: implementation-plan"
Cohesion: 0.05
Nodes (61): Claude Code Overrides (project CLAUDE.md), Handoff Preparation, Not Re-Analysis, Claude Skill: implementation-plan, Reviewer-Facing PR Description (not a changelog), Claude Skill: pr-description, Claude Skill: research-scope, Claude Skill: solution-review, Verification Gap Reporting (+53 more)

### Community 40 - "avalanche_e2e.rs"
Cohesion: 0.20
Nodes (23): ConfigSettings, main(), Error, Result, decode_blockchain_id(), forked_provider(), parse_message_id_from_native_id(), DynProvider (+15 more)

### Community 41 - "Codex Skill: gh-issue-publish"
Cohesion: 0.15
Nodes (24): Hook: allow-tmp-dirs.py (PreToolUse Bash), Hook: allow-tmp-writes.py (PreToolUse Write|Edit), Claude Skill: gh-issue-bug, Claude Skill: gh-issue-improvement, Claude Skill: gh-issue-publish, Codex Agent Interface: GitHub Issue Bug, Bug vs Improvement Issue Separation, Codex Skill: gh-issue-bug (+16 more)

### Community 42 - "Glossary"
Cohesion: 0.10
Nodes (22): Incoming ICTT Reconstruction / ICM Payload Decoding Entrypoints, Bridge, Bridge Contract, Configured Chain, Consolidation, Cross-Chain Message, Cross-Chain Transfer, Destination-Indexed Data (+14 more)

### Community 43 - "BridgeConfig"
Cohesion: 0.16
Nodes (19): ApiKeyConfig, BridgeConfig, bridges::ActiveModel, build_rpc_url(), ChainConfig, chains::ActiveModel, create_provider_pools_from_chains(), ExplorerConfig (+11 more)

### Community 44 - "InterchainStatisticsService"
Cohesion: 0.14
Nodes (17): GET /api/v1/stats/chain/{chain_id}/bridged-tokens, GET /api/v1/stats/chains, GET /api/v1/stats/common, GET /api/v1/stats/daily, GET /api/v1/stats/chain/{chain_id}/messages-paths/received, GET /api/v1/stats/chain/{chain_id}/messages-paths/sent, InterchainStatisticsService, v1GetBridgedTokensResponse (+9 more)

### Community 45 - "package.json"
Cohesion: 0.09
Nodes (22): ts-proto, bugs, url, description, devDependencies, ts-proto, typescript, homepage (+14 more)

### Community 46 - ".get_crosschain_transfers"
Cohesion: 0.12
Nodes (25): build_pagination_from_transfers(), counters_cover_all_filters(), InterchainTotalCounters, JoinedTransfer, BigDecimal, Decimal, NaiveDateTime, test_counters_filter_parity_with_filtered_lists() (+17 more)

### Community 47 - "bridge_contracts Is a Proxy, Not the Membership Set"
Cohesion: 0.10
Nodes (25): Runtime Verification Runbook, ADR-004 Stats Observability Horizon and Asset Union-Find, bridge_contracts Is a Proxy, Not the Membership Set, Canary vs Diagnostic Classification, IndexedChains::may_observe (in-memory eligibility), Observability Horizon Eligibility Rule, Query C — Hard Invariants Canary, Query D — Deferred Transfers Classified by Reason (+17 more)

### Community 48 - "spawn_configured_indexers"
Cohesion: 0.19
Nodes (23): bridge(), build_amb_chain_configs(), build_avalanche_chain_configs(), chain_config_fixture(), ChainPlan, dummy_provider(), group_contracts_by_chain(), log_amb_floor_divergence() (+15 more)

### Community 49 - "helpers/mod.rs"
Cohesion: 0.20
Nodes (9): get_raw(), init_db(), init_interchain_indexer_server(), F, String, TestDbGuard, Url, Value (+1 more)

### Community 50 - "BridgeCounts"
Cohesion: 0.19
Nodes (10): Add, BridgeCounts, Counts, record_bridge_metrics(), BridgeId, HashMap, I, Self (+2 more)

### Community 51 - "amb/indexer.rs"
Cohesion: 0.18
Nodes (23): amb_handler_failure_creates_indexer_failure_row_for_the_failing_block(), AmbChainConfig, AmbContractConfig, chain_config(), mock_block(), mock_provider(), mock_receipt(), registry_with_event() (+15 more)

### Community 52 - ".new"
Cohesion: 0.23
Nodes (11): AvalancheChainConfig, chain(), log_filter_covers_every_configured_contract_address_for_a_chain_with_several(), rejects_a_chain_with_no_contract_address(), rejects_the_same_chain_configured_twice(), Address, ChainId, Filter (+3 more)

### Community 53 - "FailureLedger"
Cohesion: 0.12
Nodes (13): FailureLedger, Arc, HashSet, Result, RwLock, Self, String, Vec (+5 more)

### Community 54 - "progress.rs"
Cohesion: 0.24
Nodes (19): CatchupProgress, CheckpointCursors, cursors(), Option, Self, test_compute_backward_compatible_with_one_directional_formula_when_m_equals_s(), test_compute_blocks_remaining_equals_interval_width(), test_compute_blocks_remaining_is_zero_once_complete() (+11 more)

### Community 55 - "TokenInfoService and Token Metadata Enrichment Flow"
Cohesion: 0.11
Nodes (20): ChainInfoService, TokenInfoService, Union-Find Asset Merge, merge_assets / ensure_asset_for_transfer — weighted union-find over stats_assets, Gotcha: Stats Asset Mapping Conflicts Merge; Only Same-Chain Collisions Skip, Gotcha: Token Identity In stats_asset_tokens Is The ICTT Contract Address, Not The Wrapped ERC-20, avalanche_icm_blockchain_ids Table, Runtime Metadata Writes (+12 more)

### Community 56 - "InterchainService"
Cohesion: 0.11
Nodes (25): Exposed REST Endpoints and Swagger URL, GET /api/v1/interchain/chains, GET /api/v1/interchain/messages/{message_id}, GET /api/v1/interchain/messages, GET /api/v1/interchain/messages:byAddress/{address}, GET /api/v1/interchain/messages:byTx/{tx_hash}, GET /api/v1/interchain/transfers, GET /api/v1/interchain/transfers:byAddress/{address} (+17 more)

### Community 57 - "MessageBuffer (Tiered Storage)"
Cohesion: 0.12
Nodes (18): upsert_cursors — GREATEST-only cursor maintenance writer (cannot lower a floor), MessageBuffer (Tiered Storage), Bridge Filtering Entrypoints, Message Buffer, Gotcha: AMB Collision Replacement Must Delete Before Insert, Gotcha: AMB Source and Destination Events Can Arrive Out of Order, Gotcha: AMB Header Sender Is Not The Source Transaction Initiator, Gotcha: AMB Transfer Sides Are Nullable and Never Mirrored (+10 more)

### Community 58 - ".fetch_token_info"
Cohesion: 0.11
Nodes (16): Erc20TokenInfoFetcher, DynProvider, Ethereum, Result, Vec, Erc20TokenHomeInfoFetcher, DynProvider, Ethereum (+8 more)

### Community 59 - "Rust Style Rules"
Cohesion: 0.12
Nodes (20): Error Handling Rules, anyhow::Result for Internal Code, API Error Sanitization, Checked/Saturating Arithmetic and Euclidean Division, Always Add Context When Propagating, Log Errors at the Handling Point, Panic Avoidance in Runtime Paths, thiserror for Public API Error Types (+12 more)

### Community 60 - "range_driver.rs"
Cohesion: 0.18
Nodes (15): a_budget_covering_the_whole_queue_attempts_each_position_once(), chunk_range(), chunk_range_clamps_a_zero_batch_size_to_one(), chunk_range_narrower_than_batch_size_yields_one_chunk(), chunk_range_splits_into_batch_size_pieces_with_a_narrower_last_chunk(), interval(), resume_index(), resume_index_re_attempts_the_chunk_a_cursor_lands_inside() (+7 more)

### Community 61 - "BatchError"
Cohesion: 0.15
Nodes (13): attributed_ranges(), BatchError, RangeDriver<P>, Error, Filter, From, Poll, Result (+5 more)

### Community 62 - "Stats Projection"
Cohesion: 0.10
Nodes (29): ADR-004: Stats Observability Horizon; Asset Identity As Union-Find, IndexedChains (Stats Eligibility), API Serving Entrypoints, Avalanche Indexing Entrypoints, Common Indexer Architecture Entrypoints, Config Loading Entrypoints, Database Schema and Migrations Entrypoints, Exploration Map (+21 more)

### Community 63 - "try_reconstruct_transfer"
Cohesion: 0.19
Nodes (19): build_reconstructed_transfer(), ClassifiedPayload, classify_payload(), destination_arm(), destination_arm_amount(), DestinationArm, dst_token_address(), Message (+11 more)

### Community 64 - "SourceData"
Cohesion: 0.18
Nodes (16): build_transfer(), AnnotatedEvent, ChainId, NaiveDateTime, ReceiveCrossChainMessage, Result, Self, SendCrossChainMessage (+8 more)

### Community 65 - "AvalancheIndexer"
Cohesion: 0.15
Nodes (11): AvalancheIndexer, IndexerCleanupGuard, Arc, AtomicBool, AtomicU64, Drop, Error, JoinHandle (+3 more)

### Community 66 - "failure_ledger/settings.rs"
Cohesion: 0.20
Nodes (16): default_backoff_base(), default_backoff_cap(), default_enabled(), default_max_chunks_per_pass(), default_record_retry_attempts(), default_record_retry_initial_backoff(), default_scan_interval(), FailureRetrySettings (+8 more)

### Community 67 - "Expected Skips Inside a Shared Transaction"
Cohesion: 0.18
Nodes (13): DecimalsConflict Domain Marker Type, Expected Skips Inside a Shared Transaction, Maintenance Transaction (messages, transfers, stats, cursor), Detect Conflicts with SELECT, Never a Failing INSERT, Never Assert a Delta on a Process-Wide Metric, STATS_EDGE_DECIMALS_CONFLICT_TOTAL (test-isolation case study), Asset-Identity Union-Find Merge, A Successful Asset Merge Is Nearly Invisible in SQL (+5 more)

### Community 68 - "BlockRange"
Cohesion: 0.24
Nodes (14): pre_union_with_reason(), BlockRange, difference_produces_expected_pieces(), fold_adjacent(), merge_bounds(), overlaps(), overlaps_or_adjacent(), pre_union() (+6 more)

### Community 69 - "policy.rs"
Cohesion: 0.22
Nodes (16): FailedInterval, NaiveDateTime, Option, String, base_ts(), capped_backoff_does_not_overflow_at_extreme_attempts(), capped_backoff_secs(), capped_backoff_widens_strictly_until_the_cap() (+8 more)

### Community 70 - "run"
Cohesion: 0.67
Nodes (3): Error, Result, run()

### Community 71 - "BufferItem"
Cohesion: 0.44
Nodes (5): BufferItem, BTreeSet, HashMap, MaintenancePlan<T>, T

### Community 72 - "workflows/ Tool-Agnostic Task Procedures"
Cohesion: 0.16
Nodes (16): Codex Solution Review Agent Interface, Solution Review Guardrails, Codex Solution Review Skill, Task Folder Artifacts (tmp/tasks/<task-name>/), Cursor GitHub Bug Issue Skill, High-Level Suggested Fix Rule, tmp/gh-issues/YYMMDD-<name>.md Draft Convention, Conceptual Proposed-Changes Rule (+8 more)

### Community 73 - "Memory Bank"
Cohesion: 0.15
Nodes (16): Explicit Human Confirmation Gate, Cursor Research Scope Skill, ADR-001: Message Buffer Tiered Storage, ADR-005: Failed-Range Ledger, Independent of Checkpoints, ADR-006: Contract Versioning Resolved By Block, bridge_contracts UNIQUE(bridge_id, chain_id, address, version), Contract Version Windows by started_at_block, adr/ Architectural Decision Records (+8 more)

### Community 74 - "interchain-indexer service"
Cohesion: 0.19
Nodes (16): docker-compose.yml Full Stack, backend service (Blockscout API), caddy service, db service (postgres:17), db-init service, frontend service, interchain-indexer service, redis-db service (+8 more)

### Community 75 - "Layer 1: Generic Pipeline"
Cohesion: 0.17
Nodes (15): Consolidate Trait, Maintenance Task, Operational Risks, Pre-Buffer Storage Gate, Canonical Indexing Write Path, indexer_checkpoints Semantics, Cursor Gap Bridging, Database Outage Followed By Recovery Leap (+7 more)

### Community 76 - "AvalancheDataApiClient"
Cohesion: 0.18
Nodes (15): ClientWithMiddleware, AvalancheDataApiClient, AvalancheDataApiClientSettings, AvalancheDataApiNetwork, GetBlockchainByIdResponse, Option, Result, Self (+7 more)

### Community 77 - "AmbDispatchMockService"
Cohesion: 0.17
Nodes (11): Id, AmbDispatchMockService, json_response(), Error, Future, Poll, RequestPacket, ResponsePacket (+3 more)

### Community 78 - "prelude.rs"
Cohesion: 0.12
Nodes (10): Date, Model, Relation, DateTime, Model, Relation, DateTime, Model (+2 more)

### Community 79 - "Checkpoint"
Cohesion: 0.20
Nodes (14): A failed floor write stays a warn (today), Checkpoint, Gotcha: AMB Queued Events Must Preserve Their Emitting Chain, Gotcha: A Checkpoint Certifies Scanning, Not Correctness, Gotcha: Checkpoint Stall When All Events Are Perpetually Filtered, FailureLedger — in-memory open-hole cache over indexer_failures, Gotcha: The Failure Ledger's Healthy Path Is DB-Free Only Because One Process Owns A Bridge, indexer_failures — the failed-range ledger (+6 more)

### Community 80 - "Database Subsystem: Schema and DB Interaction Layer"
Cohesion: 0.15
Nodes (14): Database Schema, Unindexed-Chain Read Filter, batched_upsert / run_in_batches, Database Subsystem: Schema and DB Interaction Layer, Hybrid Database Layer, InterchainDatabase Facade, Table Families, Unindexed-Chain Read Filter (+6 more)

### Community 81 - "Project Context"
Cohesion: 0.12
Nodes (18): AvalancheIndexer, High-Level Data Flow, LogStream, Whole-System Entrypoints, Teleporter / ICM, Configuration Model, Crate Map, Local Development Flow (+10 more)

### Community 82 - "Architectural Decision Records Index"
Cohesion: 0.17
Nodes (13): ADR-001: Message Buffer Tiered Storage, ADR-002: Primary Chain Filtering for Unknown Chains, ADR-003: AMB Transfers Reconstructed From Events; Nullable Transfer Sides, ADR-006: Contract Versioning Resolved By Block, At Decode Time, Architectural Decision Records Index, ADR Template, AMB / Omnibridge Token Transfer Reconstruction, build_transfer / build_destination_only_transfer (+5 more)

### Community 83 - "BridgeType"
Cohesion: 0.16
Nodes (9): Entity, Model, Relation, DateTime, Option, Related, RelationDef, String (+1 more)

### Community 84 - "fetch_receipts_for_transactions"
Cohesion: 0.18
Nodes (12): fetch_receipts_for_transactions(), FetchedTransactionReceipt, Address, B256, Block, DynProvider, Ethereum, HashMap (+4 more)

### Community 85 - "Codex Task Analysis Skill"
Cohesion: 0.18
Nodes (12): Codex Task Analysis Agent Interface, Human Evaluation-Criteria Alignment, solution_N.md Option Files, Codex Task Analysis Skill, Codex Task To Code Agent Interface, coding-task-X.md Handoff, No-Invented-Scope Rule, Codex Task To Code Skill (+4 more)

### Community 86 - "compile"
Cohesion: 0.32
Nodes (11): AsRef, compile(), dedupe_actix_duplicate_chain_info_internal(), main(), Box, Error, Path, Result (+3 more)

### Community 87 - "bridge_model_to_proto"
Cohesion: 0.32
Nodes (11): Bridge, BridgeModel, bridge_model_to_proto(), model(), Result, test_bridge_model_to_proto_multi_chain_is_sorted(), test_bridge_model_to_proto_no_configured_chains_is_empty(), test_bridge_model_to_proto_ordering_is_deterministic_across_insertion_orders() (+3 more)

### Community 88 - "MockLogsService"
Cohesion: 0.21
Nodes (10): build_logs_response(), MockLogsAction, MockLogsService, Future, Log, Mutex, RequestPacket, ResponsePacket (+2 more)

### Community 89 - "BridgeContractConfig"
Cohesion: 0.19
Nodes (12): BridgeContractConfig, ActiveModel, test_bridge_contract_config_to_active_model(), amb_contract_configs(), contract(), contract_with_address(), parse_contract_abi(), parse_contract_address() (+4 more)

### Community 90 - "stats.rs"
Cohesion: 0.14
Nodes (11): bridged_row_to_proto(), i64_to_u64_nonneg(), map_stats_error(), parse_optional_utc_date(), parse_optional_utc_date_rejects_malformed(), Error, NaiveDate, Option (+3 more)

### Community 91 - ".check"
Cohesion: 0.18
Nodes (7): Health, HealthCheckRequest, HealthCheckResponse, HealthService, Request, Response, Result

### Community 92 - "Model"
Cohesion: 0.20
Nodes (8): Entity, Model, Relation, DateTime, Option, Related, RelationDef, String

### Community 93 - "BridgedTokensSortField"
Cohesion: 0.20
Nodes (13): BridgedTokenAggDbRow, build_pagination_from_bridged_tokens(), count_column(), cursor_where_next(), cursor_where_prev(), String, Value, BridgedTokensSortField (+5 more)

### Community 94 - "Interchain Indexer"
Cohesion: 0.15
Nodes (12): Architecture, Build & Test, Configuration, Conventions, graphify, Interchain Indexer, Key Decisions, Known Gotchas (+4 more)

### Community 95 - "from_sql"
Cohesion: 0.18
Nodes (9): from_sql(), Migrator, Box, DbErr, MigrationTrait, Result, SchemaManager, Vec (+1 more)

### Community 96 - "ADR-004: Observability Horizon and Asset Union-Find"
Cohesion: 0.24
Nodes (10): ADR-002: Per-Bridge Chain Filtering, Fail-Fast Startup Validation of home_chain_id, Filter Order: Chain-Config Then Home-Chain, home_chain_id, process_unknown_chains, ADR-004: Observability Horizon and Asset Union-Find, Config Change Never Reinterprets Indexed History, include_unindexed_chains Read Filter (+2 more)

### Community 97 - "Event-Derived AMB Transfer Reconstruction"
Cohesion: 0.20
Nodes (10): ADR-003: AMB Transfers From Events; Nullable Sides, build_transfer (amb/consolidation.rs), Calldata Token Directional Ambiguity, Event-Derived AMB Transfer Reconstruction, Nullable Transfer Sides (Never Mirrored), Removal of the payload_processor Calldata Subsystem, TokensBridged (Destination Side), TokensBridgingInitiated (Source Side) (+2 more)

### Community 98 - "FailureLedger"
Cohesion: 0.22
Nodes (10): FailureLedger, indexer_failures Table, LogBatch (from_block, to_block, direction, logs), LogStream, One Scanner Per (bridge, chain), RangeDriver, RangeProcessor Trait, record Merges on Overlap or Adjacency (+2 more)

### Community 99 - "BlockchainIdResolver"
Cohesion: 0.29
Nodes (7): Cache, CacheKey, CacheValue, BlockchainIdResolver, resolves_native_id_to_chain_id_8021_and_persists_mapping(), Result, Self

### Community 100 - "AmbIndexerSettings"
Cohesion: 0.36
Nodes (8): AmbIndexerSettings, default_batch_size(), default_clock_skew_tolerance(), default_pull_interval(), default_receipt_concurrency(), Default, Duration, Self

### Community 101 - "src/utils.rs"
Cohesion: 0.56
Nodes (8): bytes_to_naive_datetime(), naive_datetime_to_bytes(), naive_datetime_to_nanos(), nanos_to_naive_datetime(), NaiveDateTime, Result, test_naive_datetime_to_bytes_round_trip(), u64_from_hex_prefixed()

### Community 102 - "Indexing Gaps, Retries, and Checkpoint Safety"
Cohesion: 0.25
Nodes (9): ADR-005: Failed-Range Ledger, Independent of Checkpoints, RangeDriver::run / run_retry_tick (indexer/range_driver.rs), Gotcha: The Retry Pass Starves The Forward Streams, And That Looks Like RPC Failure, AMB In-Memory Correlation Maps, FailureLedger / indexer_failures, Indexing Gaps, Retries, and Checkpoint Safety, LogBatch (Named Range), RangeDriver Retry Pass (+1 more)

### Community 103 - "Model"
Cohesion: 0.22
Nodes (8): Model, Relation, BigDecimal, DateTime, Decimal, Option, Vec, TransferType

### Community 104 - "Layer 2: Avalanche Reference Realization"
Cohesion: 0.22
Nodes (10): CrosschainIndexer Trait, IndexerCleanupGuard Drop Guard, Layer 2: Avalanche Reference Realization, Transaction-Grouped Processing, Async Patterns Rules, Graceful Shutdown and Cleanup Guards, Shared State (Arc RwLock), Start/Stop Invariants (+2 more)

### Community 105 - "AvalancheRangeProcessor"
Cohesion: 0.21
Nodes (9): AvalancheRangeProcessor, BatchProcessContext, process_batch(), DynProvider, Ethereum, HashSet, Message, MessageBuffer (+1 more)

### Community 106 - "Testing Rules"
Cohesion: 0.29
Nodes (8): Testing Rules, Feature-Flagged E2E Tests (avalanche-e2e), just test-with-db vs just test, fill_mock_interchain_database Fixtures, Test Attributes (tokio::test, ignore, rstest), Test Naming Format, TestDbGuard Isolated Database Tests, Prefer Repo-Native Verification Commands over cargo test

### Community 107 - "Model"
Cohesion: 0.25
Nodes (7): Model, Relation, DateTime, Json, Option, String, Vec

### Community 108 - ".record_indexer_failures"
Cohesion: 0.30
Nodes (10): indexer_failure_totals_sums_blocks_and_reports_the_oldest_created_at(), indexer_failures_rows_for(), open_indexer_failures_is_a_pure_read_with_no_side_effects(), record_indexer_failures_disjointness_holds_after_mixed_merges(), record_indexer_failures_does_not_merge_across_a_real_gap(), record_indexer_failures_growth_bound_for_consecutive_realtime_failures(), record_indexer_failures_merges_overlapping_and_adjacent_ranges_into_one_row(), resolve_indexer_failures_resets_attempts_but_keeps_parents_updated_at_on_split() (+2 more)

### Community 109 - "AvalancheIndexerSettings"
Cohesion: 0.39
Nodes (6): AvalancheIndexerSettings, default_batch_size(), default_pull_interval(), Default, Duration, Self

### Community 110 - "BlockscoutTokenInfoClientSettings"
Cohesion: 0.26
Nodes (11): BlockscoutTokenInfoClientSettings, default_icon_retry_interval(), default_ignore_chains(), default_onchain_retry_interval(), Default, Duration, Option, Self (+3 more)

### Community 111 - "Migration"
Cohesion: 0.36
Nodes (5): Migration, DbErr, MigrationTrait, Result, SchemaManager

### Community 112 - "Migration"
Cohesion: 0.36
Nodes (5): Migration, DbErr, MigrationTrait, Result, SchemaManager

### Community 113 - "Migration"
Cohesion: 0.36
Nodes (5): Migration, DbErr, MigrationTrait, Result, SchemaManager

### Community 114 - "Migration"
Cohesion: 0.36
Nodes (5): Migration, DbErr, MigrationTrait, Result, SchemaManager

### Community 115 - "Tiered Message Buffer Storage"
Cohesion: 0.33
Nodes (7): Cold Tier (pending_messages table), Entry Versioning for Cursor Tracking, Hot Tier (In-Memory DashMap), Tiered Message Buffer Storage, TTL-Based Eviction and Cache-Miss Restoration, catchup_min_cursor, Checkpoint/Ledger Independence

### Community 116 - "MessageStatus"
Cohesion: 0.33
Nodes (6): Model, Relation, DateTime, Option, Vec, MessageStatus

### Community 117 - "AbiRegistry"
Cohesion: 0.29
Nodes (7): interchain_indexer_oldest_open_hole_age_seconds, Cyclic Retry-Pass Sweep with Shared Chunk Budget, AbiRegistry, AmbChainConfig (amb_proxies, mediators lists), resolve_log(chain_id, address, topic, block_number), topic0 Cannot Substitute for Block Resolution, interchain_indexer_amb_logs_dropped_wrong_version_total

### Community 118 - "BufferItem<T>"
Cohesion: 0.21
Nodes (7): BufferItem<T>, BlockNumber, BufferItemVersion, ChainId, Result, Self, T

### Community 119 - "Model"
Cohesion: 0.29
Nodes (6): Model, Relation, DateTime, Option, String, Vec

### Community 120 - "Model"
Cohesion: 0.29
Nodes (6): Model, Relation, DateTime, Option, String, Vec

### Community 121 - "MaintenancePlan"
Cohesion: 0.36
Nodes (6): HotEvictionReason, MaintenancePlan, MessageBuffer<T>, BufferItemVersion, Result, Vec

### Community 122 - "unindexed_chain_filter.rs"
Cohesion: 0.22
Nodes (4): message_details_bridge_qualifier_contract(), TestDbGuard, seed_bridge_collision(), Utc

### Community 123 - "maintenance.rs"
Cohesion: 0.31
Nodes (6): classify_item(), ConsolidationOutcome, Option, test_buffer_settings(), test_cold_tier_restore_projects_exactly_once(), TransferDummyMessage

### Community 124 - "group_logs_by_transaction"
Cohesion: 0.38
Nodes (6): group_logs_by_transaction(), B256, HashMap, Log, Vec, test_group_logs_by_transaction_preserves_input_order()

### Community 125 - "v1ChainIndexingProgress"
Cohesion: 0.19
Nodes (13): GET /api/v1/status/indexers, GET /api/v1/status/indexing, GET /api/v1/status/indexers/{indexer_name}, StatusService, Inclusive [catchup_min_cursor, catchup_max_cursor] Unscanned Interval, v1ChainIndexingProgress, v1FullStatus, v1GetIndexingProgressResponse (+5 more)

### Community 126 - "CrosschainIndexer Worker"
Cohesion: 0.40
Nodes (6): Project-Specific Naming Conventions, BridgeContractIndexer Worker, Common Design Principles, CrosschainIndexer Worker, MessageCollector Worker, TokenFetcher Worker

### Community 127 - "Model"
Cohesion: 0.33
Nodes (5): Model, Relation, DateTime, Option, Vec

### Community 128 - "Model"
Cohesion: 0.33
Nodes (5): Model, Relation, DateTime, Option, Vec

### Community 129 - "Model"
Cohesion: 0.33
Nodes (5): Model, Relation, DateTime, Option, Vec

### Community 130 - "Model"
Cohesion: 0.33
Nodes (5): Model, Relation, DateTime, Option, String

### Community 131 - "Model"
Cohesion: 0.33
Nodes (5): Model, Relation, DateTime, Json, Option

### Community 132 - "Asset Identity as Union-Find"
Cohesion: 0.29
Nodes (7): Maintenance Task and Consolidation Pass, Asset Identity as Union-Find, Eager Weighted Asset Merge, Fragmented Assets Defect, Conflicts Are Refusals, Never Transaction Errors, stats_processed Counting Marker, Drain Must Not Clear Its Queue Before Writes Succeed

### Community 133 - "ApiError"
Cohesion: 0.40
Nodes (4): ApiError, Error, Self, String

### Community 135 - "buffer_item.rs"
Cohesion: 0.33
Nodes (4): now_naive_utc(), NaiveDateTime, Vec, token_keys_from_flushed_for_enrichment()

### Community 136 - "stats_asset_tokens.rs"
Cohesion: 0.40
Nodes (4): Model, Relation, DateTime, Vec

### Community 137 - "Gotcha: PostgreSQL Bind Parameter Limit (65535 per statement)"
Cohesion: 0.50
Nodes (4): batched_upsert() / run_in_batches() (bulk.rs), Gotcha: PostgreSQL Bind Parameter Limit (65535 per statement), Gotcha: SeaORM Entity Regeneration Overwrites Manual Changes (codegen/ vs manual/), Gotcha: SeaORM insert_many Cannot Mix Set and NotSet for the Same Column

### Community 138 - "Gotcha: Token Info Is Eventually Consistent and Reads Can Write Back"
Cohesion: 0.50
Nodes (4): Gotcha: Stats Edge Amount Side Must Follow Indexed Source Presence, Gotcha: Token Info Caches Errors (TokenInfoService negative TTL), Gotcha: Token Info Is Eventually Consistent and Reads Can Write Back, TokenInfoService (token_info/service.rs)

### Community 139 - "chain_model_to_proto"
Cohesion: 0.67
Nodes (3): ChainModel, chain_model_to_proto(), ChainInfo

### Community 140 - "is_tmp_mkdir_command"
Cohesion: 0.67
Nodes (3): is_tmp_mkdir_command(), main(), Check if the Bash command is creating directories within the tmp/ directory.…

### Community 141 - "is_tmp_path"
Cohesion: 0.67
Nodes (3): is_tmp_path(), main(), Check if the file path is within the tmp/ directory. Handles various path…

### Community 142 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 143 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 144 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 145 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 146 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 147 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 148 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 149 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 150 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 151 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 152 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 153 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 154 - ".recompute_stats_chains"
Cohesion: 0.50
Nodes (4): recompute_stats_chains_distinct_users_and_merges_message_transfer_sides(), select_stats_chains_message_user_counts(), select_stats_chains_transfer_user_counts(), SelectStatement

### Community 155 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 156 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 157 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

## Knowledge Gaps
- **187 isolated node(s):** `gh-issue-publish.sh script`, `Relation`, `ActiveModel`, `Relation`, `ActiveModel` (+182 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **27 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `InterchainDatabase` connect `InterchainDatabase` to `StatsService`, `fill_mock_interchain_database`, `persistence.rs`, `init_db`, `.new`, `ExampleIndexer`, `log_stream.rs`, `ChainInfoService`, `.new`, `InterchainServiceImpl`, `indexers.rs`, `.recompute_stats_chains`, `Settings`, `.new`, `AmbIndexer`, `TokenInfoService`, `.get_crosschain_transfers`, `FailureLedger`, `AvalancheIndexer`, `BlockchainIdResolver`, `.record_indexer_failures`?**
  _High betweenness centrality (0.221) - this node is a cross-community bridge._
- **Why does `Key` connect `Key` to `SourceData`, `AmbIndexer`, `persistence.rs`, `BufferItem`, `AvalancheRangeProcessor`, `maintenance.rs`, `FailureLedger`, `MaintenancePlan`, `avalanche/consolidation.rs`, `Settings`, `avalanche/mod.rs`, `try_reconstruct_transfer`?**
  _High betweenness centrality (0.066) - this node is a cross-community bridge._
- **Why does `IndexedChains` connect `indexed_chains.rs` to `.new`, `Status`, `StatsService`, `fill_mock_interchain_database`, `InterchainDatabase`, `init_db`, `services/utils.rs`, `projection.rs`, `bridged_tokens_query.rs`, `.new`, `InterchainServiceImpl`, `bridge_model_to_proto`?**
  _High betweenness centrality (0.059) - this node is a cross-community bridge._
- **Are the 229 inferred relationships involving `init_db()` (e.g. with `bridged_tokens_aggregation_input_output_total()` and `bridged_tokens_default_excludes_edge_unindexed_for_its_bridge()`) actually correct?**
  _`init_db()` has 229 INFERRED edges - model-reasoned connections that need verification._
- **Are the 79 inferred relationships involving `fill_mock_interchain_database()` (e.g. with `counters_cover_all_filters()` and `get_crosschain_message_native_collision_is_ambiguous_until_qualified()`) actually correct?**
  _`fill_mock_interchain_database()` has 79 INFERRED edges - model-reasoned connections that need verification._
- **What connects `gh-issue-publish.sh script`, `Relation`, `ActiveModel` to the rest of the system?**
  _187 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Key` be split into smaller, more focused modules?**
  _Cohesion score 0.05428796223446106 - nodes in this community are weakly interconnected._