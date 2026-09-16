# Graph Report - interchain-indexer  (2026-09-16)

## Corpus Check
- 294 files · ~326,086 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 4302 nodes · 10256 edges · 260 communities (228 shown, 32 thin omitted)
- Extraction: 93% EXTRACTED · 7% INFERRED · 0% AMBIGUOUS · INFERRED: 674 edges (avg confidence: 0.82)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `d9b4089f`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- interchain-indexer-filters/src/lib.rs
- provider_layers.rs
- StatsService
- .new
- persistence.rs
- indexed_chains.rs
- InterchainDatabase
- init_db
- AbiRegistry
- StatusServiceImpl
- .new
- ictt_payload.rs
- project_transfers_batch
- env_merge.rs
- config.rs
- log_stream.rs
- ChainInfoService
- BlockscoutTokenInfoClient
- bridged_tokens_query.rs
- stats_chains_query.rs
- .upsert_chains
- ADR-007: The Scan Floor Is Reconciled Against The Checkpoint, Not Against bridge_contracts (Accepted, with stated expiry)
- .new
- .message_model_to_proto
- range_driver.rs
- indexers.rs
- Avalanche Bridge Filtering
- avalanche/consolidation.rs
- MessageBuffer
- cursor.rs
- avalanche/mod.rs
- Interchain Indexer Service README
- InterchainServiceImpl
- Status
- AmbIndexer
- Codex Skill: implementation-plan
- TokenTransfer
- BridgeConfig
- services/utils.rs
- Workflow: implementation-plan.md
- Settings
- Codex Skill: gh-issue-publish
- Glossary
- build_chain_node_configs
- GET /api/v1/status/indexers
- package.json
- EventContext
- bridge_contracts Is a Proxy, Not the Membership Set
- AvalancheIndexer
- Option
- BufferItem
- stats_chains_bridge_filter.rs
- protocol_metadata.rs
- abi_registry.rs
- progress.rs
- TokenInfoService
- InterchainService
- Gotcha: Recoverable Message Fields Are Not A "Never Mirror" Case
- Consolidate
- projection.rs
- amb/consolidation.rs
- blockchain_id_resolver.rs
- Stats Projection
- stats.rs
- Indexing Gaps, Retries, and Checkpoint Safety
- BatchError
- TestRangeProcessor
- Expected Skips Inside a Shared Transaction
- Result
- retry_scheduler.rs
- Async Patterns Rules
- spawn_configured_indexers
- workflows/ Tool-Agnostic Task Procedures
- Memory Bank
- interchain-indexer service
- Layer 1: Generic Pipeline
- AvalancheDataApiNetwork
- ADR-011: Cross-Asset Stats Edges And Per-Transfer Asset Linkage
- xdai/abi.rs
- Checkpoint
- Database Subsystem: Schema and DB Interaction Layer
- send_event
- Architectural Decision Records Index
- BridgeType
- fetch_receipts_for_transactions
- Codex Task Analysis Skill
- compile
- bridge_model_to_proto
- .fetch_token_info
- Codex Skill: research-scope
- indexing_coupling.py
- .check
- Model
- CrosschainIndexerState
- Interchain Indexer
- from_sql
- ADR-004: Observability Horizon and Asset Union-Find
- Event-Derived AMB Transfer Reconstruction
- FailureLedger
- MockLogsService
- failure_ledger/settings.rs
- .record_indexer_failures
- TokenInfoService and Token Metadata Enrichment Flow
- Indexing Concurrency Model and Throughput
- K
- .new
- IndexedChains
- .new
- logging.rs
- Bridges
- Migration
- Migration
- Migration
- Migration
- Tiered Message Buffer Storage
- Testnet set
- AbiRegistry
- Utc
- ExampleIndexer
- TokenType
- amb/events.rs
- ENVs — `config/full-mainnet`
- ADR-009: Stats Chains Global-Plus-Per-Bridge Snapshots
- group_logs_by_transaction
- AMB Destination-Only Consolidation and the Permanent Missing-Source Gap
- XDaiIndexer
- Model
- Model
- Model
- Model
- Key
- ADR-008: Per-Chain Concurrency Within A Bridge, Cooperative And Single-Task
- ApiError
- ENVs — `config` (empty base set)
- CrosschainIndexerStatus
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
- native_id_blob
- Entity
- Entity
- Entity
- indexer_checkpoints::Model
- Asset Identity as Union-Find
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
- Unresolved Avalanche Blockchain IDs
- xDai Bridge: Protocol Model, Contrast With Omnibridge, and Indexing Fit
- CleanupGuard
- InterchainStatisticsService
- .get_checkpoint
- Address
- secret.rs
- BridgedTokensSortField
- ENVs — `config/avalanche`
- ENVs — `config/full-testnet`
- BlockscoutTokenInfoClientSettings
- Mainnet set
- .create_stats_asset
- BufferItem<T>
- AmbHeader
- amb/version.rs
- server.rs
- Query F — Incoming ICTT Reconstruction Diagnostic
- Migration
- resolve-blockchain-id.rs
- Rust Style Rules
- Chains
- prelude.rs
- AvalancheIndexerSettings
- Entity
- Entity
- ActiveModel
- MessageBuffer (Tiered Storage)
- Testing Rules
- ADR-010: Unresolved Avalanche Destinations And Reusable Protocol Metadata
- Exploration Map
- ChainConfig
- ExampleIndexerSettings
- avalanche_data_api.rs
- overlap_warning.rs
- interchain_service.rs
- chains_endpoint_filters.rs
- CrosschainIndexer Worker
- config/ENVs.md
- run
- seed_pre_xdai_schema
- helpers/mod.rs
- Workflow: pr-description.md
- .from_address
- recompute_stats_chains_distinct_users_and_merges_message_transfer_sides
- Migration
- Migration
- serialize
- LookupError
- Claude Code Overrides (project CLAUDE.md)
- indexer_checkpoints.rs
- Self
- ActiveValue

## God Nodes (most connected - your core abstractions)
1. `init_db()` - 273 edges
2. `InterchainDatabase` - 116 edges
3. `fill_mock_interchain_database()` - 94 edges
4. `seed_minimal_bridge()` - 56 edges
5. `Key` - 54 edges
6. `list_stats_chains()` - 45 edges
7. `IndexedChains` - 43 edges
8. `Status` - 39 edges
9. `list_bridged_token_stats_for_chain()` - 37 edges
10. `TokenInfoService` - 37 edges

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

## Communities (260 total, 32 thin omitted)

### Community 0 - "interchain-indexer-filters/src/lib.rs"
Cohesion: 0.10
Nodes (34): ChainBridgeFilter, messages_where(), Condition, Option, String, Vec, sql_messages(), sql_transfers() (+26 more)

### Community 1 - "provider_layers.rs"
Cohesion: 0.06
Nodes (66): Context, DefaultClock, HeaderMap, InMemoryState, base_node(), block_number_packet(), build_layered_http_provider(), build_layered_provider_from_services() (+58 more)

### Community 2 - "StatsService"
Cohesion: 0.14
Nodes (15): MessagePathStatsRow, BridgedTokenListRow, kickoff_enrichment_no_token_service_is_noop(), Arc, DatabaseTransaction, DbErr, Default, NaiveDate (+7 more)

### Community 3 - ".new"
Cohesion: 0.11
Nodes (55): counters_cover_all_filters(), default_filter(), InterchainTotalCounters, message_ids(), mock_db_upsert_chain(), mock_indexed_chains(), NaiveDateTime, Self (+47 more)

### Community 4 - "persistence.rs"
Cohesion: 0.07
Nodes (80): A, batch_size_for_width(), batched_upsert(), ConnectionTrait, DbErr, F, OnConflict, Result (+72 more)

### Community 5 - "indexed_chains.rs"
Cohesion: 0.07
Nodes (50): Column, Entity, chain_unindexed_condition(), cols(), message_countable_condition(), render(), Clone, Condition (+42 more)

### Community 6 - "InterchainDatabase"
Cohesion: 0.07
Nodes (42): BTreeMap, BackfillStatsReport, BridgeChainUserCountRow, build_all_time_message_paths_query(), build_bounded_message_paths_query(), CrosschainMessageLookup, expect_found(), get_crosschain_message_native_collision_is_ambiguous_until_qualified() (+34 more)

### Community 7 - "init_db"
Cohesion: 0.10
Nodes (71): completed_message(), completed_message_at(), completed_message_without_indexed_source(), insert_already_processed_bridging_transfer(), load_native_id_map_filters_missing_native_ids(), message_paths_invalid_or_empty_range_returns_empty(), recompute_stats_chains_multi_bridge_overlap_and_removal(), ActiveModel (+63 more)

### Community 8 - "AbiRegistry"
Cohesion: 0.17
Nodes (17): AbiRegistry, amb_side_for_abi(), assert_canonical_topics(), ContractKind, from_chains_registers_several_versions_of_one_address_and_allows_no_mediator(), from_chains_rejects_an_xdai_abi_offered_as_amb(), Address, B256 (+9 more)

### Community 9 - "StatusServiceImpl"
Cohesion: 0.16
Nodes (16): FullStatus, GetFullStatusRequest, GetIndexingProgressRequest, GetIndexingProgressResponse, GetStatusRequest, IndexerStatus, CrosschainIndexer, Send (+8 more)

### Community 10 - ".new"
Cohesion: 0.27
Nodes (9): Arc, DynProvider, Error, Ethereum, Filter, HashMap, Result, Self (+1 more)

### Community 11 - "ictt_payload.rs"
Cohesion: 0.11
Nodes (21): ictt_completeness(), CreditExpectation, decode_inner(), decode_transferrer_message(), IcttPayload, mainnet_bytes(), PayloadRejection, Result (+13 more)

### Community 12 - "project_transfers_batch"
Cohesion: 0.23
Nodes (33): EdgeKey, asset_has_token_on_chain(), AssetResolution, enrich_stats_assets_for_batch(), enrich_stats_assets_side(), ensure_asset_for_transfer(), ensure_conversion_asset_for_transfer(), ensure_mirror_asset_for_transfer() (+25 more)

### Community 13 - "env_merge.rs"
Cohesion: 0.13
Nodes (51): AppliedOverride, apply(), apply_env_overrides(), apply_patch(), apply_to_keyed_array(), apply_to_named_map_array(), ArrayRule, ArrayRules (+43 more)

### Community 14 - "config.rs"
Cohesion: 0.06
Nodes (29): api_key(), build_rpc_url(), derived_api_key_env_var(), IndexerType, ranked_names(), resolve_api_key(), test_build_rpc_url_header_location_returns_url_unchanged(), test_build_rpc_url_no_api_key_returns_url_unchanged() (+21 more)

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
Cohesion: 0.14
Nodes (47): add_asset_edges_on_bridge(), add_cross_asset_edge(), bridged_tokens_aggregation_input_output_total(), bridged_tokens_cross_asset_edges_split_by_focal_chain(), bridged_tokens_default_excludes_edge_unindexed_for_its_bridge(), bridged_tokens_empty_configured_pairs_restricts_nothing(), bridged_tokens_last_page(), bridged_tokens_name_sort_nulls_and_empty_last() (+39 more)

### Community 19 - "stats_chains_query.rs"
Cohesion: 0.12
Nodes (48): build_bridge_scope_join(), cursor_where_next(), cursor_where_prev(), default_query(), forward_order_clause(), inverse_order_clause(), list_stats_chains(), ConnectionTrait (+40 more)

### Community 20 - ".upsert_chains"
Cohesion: 0.17
Nodes (32): message_paths_bounded_queries_apply_open_and_half_open_ranges(), message_paths_bounded_queries_sum_daily_rows_and_order_deterministically(), message_paths_default_excludes_pair_unindexed_for_its_bridge(), message_paths_empty_configured_pairs_restricts_nothing(), message_paths_include_zero_bounded_counterparty_expands_requested_known_rows_only(), message_paths_include_zero_bounded_queries_expand_known_chains(), message_paths_include_zero_counterparty_expands_requested_known_rows_only(), message_paths_include_zero_incoming_all_time_expands_known_chains() (+24 more)

### Community 21 - "ADR-007: The Scan Floor Is Reconciled Against The Checkpoint, Not Against bridge_contracts (Accepted, with stated expiry)"
Cohesion: 0.08
Nodes (38): ADR-007: The Scan Floor Is Reconciled Against The Checkpoint, Not Against bridge_contracts (Accepted, with stated expiry), Alternative 1 (rejected): Withhold the contracts upsert when the previous floor is unknown, Alternative 2 (rejected for now, the correct long-term shape): persist the pair's floor in its own column, Alternative 3 (rejected): also lower catchup_max_cursor to the old floor, Neutral consequence: bridge_contracts returns to being purely diagnostic, bridges_pending_contracts_upsert — REMOVED by ADR-007 (evidence-preserving withhold mechanism and its startup coupling), catchup_max_cursor is deliberately untouched — lowering a floor causes no rescan in the current design, ChainPlan::floor_contracts — survives with one consumer, start_block() (+30 more)

### Community 22 - ".new"
Cohesion: 0.19
Nodes (12): AvalancheChainConfig, AvalancheRangeProcessor, chain(), log_filter_covers_every_configured_contract_address_for_a_chain_with_several(), Address, ChainId, DynProvider, Ethereum (+4 more)

### Community 23 - ".message_model_to_proto"
Cohesion: 0.11
Nodes (24): AddressInfo, BridgeInfo, CrosschainMessageModel, CrosschainTransferModel, DbMessageStatus, JoinedTransfer, BigDecimal, Decimal (+16 more)

### Community 24 - "range_driver.rs"
Cohesion: 0.17
Nodes (15): a_blocked_retry_pass_does_not_stop_the_forward_streams(), a_chain_blocked_inside_process_does_not_stop_its_siblings(), a_slow_chain_does_not_slow_down_its_siblings(), an_unrecordable_failure_on_one_chain_still_fails_the_whole_bridge(), disabled_retry_keeps_existing_ledger_rows_unchanged(), empty_batch(), escalates_and_stops_consuming_when_record_keeps_failing(), healthy_path_issues_no_ledger_write_statement() (+7 more)

### Community 25 - "indexers.rs"
Cohesion: 0.18
Nodes (25): checkpoint_floor(), enumerate_indexing_targets(), init_db(), omnibridge_config_path(), progress_for(), reconcile_catchup_floors(), reconcile_catchup_floors_amb_pair_only_resets_on_amb_proxy_lowered(), reconcile_catchup_floors_disabled_bridge_lowered_then_reenabled_ends_up_lowered() (+17 more)

### Community 26 - "Avalanche Bridge Filtering"
Cohesion: 0.09
Nodes (32): Codebase Review, Complexity Hotspots, Onboarding Friction, Recommended Research Priorities, Message Lifecycle Entrypoints, Home Chain, Avalanche Data API, External Systems (+24 more)

### Community 27 - "avalanche/consolidation.rs"
Cohesion: 0.29
Nodes (26): addr(), execution_succeeded(), key(), message_id(), receive_event(), B256, test_consolidate_configured_source_without_send_returns_none_even_with_decodable_payload(), test_consolidate_execution_only_failed_populates_message_fields() (+18 more)

### Community 28 - "MessageBuffer"
Cohesion: 0.15
Nodes (25): FnOnce, DummyMessage, MessageBuffer, MessageBuffer<T>, new_buffer(), Arc, BlockNumber, ChainId (+17 more)

### Community 29 - "cursor.rs"
Cohesion: 0.16
Nodes (24): FnMut, block_sets_bootstrap_delegates(), block_sets_extend_delegates(), BlockSets, bootstrap(), BootstrapCase, Cursor, cursor_blocks_builder_keys_iterator() (+16 more)

### Community 30 - "avalanche/mod.rs"
Cohesion: 0.16
Nodes (30): gate_receiver_ictt_arm(), handle_log(), handle_message_executed(), handle_message_execution_failed(), handle_receive_cross_chain_message(), handle_send_cross_chain_message(), LogHandleContext, parse_execution_outcome_log() (+22 more)

### Community 31 - "Interchain Indexer Service README"
Cohesion: 0.14
Nodes (21): database service (UBI stack), interchain-indexer service (UBI stack, built from Dockerfile), ./config Mounted into /app/config, Inclusive [catchup_min_cursor, catchup_max_cursor] Unscanned Interval, failed_blocks Is a Single Number by Design, v1ChainIndexingProgress, Interchain Indexer Service README, AMB Indexer Env Settings (+13 more)

### Community 32 - "InterchainServiceImpl"
Cohesion: 0.14
Nodes (23): GetBridgesRequest, GetBridgesResponse, GetChainsRequest, GetChainsResponse, GetMessageDetailsRequest, GetMessagesByAddressRequest, GetMessagesByTransactionRequest, GetMessagesRequest (+15 more)

### Community 33 - "Status"
Cohesion: 0.16
Nodes (21): GetBridgedTokensRequest, GetBridgedTokensResponse, GetChainsStatsRequest, GetChainsStatsResponse, GetCommonStatisticsRequest, GetCommonStatisticsResponse, GetDailyStatisticsRequest, GetDailyStatisticsResponse (+13 more)

### Community 34 - "AmbIndexer"
Cohesion: 0.06
Nodes (58): Id, amb_handler_failure_creates_indexer_failure_row_for_the_failing_block(), AmbChainConfig, AmbContractConfig, AmbDispatchMockService, AmbIndexer, chain_config(), json_response() (+50 more)

### Community 35 - "Codex Skill: implementation-plan"
Cohesion: 0.21
Nodes (17): Handoff Preparation, Not Re-Analysis, Claude Skill: implementation-plan, Reviewer-Facing PR Description (not a changelog), Claude Skill: pr-description, Claude Skill: solution-review, Verification Gap Reporting, Claude Skill: task-analysis, tmp/tasks/<task-name>/ Artifact Convention (+9 more)

### Community 36 - "TokenTransfer"
Cohesion: 0.09
Nodes (30): CallFailed, CallSucceeded, call_outcome_transfer(), tokens_sent_and_withdrawn_transfer(), tokens_sent_transfer(), AnnotatedEvent, AnnotatedICTTSource, CallOutcome (+22 more)

### Community 37 - "BridgeConfig"
Cohesion: 0.12
Nodes (35): BridgeConfig, bridges::ActiveModel, collect_json_files(), deserialize_address(), fixture_vars(), load_bridges_from_file(), load_bridges_impl(), load_chains_from_file() (+27 more)

### Community 38 - "services/utils.rs"
Cohesion: 0.14
Nodes (19): build_chain_bridge_filter(), build_chain_bridge_filter_all_indexed_is_none_even_without_opt_in(), build_chain_bridge_filter_default_sets_sorted_pairs(), build_chain_bridge_filter_include_unindexed_true_clears_restriction(), build_chain_bridge_filter_prunes_pairs_to_requested_bridge_ids(), checked_bridge_id(), checked_bridge_id_rejects_above_i32_max(), non_empty() (+11 more)

### Community 39 - "Workflow: implementation-plan.md"
Cohesion: 0.19
Nodes (13): Workflow: implementation-plan.md, coding-task-X.md Handoff Artifact, implementation-plan-X.md Artifact, Coding Handoff Must Be Self-Sufficient, Block the Coding Handoff on User Confirmation, Workflow: task-analysis.md, Align Evaluation Criteria with the Human, Task Analysis Outcome Status Vocabulary (+5 more)

### Community 40 - "Settings"
Cohesion: 0.09
Nodes (39): ConfigSettings, DatabaseSettings, main(), Error, Result, default_stats_chains_recalculation_period_secs(), default_stats_include_zero_chains(), default_swagger_path() (+31 more)

### Community 41 - "Codex Skill: gh-issue-publish"
Cohesion: 0.15
Nodes (24): Hook: allow-tmp-dirs.py (PreToolUse Bash), Hook: allow-tmp-writes.py (PreToolUse Write|Edit), Claude Skill: gh-issue-bug, Claude Skill: gh-issue-improvement, Claude Skill: gh-issue-publish, Codex Agent Interface: GitHub Issue Bug, Bug vs Improvement Issue Separation, Codex Skill: gh-issue-bug (+16 more)

### Community 42 - "Glossary"
Cohesion: 0.09
Nodes (24): Incoming ICTT Reconstruction / ICM Payload Decoding Entrypoints, Bridge, Bridge Contract, Configured Chain, Consolidation, Cross-Chain Message, Cross-Chain Transfer, Destination-Indexed Data (+16 more)

### Community 43 - "build_chain_node_configs"
Cohesion: 0.22
Nodes (18): build_chain_node_configs(), chain_fixture(), create_provider_pools_from_chains(), create_provider_pools_impl(), DynProvider, Ethereum, HashMap, test_build_chain_node_configs_header_prefix_produces_prefixed_value() (+10 more)

### Community 44 - "GET /api/v1/status/indexers"
Cohesion: 0.33
Nodes (7): GET /api/v1/status/indexers, GET /api/v1/status/indexing, GET /api/v1/status/indexers/{indexer_name}, StatusService, v1FullStatus, v1GetIndexingProgressResponse, v1IndexerStatus

### Community 45 - "package.json"
Cohesion: 0.09
Nodes (22): ts-proto, bugs, url, description, devDependencies, ts-proto, typescript, homepage (+14 more)

### Community 46 - "EventContext"
Cohesion: 0.15
Nodes (38): a_successful_drain_removes_the_queue_entry(), alter_amb(), apply_collected_signatures(), apply_validator_confirmation(), dispatch_transaction(), drain_pending_message_hash_events(), EventContext, expect_address() (+30 more)

### Community 47 - "bridge_contracts Is a Proxy, Not the Membership Set"
Cohesion: 0.12
Nodes (21): Runtime Verification Runbook, bridge_contracts Is a Proxy, Not the Membership Set, Canary vs Diagnostic Classification, IndexedChains::may_observe (in-memory eligibility), Observability Horizon Eligibility Rule, Query C — Hard Invariants Canary, Query D — Deferred Transfers Classified by Reason, Query E — Unindexed-Chain Edges Diagnostic (+13 more)

### Community 48 - "AvalancheIndexer"
Cohesion: 0.14
Nodes (13): AvalancheIndexer, BatchProcessContext, IndexerCleanupGuard, Arc, AtomicBool, AtomicU64, Drop, Error (+5 more)

### Community 49 - "Option"
Cohesion: 0.19
Nodes (20): build_reconstructed_transfer(), ClassifiedPayload, classify_payload(), destination_arm(), destination_arm_amount(), DestinationArm, dst_token_address(), Message (+12 more)

### Community 50 - "BufferItem"
Cohesion: 0.10
Nodes (27): Add, BufferItem, BTreeSet, HashMap, BridgeCounts, classify_item(), ConsolidationOutcome, Counts (+19 more)

### Community 51 - "stats_chains_bridge_filter.rs"
Cohesion: 0.28
Nodes (11): absent_and_blank_bridge_ids_match_the_unfiltered_baseline(), bridge_ids_scope_chain_candidates_even_when_unindexed_chains_are_included(), count_for(), DatabaseConnection, Option, Value, seed_by_bridge(), seed_global() (+3 more)

### Community 52 - "protocol_metadata.rs"
Cohesion: 0.15
Nodes (14): AvalancheIcmDestination, ProtocolMetadata, PublicMetadata, render_extra_nests_the_namespace_as_one_json_object(), round_trip_serializes_flat_with_reason_and_protocol_fields_at_same_level(), Option, Self, String (+6 more)

### Community 53 - "abi_registry.rs"
Cohesion: 0.20
Nodes (17): AbiRegistry, event_abi(), filter_for_chain_unions_topics_across_versions(), filter_for_chain_uses_precomputed_topic0_values_directly(), insert_contract_keeps_every_version_of_one_address_ordered_by_start_block(), insert_contract_rejects_two_versions_starting_at_the_same_block(), registry_with_versions(), resolve_log_reports_a_block_below_every_version_as_wrong_version() (+9 more)

### Community 54 - "progress.rs"
Cohesion: 0.19
Nodes (24): CatchupProgress, CheckpointCursors, cursors(), Option, Self, test_catchup_complete_is_false_when_floor_sits_above_chain_head(), test_catchup_complete_is_false_while_blocks_remain_even_with_no_failures(), test_catchup_complete_is_false_with_no_checkpoint_row() (+16 more)

### Community 55 - "TokenInfoService"
Cohesion: 0.17
Nodes (19): bare_token_info(), native_without_seed_or_provider_returns_typed_metadata(), Arc, Box, DateTime, DynProvider, Ethereum, HashMap (+11 more)

### Community 56 - "InterchainService"
Cohesion: 0.11
Nodes (25): Exposed REST Endpoints and Swagger URL, GET /api/v1/interchain/chains, GET /api/v1/interchain/messages/{message_id}, GET /api/v1/interchain/messages, GET /api/v1/interchain/messages:byAddress/{address}, GET /api/v1/interchain/messages:byTx/{tx_hash}, GET /api/v1/interchain/transfers, GET /api/v1/interchain/transfers:byAddress/{address} (+17 more)

### Community 57 - "Gotcha: Recoverable Message Fields Are Not A "Never Mirror" Case"
Cohesion: 0.25
Nodes (8): Gotcha: AMB Collision Replacement Must Delete Before Insert, Gotcha: AMB Source and Destination Events Can Arrive Out of Order, Gotcha: AMB Header Sender Is Not The Source Transaction Initiator, Gotcha: AMB Transfer Sides Are Nullable and Never Mirrored, crosschain_messages_on_conflict — keep_existing_if_terminal vs prefer_incoming (message_buffer/persistence.rs), Gotcha: recipient_address On A Terminal crosschain_messages Row Can Never Be Patched Later, Gotcha: Recoverable Message Fields Are Not A "Never Mirror" Case, SourceData::from_receive / from_execution (indexer/avalanche/consolidation.rs)

### Community 58 - "Consolidate"
Cohesion: 0.21
Nodes (10): Deserialize, Consolidate, Clone, Send, Sync, default_hot_ttl(), default_maintenance_interval(), Duration (+2 more)

### Community 59 - "projection.rs"
Cohesion: 0.12
Nodes (28): EdgeAmountSide, Model, Relation, BigDecimal, DateTime, Option, both_endpoints_known_falls_back_to_awaiting_confirmation(), conversion_never_falls_back_across_sides() (+20 more)

### Community 60 - "amb/consolidation.rs"
Cohesion: 0.09
Nodes (68): new_transfer(), ActiveModel, addr(), amount_to_decimal(), build_destination_only(), build_destination_only_transfer(), build_source_led(), build_transfer() (+60 more)

### Community 61 - "blockchain_id_resolver.rs"
Cohesion: 0.19
Nodes (12): Cache, CacheKey, BlockchainIdResolver, classify_data_api_result(), DataApiClassification, destination_negative_cache_does_not_leak_into_source_path(), other_error_response(), Resolution (+4 more)

### Community 62 - "Stats Projection"
Cohesion: 0.16
Nodes (19): ADR-004: Stats Observability Horizon; Asset Identity As Union-Find, IndexedChains (Stats Eligibility), Runtime Verification Runbook, Stats Entrypoints, Countable / Deferred (Stats), IndexedChains (glossary), Observability Horizon, Projection (+11 more)

### Community 63 - "stats.rs"
Cohesion: 0.15
Nodes (11): bridged_row_to_proto(), i64_to_u64_nonneg(), map_stats_error(), parse_optional_utc_date(), parse_optional_utc_date_rejects_malformed(), Error, NaiveDate, Option (+3 more)

### Community 64 - "Indexing Gaps, Retries, and Checkpoint Safety"
Cohesion: 0.25
Nodes (9): ADR-005: Failed-Range Ledger, Independent of Checkpoints, RangeDriver::run / run_retry_tick (indexer/range_driver.rs), Gotcha: The Retry Pass Starves The Forward Streams, And That Looks Like RPC Failure, AMB In-Memory Correlation Maps, FailureLedger / indexer_failures, Indexing Gaps, Retries, and Checkpoint Safety, LogBatch (Named Range), RangeDriver Retry Pass (+1 more)

### Community 65 - "BatchError"
Cohesion: 0.18
Nodes (10): attributed_ranges(), BatchError, MockLogsAction, RangeDriver<P>, Error, From, String, Vec (+2 more)

### Community 66 - "TestRangeProcessor"
Cohesion: 0.14
Nodes (15): AtomicUsize, RangeProcessor, ReplayTestMessage, DynProvider, Ethereum, Filter, HashMap, HashSet (+7 more)

### Community 67 - "Expected Skips Inside a Shared Transaction"
Cohesion: 0.12
Nodes (18): DecimalsConflict Domain Marker Type, Expected Skips Inside a Shared Transaction, Maintenance Transaction (messages, transfers, stats, cursor), Detect Conflicts with SELECT, Never a Failing INSERT, Never Assert a Delta on a Process-Wide Metric, STATS_EDGE_DECIMALS_CONFLICT_TOTAL (test-isolation case study), ADR-004 Stats Observability Horizon and Asset Union-Find, Asset-Identity Union-Find Merge (+10 more)

### Community 68 - "Result"
Cohesion: 0.07
Nodes (36): BridgedTokensListPagination, build_pagination_from_messages(), build_pagination_from_transfers(), BridgedTokensPaginationLogic, ListMarker, MessagesPaginationLogic, OutputPagination, OutputPagination<P> (+28 more)

### Community 69 - "retry_scheduler.rs"
Cohesion: 0.06
Nodes (80): BlockRange, difference_produces_expected_pieces(), FailedInterval, fold_adjacent(), merge_bounds(), overlaps(), overlaps_or_adjacent(), pre_union() (+72 more)

### Community 70 - "Async Patterns Rules"
Cohesion: 0.29
Nodes (8): CrosschainIndexer Trait, IndexerCleanupGuard Drop Guard, Async Patterns Rules, Graceful Shutdown and Cleanup Guards, Shared State (Arc RwLock), Start/Stop Invariants, Task Spawning and JoinHandle Rule, Async Trait Methods Rule

### Community 71 - "spawn_configured_indexers"
Cohesion: 0.16
Nodes (29): amb_contract_configs(), bridge(), build_amb_chain_configs(), build_avalanche_chain_configs(), build_xdai_chain_configs(), chain_config_fixture(), ChainPlan, dummy_provider() (+21 more)

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

### Community 76 - "AvalancheDataApiNetwork"
Cohesion: 0.20
Nodes (17): ClientWithMiddleware, AvalancheDataApiClient, AvalancheDataApiClientSettings, AvalancheDataApiNetwork, blockchain_id_to_cb58(), classify_error_response(), DataApiError, GetBlockchainByIdResponse (+9 more)

### Community 77 - "ADR-011: Cross-Asset Stats Edges And Per-Transfer Asset Linkage"
Cohesion: 0.08
Nodes (23): 1. The edge becomes binary, 2. The indexer declares linkage per transfer; the projection never infers it, 3. `mirror` keeps ADR-004's union-find; `conversion` resolves each side independently, 4. Two contradiction guards, both warn-and-continue, 5. The read path unions two directional projections, A per-bridge `supports_cross_asset_transfers()` capability / config default for `asset_linkage`, ADR-011: Cross-Asset Stats Edges And Per-Transfer Asset Linkage, Alternatives Considered (+15 more)

### Community 78 - "xdai/abi.rs"
Cohesion: 0.09
Nodes (37): AbiRegistry, amb_foreign_event_abi(), assert_canonical_topics(), chain_config(), ContractKind, dummy_provider(), foreign_event_abi(), foreign_proxy_address_returns_the_configured_address() (+29 more)

### Community 79 - "Checkpoint"
Cohesion: 0.20
Nodes (14): A failed floor write stays a warn (today), Checkpoint, Gotcha: AMB Queued Events Must Preserve Their Emitting Chain, Gotcha: A Checkpoint Certifies Scanning, Not Correctness, Gotcha: Checkpoint Stall When All Events Are Perpetually Filtered, FailureLedger — in-memory open-hole cache over indexer_failures, Gotcha: The Failure Ledger's Healthy Path Is DB-Free Only Because One Process Owns A Bridge, indexer_failures — the failed-range ledger (+6 more)

### Community 80 - "Database Subsystem: Schema and DB Interaction Layer"
Cohesion: 0.15
Nodes (14): Database Schema, Unindexed-Chain Read Filter, batched_upsert / run_in_batches, Database Subsystem: Schema and DB Interaction Layer, Hybrid Database Layer, InterchainDatabase Facade, Table Families, Unindexed-Chain Read Filter (+6 more)

### Community 81 - "send_event"
Cohesion: 0.18
Nodes (14): build_transfer(), AnnotatedEvent, ChainId, NaiveDateTime, ReceiveCrossChainMessage, Self, SendCrossChainMessage, send_event() (+6 more)

### Community 82 - "Architectural Decision Records Index"
Cohesion: 0.17
Nodes (13): ADR-001: Message Buffer Tiered Storage, ADR-002: Primary Chain Filtering for Unknown Chains, ADR-003: AMB Transfers Reconstructed From Events; Nullable Transfer Sides, ADR-006: Contract Versioning Resolved By Block, At Decode Time, Architectural Decision Records Index, ADR Template, AMB / Omnibridge Token Transfer Reconstruction, build_transfer / build_destination_only_transfer (+5 more)

### Community 83 - "BridgeType"
Cohesion: 0.22
Nodes (10): Model, Relation, DateTime, Option, String, BridgeType, deserialize_abi(), deserialize_bridge_type() (+2 more)

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

### Community 88 - ".fetch_token_info"
Cohesion: 0.09
Nodes (16): Erc20TokenInfoFetcher, DynProvider, Ethereum, Result, Vec, Erc20TokenHomeInfoFetcher, DynProvider, Ethereum (+8 more)

### Community 89 - "Codex Skill: research-scope"
Cohesion: 0.22
Nodes (11): Claude Skill: research-scope, Plan Review Gate Before Coding Task, Codex Agent Interface: Research Scope, Explicit Human Confirmation Before Persisting Research, Codex Skill: research-scope, Memory Bank: exploration-map.md, Memory Bank: research/README.md, Scope Research Workflow (+3 more)

### Community 90 - "indexing_coupling.py"
Cohesion: 0.83
Nodes (3): main(), parse(), summarize()

### Community 91 - ".check"
Cohesion: 0.18
Nodes (7): Health, HealthCheckRequest, HealthCheckResponse, HealthService, Request, Response, Result

### Community 92 - "Model"
Cohesion: 0.20
Nodes (8): Entity, Model, Relation, DateTime, Option, Related, RelationDef, String

### Community 93 - "CrosschainIndexerState"
Cohesion: 0.20
Nodes (5): CrosschainIndexerState, Display, Formatter, Result, String

### Community 94 - "Interchain Indexer"
Cohesion: 0.15
Nodes (12): Architecture, Build & Test, Configuration, Conventions, graphify, Interchain Indexer, Key Decisions, Known Gotchas (+4 more)

### Community 95 - "from_sql"
Cohesion: 0.18
Nodes (9): from_sql(), Migrator, Box, DbErr, MigrationTrait, MigratorTrait, Result, SchemaManager (+1 more)

### Community 96 - "ADR-004: Observability Horizon and Asset Union-Find"
Cohesion: 0.24
Nodes (10): ADR-002: Per-Bridge Chain Filtering, Fail-Fast Startup Validation of home_chain_id, Filter Order: Chain-Config Then Home-Chain, home_chain_id, process_unknown_chains, ADR-004: Observability Horizon and Asset Union-Find, Config Change Never Reinterprets Indexed History, include_unindexed_chains Read Filter (+2 more)

### Community 97 - "Event-Derived AMB Transfer Reconstruction"
Cohesion: 0.20
Nodes (10): ADR-003: AMB Transfers From Events; Nullable Sides, build_transfer (amb/consolidation.rs), Calldata Token Directional Ambiguity, Event-Derived AMB Transfer Reconstruction, Nullable Transfer Sides (Never Mirrored), Removal of the payload_processor Calldata Subsystem, TokensBridged (Destination Side), TokensBridgingInitiated (Source Side) (+2 more)

### Community 98 - "FailureLedger"
Cohesion: 0.22
Nodes (10): FailureLedger, indexer_failures Table, LogBatch (from_block, to_block, direction, logs), LogStream, One Scanner Per (bridge, chain), RangeDriver, RangeProcessor Trait, record Merges on Overlap or Adjacency (+2 more)

### Community 100 - "MockLogsService"
Cohesion: 0.15
Nodes (11): build_logs_response(), MockLogsService, Future, Log, Mutex, Poll, RequestPacket, ResponsePacket (+3 more)

### Community 101 - "failure_ledger/settings.rs"
Cohesion: 0.19
Nodes (18): default_backoff_base(), default_backoff_cap(), default_enabled(), default_max_chunks_per_pass(), default_record_retry_attempts(), default_record_retry_initial_backoff(), default_scan_interval(), default_split_after_attempts() (+10 more)

### Community 102 - ".record_indexer_failures"
Cohesion: 0.28
Nodes (11): indexer_failure_totals_sums_blocks_and_reports_the_oldest_created_at(), indexer_failures_rows_for(), open_indexer_failures_is_a_pure_read_with_no_side_effects(), pre_union_with_reason(), record_indexer_failures_disjointness_holds_after_mixed_merges(), record_indexer_failures_does_not_merge_across_a_real_gap(), record_indexer_failures_growth_bound_for_consecutive_realtime_failures(), record_indexer_failures_merges_overlapping_and_adjacent_ranges_into_one_row() (+3 more)

### Community 103 - "TokenInfoService and Token Metadata Enrichment Flow"
Cohesion: 0.11
Nodes (20): ChainInfoService, TokenInfoService, Union-Find Asset Merge, merge_assets / ensure_asset_for_transfer — weighted union-find over stats_assets, Gotcha: Stats Asset Mapping Conflicts Merge; Only Same-Chain Collisions Skip, Gotcha: Token Identity In stats_asset_tokens Is The ICTT Contract Address, Not The Wrapped ERC-20, avalanche_icm_blockchain_ids Table, Runtime Metadata Writes (+12 more)

### Community 104 - "Indexing Concurrency Model and Throughput"
Cohesion: 0.09
Nodes (22): After configuration tuning — 2026-08-20, 662 s window, After per-chain concurrency — 2026-08-21, Baseline — 2026-08-19, 250 s window, Change Triggers, Edge Cases / Gotchas, Failure Modes / Observability, How these measurements were taken, Indexing Concurrency Model and Throughput (+14 more)

### Community 105 - "K"
Cohesion: 0.22
Nodes (12): AbiRegistry<K>, ContractAbi, ContractAbi<K>, ContractVersion, LogResolution, B256, Default, Event (+4 more)

### Community 106 - ".new"
Cohesion: 0.49
Nodes (14): cancellation_during_retry_fetch_keeps_ledger_coverage(), missing_target_consumes_budget_without_starving_another_chain(), mock_provider(), retry_decision_time(), retry_pass_chunking_leaves_only_the_failing_remainder(), retry_pass_fairness_reaches_chunks_beyond_a_permanently_failing_prefix(), retry_pass_fetch_failure_re_records_the_chunk_without_escalating(), retry_pass_is_bounded_by_max_chunks_per_pass() (+6 more)

### Community 107 - "IndexedChains"
Cohesion: 0.16
Nodes (6): IndexedChains, HashMap, HashSet, Option, Vec, deferral_reason()

### Community 108 - ".new"
Cohesion: 0.33
Nodes (14): ChainIndexingProgress, collect_indexing_progress(), collect_indexing_progress_catchup_complete_requires_a_clean_failure_ledger(), collect_indexing_progress_pair_with_no_checkpoint_row_reports_zero_and_absent_updated_at(), collect_indexing_progress_pushes_both_filters_down_to_both_queries(), collect_indexing_progress_reports_zero_failed_blocks_with_no_rows(), collect_indexing_progress_sums_disjoint_indexer_failures_rows(), init_db() (+6 more)

### Community 109 - "logging.rs"
Cohesion: 0.15
Nodes (10): init_logs(), is_suppressed(), matches_target(), Error, JaegerSettings, Result, TracingSettings, Vec (+2 more)

### Community 110 - "Bridges"
Cohesion: 0.29
Nodes (7): Bridge `1` — AMB/Omnibridge, Bridge `2` — Avalanche ICTT, Bridge `3` — xDai Bridge, Bridges, Contracts of bridge `1`, Contracts of bridge `2`, Contracts of bridge `3`

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

### Community 116 - "Testnet set"
Cohesion: 0.20
Nodes (9): Bridge `1001` — AMB/Omnibridge, Bridges, Chain `10200` — Chiado, Chain `11155111` — Sepolia, Chains, Config files, Contracts of bridge `1001`, ENVs — `config/omnibridge` (+1 more)

### Community 117 - "AbiRegistry"
Cohesion: 0.29
Nodes (7): interchain_indexer_oldest_open_hole_age_seconds, Cyclic Retry-Pass Sweep with Shared Chunk Budget, AbiRegistry, AmbChainConfig (amb_proxies, mediators lists), resolve_log(chain_id, address, topic, block_number), topic0 Cannot Substitute for Block Resolution, interchain_indexer_amb_logs_dropped_wrong_version_total

### Community 118 - "Utc"
Cohesion: 0.10
Nodes (9): message_details_bridge_qualifier_contract(), TestDbGuard, seed_bridge_collision(), assert_projected_native_token_contract(), native_metadata(), ActiveModel, stats_bridged_tokens_preserve_seeded_native_type_through_projection(), stats_bridged_tokens_serialize_native_and_erc20_types_without_metadata() (+1 more)

### Community 119 - "ExampleIndexer"
Cohesion: 0.20
Nodes (8): ExampleIndexer, AtomicBool, AtomicU64, JoinHandle, NaiveDateTime, Option, RwLock, String

### Community 120 - "TokenType"
Cohesion: 0.18
Nodes (11): TokenType, Model, Relation, DateTime, Vec, Model, Relation, DateTime (+3 more)

### Community 121 - "amb/events.rs"
Cohesion: 0.15
Nodes (24): a_drain_keeps_events_queued_during_its_awaits(), a_failed_drain_keeps_the_queued_events_for_the_replay(), collected_signatures_identity(), confirmation_for(), DestinationKind, find_tokens_bridged_ignores_other_message_ids(), find_tokens_bridged_matches_by_message_id_independent_of_executor(), pending_with_unapplicable_signatures() (+16 more)

### Community 122 - "ENVs — `config/full-mainnet`"
Cohesion: 0.20
Nodes (10): Chain `100` — Gnosis, Chain `1` — Ethereum, Chain `43114` — Avalanche C-Chain, Chain `68414` — Henesys, Chain `8021` — NUMINE Mainnet, Chains, Config files, ENVs — `config/full-mainnet` (+2 more)

### Community 123 - "ADR-009: Stats Chains Global-Plus-Per-Bridge Snapshots"
Cohesion: 0.14
Nodes (13): ADR-009: Stats Chains Global-Plus-Per-Bridge Snapshots, Alternative 1 (solution 1): Visibility-only filter over the existing global snapshot, Alternative 2 (solution 3): Request-time exact `COUNT(DISTINCT ...)` over canonical rows, scoped by `bridge_ids`, Alternative 3 (solution 4): Persisted user-identity table, Alternative 4: Mergeable sketches (HyperLogLog) per bridge, Alternatives Considered, Consequences, Context (+5 more)

### Community 124 - "group_logs_by_transaction"
Cohesion: 0.38
Nodes (6): group_logs_by_transaction(), B256, HashMap, Log, Vec, test_group_logs_by_transaction_preserves_input_order()

### Community 125 - "AMB Destination-Only Consolidation and the Permanent Missing-Source Gap"
Cohesion: 0.13
Nodes (15): AMB Destination-Only Consolidation and the Permanent Missing-Source Gap, Change Triggers, Edge Cases / Gotchas, Failure Modes / Observability, Invariants, Key Types / Tables / Contracts, Open Questions, Production Evidence (+7 more)

### Community 126 - "XDaiIndexer"
Cohesion: 0.08
Nodes (61): affirmation_completed_log(), block_with_timestamp(), check_source_asset_matches_latest(), collected_signatures_before_its_source_is_queued_then_drained_to_ready_to_claim(), collected_signatures_log(), completed_transfer_reaches_two_assets_joined_by_a_conversion_edge(), dai_address(), dummy_provider() (+53 more)

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

### Community 131 - "Key"
Cohesion: 0.27
Nodes (8): is_canonical_message_hash_lookup(), remove_message_hash_lookup_if_canonical(), remove_message_hash_lookup_if_canonical_keeps_a_mapping_re_inserted_for_another_key(), remove_message_hash_lookup_if_canonical_removes_a_mapping_that_still_points_at_the_key(), DashMap, Key, BridgeId, Self

### Community 132 - "ADR-008: Per-Chain Concurrency Within A Bridge, Cooperative And Single-Task"
Cohesion: 0.18
Nodes (10): ADR-008: Per-Chain Concurrency Within A Bridge, Cooperative And Single-Task, Alternative 1: One `RangeDriver` per chain, joined at the call site, Alternative 2: `tokio::spawn` per chain, with a supervisor, Alternatives Considered, Consequences, Context, Decision, Negative / accepted (+2 more)

### Community 133 - "ApiError"
Cohesion: 0.40
Nodes (4): ApiError, Error, Self, String

### Community 135 - "ENVs — `config` (empty base set)"
Cohesion: 0.17
Nodes (12): `bridges[]`, `bridges[].contracts[]`, `chains[]`, `chains[].rpcs[<provider>]`, `chains[].rpcs[<provider>].api_key`, ENVs — `config` (empty base set), Field reference, Gotchas (+4 more)

### Community 136 - "CrosschainIndexerStatus"
Cohesion: 0.24
Nodes (4): CrosschainIndexerStatus, HashMap, NaiveDateTime, Value

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

### Community 154 - "native_id_blob"
Cohesion: 0.05
Nodes (101): Model, Relation, DateTime, Json, Option, Vec, MessageStatus, addr() (+93 more)

### Community 155 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 156 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 157 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 159 - "Asset Identity as Union-Find"
Cohesion: 0.29
Nodes (7): Maintenance Task and Consolidation Pass, Asset Identity as Union-Find, Eager Weighted Asset Merge, Fragmented Assets Defect, Conflicts Are Refusals, Never Transaction Errors, stats_processed Counting Marker, Drain Must Not Clear Its Queue Before Writes Succeed

### Community 190 - "Unresolved Avalanche Blockchain IDs"
Cohesion: 0.13
Nodes (15): Accepted Handling (Implemented), Change Triggers, Edge Cases / Gotchas, Failure Modes / Observability, Invariants, Key Types / Tables / Contracts, Nullable Source Impact Audit (2026-09-10), Production Evidence (+7 more)

### Community 201 - "xDai Bridge: Protocol Model, Contrast With Omnibridge, and Indexing Fit"
Cohesion: 0.06
Nodes (30): Architecture Fit, Asset model (DAI / USDS / native xDAI), Breaking changes, category A — `topic0` changes (detectable), Breaking changes, category B — `topic0` unchanged (silent), Change Triggers, Edge Cases / Gotchas, Ethereum → Gnosis (affirmation), Event grammar (current implementations) (+22 more)

### Community 204 - "CleanupGuard"
Cohesion: 0.25
Nodes (7): CleanupGuard, Arc, AtomicBool, Drop, JoinHandle, Option, RwLock

### Community 205 - "InterchainStatisticsService"
Cohesion: 0.14
Nodes (17): just check-envs / just generate-envs Requirement on ENV Changes, GET /api/v1/stats/chains, GET /api/v1/stats/common, GET /api/v1/stats/daily, GET /api/v1/stats/chain/{chain_id}/messages-paths/received, GET /api/v1/stats/chain/{chain_id}/messages-paths/sent, InterchainStatisticsService, v1GetChainsStatsResponse (+9 more)

### Community 206 - ".get_checkpoint"
Cohesion: 0.23
Nodes (10): indexer_failures_and_mark_catchup_complete_are_independent_records(), list_indexer_checkpoints_filters_and_orders_deterministically(), lower_catchup_floor_is_idempotent_and_never_raises_or_touches_max_cursor(), mark_catchup_complete_upserts_empty_range_checkpoint(), mark_catchup_complete_without_safe_realtime_cursor_does_not_insert(), seed_catchup_floor_conflict_clause_touches_only_the_floor_and_is_idempotent(), seed_catchup_floor_does_not_lower_an_already_advanced_floor(), seed_catchup_floor_heals_a_stored_zero() (+2 more)

### Community 207 - "Address"
Cohesion: 0.44
Nodes (9): Bytes, encode_transferrer(), execution_failed(), multi_hop_call_payload(), multi_hop_send_payload(), register_remote_payload(), Address, single_hop_call_payload() (+1 more)

### Community 208 - "secret.rs"
Cohesion: 0.11
Nodes (20): Debug, E, redact_urls(), redact_urls_does_not_panic_on_multi_byte_input(), redact_urls_handles_a_realistic_transport_error_rendering(), redact_urls_handles_two_urls_in_one_string(), redact_urls_strips_a_path_embedded_secret(), redact_urls_strips_a_query_embedded_secret() (+12 more)

### Community 209 - "BridgedTokensSortField"
Cohesion: 0.21
Nodes (11): build_pagination_from_bridged_tokens(), count_column(), cursor_where_next(), cursor_where_prev(), Value, BridgedTokensSortField, StatsSortOrder, Option (+3 more)

### Community 210 - "ENVs — `config/avalanche`"
Cohesion: 0.18
Nodes (11): Bridge `2` — Avalanche ICTT, Bridges, Chain `43114` — Avalanche C-Chain, Chain `68414` — Henesys, Chain `8021` — NUMINE Mainnet, Chains, Config files, Contracts of bridge `2` (+3 more)

### Community 211 - "ENVs — `config/full-testnet`"
Cohesion: 0.22
Nodes (8): Bridge `1001` — AMB/Omnibridge, Bridges, Chain `10200` — Chiado, Chain `11155111` — Sepolia, Chains, Config files, Contracts of bridge `1001`, ENVs — `config/full-testnet`

### Community 212 - "BlockscoutTokenInfoClientSettings"
Cohesion: 0.26
Nodes (11): BlockscoutTokenInfoClientSettings, default_icon_retry_interval(), default_ignore_chains(), default_onchain_retry_interval(), Default, Duration, Option, Self (+3 more)

### Community 213 - "Mainnet set"
Cohesion: 0.25
Nodes (8): Bridge `1` — AMB/Omnibridge, Bridges, Chain `100` — Gnosis, Chain `1` — Ethereum, Chains, Config files, Contracts of bridge `1`, Mainnet set

### Community 214 - ".create_stats_asset"
Cohesion: 0.36
Nodes (6): stats_asset_delete_cascades(), stats_asset_insert_and_get(), stats_link_token_without_tokens_row(), stats_migration_applies(), stats_reject_same_token_two_assets(), stats_reject_two_tokens_same_chain_one_asset()

### Community 215 - "BufferItem<T>"
Cohesion: 0.13
Nodes (11): BufferItem<T>, now_naive_utc(), BlockNumber, BufferItemVersion, ChainId, NaiveDateTime, Result, Self (+3 more)

### Community 216 - "AmbHeader"
Cohesion: 0.29
Nodes (10): AmbHeader, parse_amb_header(), parse_chain_id(), parse_modern_header(), Address, B256, Result, test_parse_modern_header_extracts_chain_ids_and_payload_offset() (+2 more)

### Community 217 - "amb/version.rs"
Cohesion: 0.27
Nodes (8): amb_side_for_abi_infers_side_from_configured_event_set(), amb_grammar_for(), AmbGrammar, AmbVersion, mediator_grammar_for(), MediatorGrammar, MediatorVersion, Result

### Community 218 - "server.rs"
Cohesion: 0.27
Nodes (13): FailureTotalsResult, GaugeValue, IndexingTarget, gauge_refresh_values(), gauge_refresh_values_returns_none_on_error_so_callers_leave_values_untouched(), gauge_refresh_values_uses_the_aggregate_for_a_pair_present_in_totals(), gauge_refresh_values_zeroes_a_configured_pair_absent_from_totals(), refresh_failure_ledger_gauges() (+5 more)

### Community 219 - "Query F — Incoming ICTT Reconstruction Diagnostic"
Cohesion: 0.29
Nodes (7): Functional Style for Boolean Logic, pending_messages Cold Storage Retention Is Load-Bearing, Query F — Incoming ICTT Reconstruction Diagnostic, Query G — pending_messages Backlog Trend, reconstruct_incoming_ictt_transfers Kill Switch, process_unknown_chains / home_chain_id Filter Pair, reconstruct_incoming_ictt_transfers Bridge Flag

### Community 220 - "Migration"
Cohesion: 0.36
Nodes (5): Migration, DbErr, MigrationTrait, Result, SchemaManager

### Community 221 - "resolve-blockchain-id.rs"
Cohesion: 0.48
Nodes (5): main(), parse_args(), parse_blockchain_id(), Result, String

### Community 222 - "Rust Style Rules"
Cohesion: 0.08
Nodes (28): Error Handling Rules, anyhow::Result for Internal Code, API Error Sanitization, Checked/Saturating Arithmetic and Euclidean Division, Always Add Context When Propagating, Log Errors at the Handling Point, Panic Avoidance in Runtime Paths, thiserror for Public API Error Types (+20 more)

### Community 223 - "Chains"
Cohesion: 0.22
Nodes (9): Bridge `3` — xDai Bridge, Bridges, Chain `100` — Gnosis, Chain `1` — Ethereum, Chains, Chains, Config files, Contracts of bridge `3` (+1 more)

### Community 224 - "prelude.rs"
Cohesion: 0.04
Nodes (39): Date, Model, Relation, DateTime, Option, String, Vec, Model (+31 more)

### Community 225 - "AvalancheIndexerSettings"
Cohesion: 0.36
Nodes (7): AvalancheIndexerSettings, default_batch_size(), default_pull_interval(), default_receipt_concurrency(), Default, Duration, Self

### Community 226 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 227 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 229 - "MessageBuffer (Tiered Storage)"
Cohesion: 0.12
Nodes (20): upsert_cursors — GREATEST-only cursor maintenance writer (cannot lower a floor), AvalancheIndexer, High-Level Data Flow, LogStream, MessageBuffer (Tiered Storage), Bridge Filtering Entrypoints, Message Buffer, Teleporter / ICM (+12 more)

### Community 230 - "Testing Rules"
Cohesion: 0.29
Nodes (8): Testing Rules, Feature-Flagged E2E Tests (avalanche-e2e), just test-with-db vs just test, fill_mock_interchain_database Fixtures, Test Attributes (tokio::test, ignore, rstest), Test Naming Format, TestDbGuard Isolated Database Tests, Prefer Repo-Native Verification Commands over cargo test

### Community 231 - "ADR-010: Unresolved Avalanche Destinations And Reusable Protocol Metadata"
Cohesion: 0.15
Nodes (10): 1. Preserve unresolved destinations in canonical messages, 2. Preserve current unresolved-source behavior, 3. Keep metadata sparse, 4. Make the representation reusable, ADR-010: Unresolved Avalanche Destinations And Reusable Protocol Metadata, Alternatives Considered, Consequences, Context (+2 more)

### Community 232 - "Exploration Map"
Cohesion: 0.15
Nodes (14): API Serving Entrypoints, Avalanche Indexing Entrypoints, Common Indexer Architecture Entrypoints, Config Loading Entrypoints, Database Schema and Migrations Entrypoints, Exploration Map, Whole-System Entrypoints, Configuration Model (+6 more)

### Community 233 - "ChainConfig"
Cohesion: 0.13
Nodes (21): ApiKeyConfig, ApiKeyLocation, BridgeContractConfig, chain_declares_api_key(), ChainConfig, chains::ActiveModel, encode_path_segment(), ExplorerConfig (+13 more)

### Community 234 - "ExampleIndexerSettings"
Cohesion: 0.43
Nodes (5): default_fetch_interval(), ExampleIndexerSettings, Default, Duration, Self

### Community 236 - "overlap_warning.rs"
Cohesion: 0.18
Nodes (3): overlap_transition(), OverlapTransition, Option

### Community 237 - "interchain_service.rs"
Cohesion: 0.52
Nodes (6): DbTokenType, TokenInfoModel, token_info_logic_to_proto(), token_info_logic_to_proto_omits_address_hash_exactly_for_native(), token_model(), token_type_to_proto()

### Community 238 - "chains_endpoint_filters.rs"
Cohesion: 0.17
Nodes (4): chain_ids_of(), String, Value, Vec

### Community 239 - "CrosschainIndexer Worker"
Cohesion: 0.40
Nodes (6): Project-Specific Naming Conventions, BridgeContractIndexer Worker, Common Design Principles, CrosschainIndexer Worker, MessageCollector Worker, TokenFetcher Worker

### Community 241 - "run"
Cohesion: 0.20
Nodes (9): HttpRouter, Router, Arc, Error, PathBuf, Result, run(), spawn_stats_chains_recalculation_worker() (+1 more)

### Community 245 - "seed_pre_xdai_schema"
Cohesion: 0.25
Nodes (12): assert_migrated_tokens(), EmptyMigrator, merged_migration_does_not_reference_its_own_new_enum_values_incrementally(), Box, DatabaseConnection, DbErr, MigrationTrait, MigratorTrait (+4 more)

### Community 246 - "helpers/mod.rs"
Cohesion: 0.24
Nodes (9): get_raw(), init_db(), init_interchain_indexer_server(), F, StatusCode, String, TestDbGuard, Url (+1 more)

### Community 247 - "Workflow: pr-description.md"
Cohesion: 0.28
Nodes (9): Workflow: pr-description.md, Explicit None. for API / ENV / Migration Sections, pr-description.md Artifact, Reflect the Implemented State, Not the Plan, Workflow: solution-review.md, Mismatch vs Risk vs Verification Gap vs Ambiguity, Do Not Silently Repair Missing Implementation During Review, review.md Artifact (+1 more)

### Community 250 - "Migration"
Cohesion: 0.36
Nodes (5): Migration, DbErr, MigrationTrait, Result, SchemaManager

### Community 251 - "Migration"
Cohesion: 0.36
Nodes (5): Migration, DbErr, MigrationTrait, Result, SchemaManager

### Community 253 - "serialize"
Cohesion: 0.33
Nodes (7): deserialize(), D, Error, Result, S, serialize(), Ok

### Community 254 - "LookupError"
Cohesion: 0.33
Nodes (5): LookupError, Error, From, Self, UnresolvedReason

### Community 256 - "Claude Code Overrides (project CLAUDE.md)"
Cohesion: 0.50
Nodes (5): Claude Code Overrides (project CLAUDE.md), Canonical Workflow Delegation (thin skill wrappers), Memory Bank: adr/README.md, Memory Bank: rules/ (coding conventions), Memory Bank: workflows/ (canonical task procedures)

### Community 257 - "indexer_checkpoints.rs"
Cohesion: 0.40
Nodes (4): Model, Relation, DateTime, Option

### Community 258 - "Self"
Cohesion: 0.29
Nodes (4): RangeDriver, Arc, P, Self

### Community 259 - "ActiveValue"
Cohesion: 0.67
Nodes (3): ActiveValue, T, set_value()

## Knowledge Gaps
- **349 isolated node(s):** `gh-issue-publish.sh script`, `Relation`, `ActiveModel`, `Relation`, `ActiveModel` (+344 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **32 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `InterchainDatabase` connect `InterchainDatabase` to `StatsService`, `.new`, `persistence.rs`, `init_db`, `StatusServiceImpl`, `.new`, `log_stream.rs`, `ChainInfoService`, `.upsert_chains`, `indexers.rs`, `MessageBuffer`, `InterchainServiceImpl`, `AmbIndexer`, `AvalancheIndexer`, `TokenInfoService`, `blockchain_id_resolver.rs`, `Result`, `retry_scheduler.rs`, `.get_checkpoint`, `.create_stats_asset`, `server.rs`, `.record_indexer_failures`, `.new`, `ExampleIndexer`, `recompute_stats_chains_distinct_users_and_merges_message_transfer_sides`, `XDaiIndexer`?**
  _High betweenness centrality (0.192) - this node is a cross-community bridge._
- **Why does `Key` connect `Key` to `AmbIndexer`, `TestRangeProcessor`, `XDaiIndexer`, `persistence.rs`, `MessageBuffer`, `EventContext`, `send_event`, `Option`, `BufferItem`, `native_id_blob`, `avalanche/consolidation.rs`, `amb/consolidation.rs`, `avalanche/mod.rs`?**
  _High betweenness centrality (0.090) - this node is a cross-community bridge._
- **Why does `init_db()` connect `init_db` to `StatsService`, `.new`, `persistence.rs`, `InterchainDatabase`, `ChainInfoService`, `bridged_tokens_query.rs`, `stats_chains_query.rs`, `.upsert_chains`, `.new`, `range_driver.rs`, `MessageBuffer`, `AmbIndexer`, `BufferItem`, `TokenInfoService`, `retry_scheduler.rs`, `.get_checkpoint`, `.create_stats_asset`, `.record_indexer_failures`, `.new`, `recompute_stats_chains_distinct_users_and_merges_message_transfer_sides`, `XDaiIndexer`?**
  _High betweenness centrality (0.079) - this node is a cross-community bridge._
- **Are the 271 inferred relationships involving `init_db()` (e.g. with `bridged_tokens_aggregation_input_output_total()` and `bridged_tokens_cross_asset_edges_split_by_focal_chain()`) actually correct?**
  _`init_db()` has 271 INFERRED edges - model-reasoned connections that need verification._
- **Are the 90 inferred relationships involving `fill_mock_interchain_database()` (e.g. with `counters_cover_all_filters()` and `get_crosschain_message_native_collision_is_ambiguous_until_qualified()`) actually correct?**
  _`fill_mock_interchain_database()` has 90 INFERRED edges - model-reasoned connections that need verification._
- **What connects `gh-issue-publish.sh script`, `Relation`, `ActiveModel` to the rest of the system?**
  _349 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `interchain-indexer-filters/src/lib.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.09634146341463415 - nodes in this community are weakly interconnected._