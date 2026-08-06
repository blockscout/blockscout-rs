# Graph Report - .  (2026-08-06)

## Corpus Check
- 250 files · ~229,019 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 3326 nodes · 7904 edges · 204 communities (177 shown, 27 thin omitted)
- Extraction: 93% EXTRACTED · 7% INFERRED · 0% AMBIGUOUS · INFERRED: 554 edges (avg confidence: 0.83)
- Token cost: 813,659 input · 0 output

## Community Hubs (Navigation)
- AMB Message Consolidation
- RPC Provider Layering & Failover
- Stats Read Service
- Counters & Checkpoint Queries
- Batched Upsert Persistence
- IndexedChains Eligibility Set
- Interchain Database Facade
- Database Test Fixtures
- AMB ABI Registry & Versioning
- Indexer Status & Progress Entities
- Cursor Pagination Tokens
- Indexer Lifecycle & Cleanup Guard
- Stats Asset Edge Projection
- Env Override Deep Merge
- Config Parsing & Defaults
- Log Stream & Batch Fetching
- Chains Entity & Chain Info Service
- Blockscout Token Info Client
- Bridged Tokens Query
- Stats Chains Listing Query
- Message Paths Stats Queries
- Scan Floor Reconciliation ADR
- Message & Transfer SQL Filters
- Interchain gRPC Service
- Range Processor Retry Escalation
- Indexer Target Enumeration
- Codebase Review & Blockchain ID Risks
- Avalanche ICM Consolidation
- Tiered Message Buffer
- Checkpoint Cursor Updates
- Avalanche Log Handlers
- Indexing Progress API Semantics
- Interchain Proto Request Types
- Statistics gRPC Service
- AMB Indexer Runtime
- ICTT Payload Decoding
- Avalanche Domain Types
- Indexer Lifecycle & Cleanup Guard
- Chain-Bridge Filter Helpers
- Coding Handoff Standards
- Server Entrypoint & E2E
- GitHub Issue Skills & Hooks
- Exploration Map & Glossary
- Config-to-Model Conversion
- Statistics REST Endpoints
- Proto TypeScript Package
- Claude Task Workflow Skills
- Runtime Verification Canaries
- Indexing Target Planning
- Server Test Helpers
- Maintenance Pass & Buffer Eviction
- AMB Indexer Failure Tests
- Avalanche Range Processor
- Failure Ledger Core
- Catch-up Progress Computation
- Buffer Item Dirty Tracking
- REST Endpoint Surface
- Architecture Overview
- Asset Union-Find & Token Identity
- Error Handling Conventions
- Range Driver Chunking Tests
- Range Driver Retry Ticks
- Observability Horizon ADR
- Incoming ICTT Reconstruction
- Avalanche Transfer Building
- Avalanche Indexer Runtime
- Failure Retry Settings
- Transaction Skip & Testing Rules
- Block Interval Arithmetic
- Failed Interval Backoff Policy
- Maintenance Plan & Hot Eviction
- Stats Proto Mapping
- Cursor Review & Issue Skills
- Contract Version Windows ADR
- Docker Compose Stack
- Canonical Write Path & Consolidate Contract
- Avalanche Data API Client
- AMB Dispatch Mock Service
- Crosschain Messages Entity
- AMB Event Ordering Gotchas
- Blockchain ID Resolution & DB Gotchas
- Two-Channel Config Model
- ADR Index & AMB Token Reconstruction
- Bridges Entity
- Receipt Fetching
- Codex Task Skills
- Proto Build Script
- Bridge Proto Mapping
- Mock Logs RPC Service
- Bridge Contract Config
- Research Scope Skill
- Health Check Service
- Stats Assets Entity
- Bridged Tokens Pagination Cursor
- Indexer Failure Merge Semantics
- Migration Runner
- Primary Chain Filtering ADR
- Event-Derived AMB Transfers ADR
- Failed-Range Ledger Mechanism
- Blockchain ID Resolver Cache
- AMB Indexer Settings
- Datetime Byte Encoding Utils
- Failed-Range Ledger & Retry Starvation
- Crosschain Transfers Entity
- Message Lifecycle & Async Rules
- AMB Ordering & Terminal-Row Gotchas
- Testing Conventions
- Bridge Contracts Entity
- Stats Asset Persistence
- Avalanche Indexer Settings
- Message Buffer Settings
- Initial Schema Migration
- Stats Tables Migration
- AMB Indexer Migration
- Read Filters Migration
- Message Buffer Tiering ADR
- Maintenance Consolidation Rules
- Contract Version Resolution Metrics
- Consolidate Trait Bounds
- AMB Message Anomalies Entity
- Tokens Entity
- Blockchain ID Resolver CLI
- Total Counters & Joined Transfers
- Stats Chains Recomputation
- Transaction Log Grouping
- Status REST Endpoints
- Worker Roles & Design Principles
- AMB Confirmations Entity
- Avalanche ICM Blockchain IDs Entity
- Bridge Txs Entity
- Indexer Failures Entity
- Pending Messages Entity
- Token Info Stats Enrichment
- API Error Type
- Stats Messages Days Entity
- Stats Asset Tokens Entity
- Bulk Upsert & SeaORM Gotchas
- Token Info Consistency Gotchas
- Chain Info Proto Mapping
- Tmp Mkdir Permission Hook
- Tmp Write Permission Hook
- Entity Relation Defs (a)
- Entity Relation Defs (b)
- Entity Relation Defs (c)
- Entity Relation Defs (d)
- Entity Relation Defs (e)
- Entity Relation Defs (f)
- Entity Relation Defs (g)
- Entity Relation Defs (h)
- Entity Relation Defs (i)
- Entity Relation Defs (j)
- Entity Relation Defs (k)
- Entity Relation Defs (l)
- Stats Messages Entity
- Entity Relation Defs (m)
- Entity Relation Defs (n)
- Entity Relation Defs (o)
- Checkpoint Cursor Validation
- ActiveValue Helper
- BigDecimal Rename Script
- Env Override Gotchas
- HTTP Query Param Gotchas
- ActiveModelBehavior (a)
- ActiveModelBehavior (b)
- ActiveModelBehavior (c)
- ActiveModelBehavior (d)
- ActiveModelBehavior (e)
- ActiveModelBehavior (f)
- ActiveModelBehavior (g)
- ActiveModelBehavior (h)
- ActiveModelBehavior (i)
- ActiveModelBehavior (j)
- ActiveModelBehavior (k)
- ActiveModelBehavior (l)
- ActiveModelBehavior (m)
- ActiveModelBehavior (n)
- ActiveModelBehavior (o)
- ActiveModelBehavior (p)
- ActiveModelBehavior (q)
- ActiveModelBehavior (r)
- ActiveModelBehavior (s)
- Started-At-Block Validation
- GitHub Issue Publish Script
- Migration Update Script
- Cleanup Guard Panic Gotcha

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

## Communities (204 total, 27 thin omitted)

### Community 0 - "AMB Message Consolidation"
Cohesion: 0.05
Nodes (115): DynSolValue, addr(), amount_to_decimal(), build_destination_only(), build_destination_only_transfer(), build_source_led(), build_transfer(), destination() (+107 more)

### Community 1 - "RPC Provider Layering & Failover"
Cohesion: 0.06
Nodes (61): Context, DefaultClock, InMemoryState, base_node(), block_number_packet(), build_layered_http_provider(), build_layered_provider_from_services(), build_mock_error_response() (+53 more)

### Community 2 - "Stats Read Service"
Cohesion: 0.05
Nodes (47): BridgedTokenListRow, kickoff_enrichment_no_token_service_is_noop(), Arc, DatabaseTransaction, DbErr, Default, NaiveDate, Option (+39 more)

### Community 3 - "Counters & Checkpoint Queries"
Cohesion: 0.09
Nodes (70): counters_cover_all_filters(), default_filter(), indexer_failure_totals_sums_blocks_and_reports_the_oldest_created_at(), indexer_failures_and_mark_catchup_complete_are_independent_records(), list_indexer_checkpoints_filters_and_orders_deterministically(), lower_catchup_floor_is_idempotent_and_never_raises_or_touches_max_cursor(), mark_catchup_complete_upserts_empty_range_checkpoint(), mark_catchup_complete_without_safe_realtime_cursor_does_not_insert() (+62 more)

### Community 4 - "Batched Upsert Persistence"
Cohesion: 0.07
Nodes (70): A, batch_size_for_width(), batched_upsert(), ConnectionTrait, DbErr, F, OnConflict, Result (+62 more)

### Community 5 - "IndexedChains Eligibility Set"
Cohesion: 0.06
Nodes (48): Column, Entity, chain_unindexed_condition(), cols(), IndexedChains, message_countable_condition(), render(), Clone (+40 more)

### Community 6 - "Interchain Database Facade"
Cohesion: 0.09
Nodes (31): Fn, BackfillStatsReport, build_all_time_message_paths_query(), build_bounded_message_paths_query(), CrosschainMessageLookup, expect_found(), get_crosschain_message_native_collision_is_ambiguous_until_qualified(), get_crosschain_message_numeric_collision_is_ambiguous_until_qualified() (+23 more)

### Community 7 - "Database Test Fixtures"
Cohesion: 0.09
Nodes (64): completed_message(), completed_message_at(), completed_message_without_indexed_source(), insert_already_processed_bridging_transfer(), load_native_id_map_filters_missing_native_ids(), push_indexed_pairs_predicate(), DatabaseConnection, RwLock (+56 more)

### Community 8 - "AMB ABI Registry & Versioning"
Cohesion: 0.07
Nodes (47): AbiRegistry, amb_side_for_abi(), amb_side_for_abi_infers_side_from_configured_event_set(), ContractAbi, ContractKind, ContractVersion, event_abi(), filter_for_chain_unions_topics_across_versions() (+39 more)

### Community 9 - "Indexer Status & Progress Entities"
Cohesion: 0.06
Nodes (51): ChainIndexingProgress, FailureTotalsResult, FullStatus, GaugeValue, GetFullStatusRequest, GetIndexingProgressRequest, GetIndexingProgressResponse, GetStatusRequest (+43 more)

### Community 10 - "Cursor Pagination Tokens"
Cohesion: 0.09
Nodes (25): BridgedTokensListPagination, build_pagination_from_messages(), build_pagination_from_transfers(), ListMarker, MessagesPaginationLogic, OutputPagination, OutputPagination<P>, PaginationDirection (+17 more)

### Community 11 - "Indexer Lifecycle & Cleanup Guard"
Cohesion: 0.06
Nodes (38): DatabaseSettings, ExampleIndexer, Arc, AtomicBool, AtomicU64, DynProvider, Error, Ethereum (+30 more)

### Community 12 - "Stats Asset Edge Projection"
Cohesion: 0.12
Nodes (50): EdgeKey, EdgeAmountSide, Model, Relation, BigDecimal, DateTime, Option, asset_has_token_on_chain() (+42 more)

### Community 13 - "Env Override Deep Merge"
Cohesion: 0.13
Nodes (51): AppliedOverride, apply(), apply_env_overrides(), apply_patch(), apply_to_keyed_array(), apply_to_named_map_array(), ArrayRule, ArrayRules (+43 more)

### Community 14 - "Config Parsing & Defaults"
Cohesion: 0.08
Nodes (33): D, collect_json_files(), deserialize_abi(), deserialize_address(), deserialize_bridge_type(), fixture_vars(), load_bridges_from_file(), load_bridges_impl() (+25 more)

### Community 15 - "Log Stream & Batch Fetching"
Cohesion: 0.07
Nodes (34): build_log_stream_for_chain(), BoxStream, Duration, DynProvider, Ethereum, Filter, Result, fetch_logs() (+26 more)

### Community 16 - "Chains Entity & Chain Info Service"
Cohesion: 0.08
Nodes (31): Entity, Model, Relation, DateTime, Json, Option, Related, RelationDef (+23 more)

### Community 17 - "Blockscout Token Info Client"
Cohesion: 0.10
Nodes (28): Client, BlockscoutTokenInfo, BlockscoutTokenInfoClient, BlockscoutTokenInfoError, CachedIconResult, Arc, Error, HashMap (+20 more)

### Community 18 - "Bridged Tokens Query"
Cohesion: 0.16
Nodes (41): add_asset_edges_on_bridge(), bridged_tokens_aggregation_input_output_total(), bridged_tokens_default_excludes_edge_unindexed_for_its_bridge(), bridged_tokens_empty_configured_pairs_restricts_nothing(), bridged_tokens_last_page(), bridged_tokens_name_sort_nulls_and_empty_last(), bridged_tokens_opt_in_returns_same_rows_until_projection_widens(), bridged_tokens_pagination_after_bridge_collapse() (+33 more)

### Community 19 - "Stats Chains Listing Query"
Cohesion: 0.11
Nodes (39): StatsSortOrder, cursor_where_next(), cursor_where_prev(), forward_order_clause(), inverse_order_clause(), list_stats_chains(), ConnectionTrait, DatabaseConnection (+31 more)

### Community 20 - "Message Paths Stats Queries"
Cohesion: 0.16
Nodes (33): message_paths_bounded_queries_apply_open_and_half_open_ranges(), message_paths_bounded_queries_sum_daily_rows_and_order_deterministically(), message_paths_default_excludes_pair_unindexed_for_its_bridge(), message_paths_empty_configured_pairs_restricts_nothing(), message_paths_include_zero_bounded_counterparty_expands_requested_known_rows_only(), message_paths_include_zero_bounded_queries_expand_known_chains(), message_paths_include_zero_counterparty_expands_requested_known_rows_only(), message_paths_include_zero_incoming_all_time_expands_known_chains() (+25 more)

### Community 21 - "Scan Floor Reconciliation ADR"
Cohesion: 0.08
Nodes (38): ADR-007: The Scan Floor Is Reconciled Against The Checkpoint, Not Against bridge_contracts (Accepted, with stated expiry), Alternative 1 (rejected): Withhold the contracts upsert when the previous floor is unknown, Alternative 2 (rejected for now, the correct long-term shape): persist the pair's floor in its own column, Alternative 3 (rejected): also lower catchup_max_cursor to the old floor, Neutral consequence: bridge_contracts returns to being purely diagnostic, bridges_pending_contracts_upsert — REMOVED by ADR-007 (evidence-preserving withhold mechanism and its startup coupling), catchup_max_cursor is deliberately untouched — lowering a floor causes no rescan in the current design, ChainPlan::floor_contracts — survives with one consumer, start_block() (+30 more)

### Community 22 - "Message & Transfer SQL Filters"
Cohesion: 0.10
Nodes (31): messages_where(), Condition, String, sql_messages(), sql_transfers(), test_messages_condition_both_directions_no_focal(), test_messages_condition_bridge_only(), test_messages_condition_counterparties_only() (+23 more)

### Community 23 - "Interchain gRPC Service"
Cohesion: 0.13
Nodes (24): AddressInfo, BridgeInfo, CrosschainMessageModel, CrosschainTransferModel, DbMessageStatus, hex_string_opt(), Option, String (+16 more)

### Community 24 - "Range Processor Retry Escalation"
Cohesion: 0.12
Nodes (24): AtomicUsize, empty_batch(), escalates_and_stops_consuming_when_record_keeps_failing(), healthy_path_issues_no_ledger_write_statement(), mock_provider(), RangeProcessor, records_a_failed_batch_and_resolves_it_on_a_later_success(), replaying_an_already_persisted_interval_leaves_the_message_row_intact() (+16 more)

### Community 25 - "Indexer Target Enumeration"
Cohesion: 0.16
Nodes (31): BridgeType, bridge(), chain_config_fixture(), checkpoint_floor(), dummy_provider(), init_db(), omnibridge_config_path(), progress_for() (+23 more)

### Community 26 - "Codebase Review & Blockchain ID Risks"
Cohesion: 0.09
Nodes (32): Codebase Review, Complexity Hotspots, Onboarding Friction, Recommended Research Priorities, Message Lifecycle Entrypoints, Home Chain, Avalanche Data API, External Systems (+24 more)

### Community 27 - "Avalanche ICM Consolidation"
Cohesion: 0.26
Nodes (30): Bytes, addr(), call_outcome_transfer(), encode_transferrer(), execution_failed(), execution_succeeded(), key(), message_id() (+22 more)

### Community 28 - "Tiered Message Buffer"
Cohesion: 0.15
Nodes (23): FnOnce, DummyMessage, MessageBuffer, MessageBuffer<T>, new_buffer(), Arc, BlockNumber, ChainId (+15 more)

### Community 29 - "Checkpoint Cursor Updates"
Cohesion: 0.16
Nodes (24): FnMut, block_sets_bootstrap_delegates(), block_sets_extend_delegates(), BlockSets, bootstrap(), BootstrapCase, Cursor, cursor_blocks_builder_keys_iterator() (+16 more)

### Community 30 - "Avalanche Log Handlers"
Cohesion: 0.19
Nodes (29): gate_receiver_ictt_arm(), handle_log(), handle_message_executed(), handle_message_execution_failed(), handle_receive_cross_chain_message(), handle_send_cross_chain_message(), LogHandleContext, parse_execution_outcome_log() (+21 more)

### Community 31 - "Indexing Progress API Semantics"
Cohesion: 0.09
Nodes (30): Config Structs Use deny_unknown_fields, Functional Style for Boolean Logic, pending_messages Cold Storage Retention Is Load-Bearing, Query F — Incoming ICTT Reconstruction Diagnostic, Query G — pending_messages Backlog Trend, reconstruct_incoming_ictt_transfers Kill Switch, database service (UBI stack), interchain-indexer service (UBI stack, built from Dockerfile) (+22 more)

### Community 32 - "Interchain Proto Request Types"
Cohesion: 0.14
Nodes (20): GetBridgesRequest, GetBridgesResponse, GetChainsRequest, GetChainsResponse, GetMessageDetailsRequest, GetMessagesByAddressRequest, GetMessagesByTransactionRequest, GetMessagesRequest (+12 more)

### Community 33 - "Statistics gRPC Service"
Cohesion: 0.16
Nodes (21): GetBridgedTokensRequest, GetBridgedTokensResponse, GetChainsStatsRequest, GetChainsStatsResponse, GetCommonStatisticsRequest, GetCommonStatisticsResponse, GetDailyStatisticsRequest, GetDailyStatisticsResponse (+13 more)

### Community 34 - "AMB Indexer Runtime"
Cohesion: 0.14
Nodes (16): AmbChainConfig, AmbIndexer, Arc, AtomicBool, AtomicU64, DashMap, Filter, Message (+8 more)

### Community 35 - "ICTT Payload Decoding"
Cohesion: 0.11
Nodes (21): ictt_completeness(), CreditExpectation, decode_inner(), decode_transferrer_message(), IcttPayload, mainnet_bytes(), PayloadRejection, Result (+13 more)

### Community 36 - "Avalanche Domain Types"
Cohesion: 0.10
Nodes (26): CallFailed, CallSucceeded, AnnotatedEvent, AnnotatedICTTSource, CallOutcome, Message, MessageExecutionOutcome, Address (+18 more)

### Community 37 - "Indexer Lifecycle & Cleanup Guard"
Cohesion: 0.09
Nodes (16): CleanupGuard, Arc, AtomicBool, Drop, JoinHandle, Option, RwLock, CrosschainIndexerState (+8 more)

### Community 38 - "Chain-Bridge Filter Helpers"
Cohesion: 0.13
Nodes (20): build_chain_bridge_filter(), build_chain_bridge_filter_all_indexed_is_none_even_without_opt_in(), build_chain_bridge_filter_default_sets_sorted_pairs(), build_chain_bridge_filter_include_unindexed_true_clears_restriction(), build_chain_bridge_filter_prunes_pairs_to_requested_bridge_ids(), checked_bridge_id(), checked_bridge_id_rejects_above_i32_max(), non_empty() (+12 more)

### Community 39 - "Coding Handoff Standards"
Cohesion: 0.10
Nodes (25): just format (cargo sort + cargo fmt), Workflow: implementation-plan.md, coding-task-X.md Handoff Artifact, implementation-plan-X.md Artifact, Coding Handoff Must Be Self-Sufficient, Block the Coding Handoff on User Confirmation, Workflow: pr-description.md, Explicit None. for API / ENV / Migration Sections (+17 more)

### Community 40 - "Server Entrypoint & E2E"
Cohesion: 0.20
Nodes (23): ConfigSettings, main(), Error, Result, decode_blockchain_id(), forked_provider(), parse_message_id_from_native_id(), DynProvider (+15 more)

### Community 41 - "GitHub Issue Skills & Hooks"
Cohesion: 0.15
Nodes (24): Hook: allow-tmp-dirs.py (PreToolUse Bash), Hook: allow-tmp-writes.py (PreToolUse Write|Edit), Claude Skill: gh-issue-bug, Claude Skill: gh-issue-improvement, Claude Skill: gh-issue-publish, Codex Agent Interface: GitHub Issue Bug, Bug vs Improvement Issue Separation, Codex Skill: gh-issue-bug (+16 more)

### Community 42 - "Exploration Map & Glossary"
Cohesion: 0.09
Nodes (24): Incoming ICTT Reconstruction / ICM Payload Decoding Entrypoints, Bridge, Bridge Contract, Configured Chain, Consolidation, Cross-Chain Message, Cross-Chain Transfer, Destination-Indexed Data (+16 more)

### Community 43 - "Config-to-Model Conversion"
Cohesion: 0.16
Nodes (19): ApiKeyConfig, BridgeConfig, bridges::ActiveModel, build_rpc_url(), ChainConfig, chains::ActiveModel, create_provider_pools_from_chains(), ExplorerConfig (+11 more)

### Community 44 - "Statistics REST Endpoints"
Cohesion: 0.10
Nodes (23): Proto Build Serde Attributes Are Behavior, just check-envs / just generate-envs Requirement on ENV Changes, Exposed REST Endpoints and Swagger URL, api_config_http.yaml — gRPC-to-HTTP Rule Map, GET /api/v1/stats/chains, GET /api/v1/stats/common, GET /api/v1/stats/daily, GET /api/v1/stats/chain/{chain_id}/messages-paths/received (+15 more)

### Community 45 - "Proto TypeScript Package"
Cohesion: 0.09
Nodes (22): ts-proto, bugs, url, description, devDependencies, ts-proto, typescript, homepage (+14 more)

### Community 46 - "Claude Task Workflow Skills"
Cohesion: 0.16
Nodes (22): Claude Code Overrides (project CLAUDE.md), Handoff Preparation, Not Re-Analysis, Claude Skill: implementation-plan, Reviewer-Facing PR Description (not a changelog), Claude Skill: pr-description, Claude Skill: solution-review, Verification Gap Reporting, Claude Skill: task-analysis (+14 more)

### Community 47 - "Runtime Verification Canaries"
Cohesion: 0.12
Nodes (22): Runtime Verification Runbook, bridge_contracts Is a Proxy, Not the Membership Set, Canary vs Diagnostic Classification, IndexedChains::may_observe (in-memory eligibility), Observability Horizon Eligibility Rule, Query C — Hard Invariants Canary, Query D — Deferred Transfers Classified by Reason, Query E — Unindexed-Chain Edges Diagnostic (+14 more)

### Community 48 - "Indexing Target Planning"
Cohesion: 0.18
Nodes (21): amb_contract_configs(), build_amb_chain_configs(), build_avalanche_chain_configs(), ChainPlan, enumerate_indexing_targets(), group_contracts_by_chain(), log_amb_floor_divergence(), plan_bridge() (+13 more)

### Community 49 - "Server Test Helpers"
Cohesion: 0.10
Nodes (13): get_raw(), init_db(), init_interchain_indexer_server(), F, String, TestDbGuard, Url, Value (+5 more)

### Community 50 - "Maintenance Pass & Buffer Eviction"
Cohesion: 0.16
Nodes (12): Add, BridgeCounts, Counts, MaintenancePlan<T>, record_bridge_metrics(), BridgeId, HashMap, I (+4 more)

### Community 51 - "AMB Indexer Failure Tests"
Cohesion: 0.22
Nodes (19): amb_handler_failure_creates_indexer_failure_row_for_the_failing_block(), AmbContractConfig, chain_config(), mock_block(), mock_provider(), mock_receipt(), registry_with_event(), repeated_amb_handler_failure_during_retry_does_not_resolve_existing_hole() (+11 more)

### Community 52 - "Avalanche Range Processor"
Cohesion: 0.20
Nodes (12): AvalancheChainConfig, AvalancheRangeProcessor, chain(), log_filter_covers_every_configured_contract_address_for_a_chain_with_several(), process_batch(), Address, ChainId, DynProvider (+4 more)

### Community 53 - "Failure Ledger Core"
Cohesion: 0.12
Nodes (13): FailureLedger, Arc, HashSet, Result, RwLock, Self, String, Vec (+5 more)

### Community 54 - "Catch-up Progress Computation"
Cohesion: 0.24
Nodes (19): CatchupProgress, CheckpointCursors, cursors(), Option, Self, test_compute_backward_compatible_with_one_directional_formula_when_m_equals_s(), test_compute_blocks_remaining_equals_interval_width(), test_compute_blocks_remaining_is_zero_once_complete() (+11 more)

### Community 55 - "Buffer Item Dirty Tracking"
Cohesion: 0.13
Nodes (14): BufferItem, BufferItem<T>, now_naive_utc(), BlockNumber, BTreeSet, BufferItemVersion, ChainId, HashMap (+6 more)

### Community 56 - "REST Endpoint Surface"
Cohesion: 0.13
Nodes (21): GET /api/v1/interchain/chains, GET /api/v1/interchain/messages/{message_id}, GET /api/v1/interchain/messages, GET /api/v1/interchain/messages:byAddress/{address}, GET /api/v1/interchain/messages:byTx/{tx_hash}, GET /api/v1/interchain/transfers, GET /api/v1/interchain/transfers:byAddress/{address}, GET /api/v1/interchain/transfers:byTx/{tx_hash} (+13 more)

### Community 57 - "Architecture Overview"
Cohesion: 0.12
Nodes (20): upsert_cursors — GREATEST-only cursor maintenance writer (cannot lower a floor), AvalancheIndexer, High-Level Data Flow, LogStream, MessageBuffer (Tiered Storage), Bridge Filtering Entrypoints, Message Buffer, Teleporter / ICM (+12 more)

### Community 58 - "Asset Union-Find & Token Identity"
Cohesion: 0.11
Nodes (20): ChainInfoService, TokenInfoService, Union-Find Asset Merge, merge_assets / ensure_asset_for_transfer — weighted union-find over stats_assets, Gotcha: Stats Asset Mapping Conflicts Merge; Only Same-Chain Collisions Skip, Gotcha: Token Identity In stats_asset_tokens Is The ICTT Contract Address, Not The Wrapped ERC-20, avalanche_icm_blockchain_ids Table, Runtime Metadata Writes (+12 more)

### Community 59 - "Error Handling Conventions"
Cohesion: 0.12
Nodes (20): Error Handling Rules, anyhow::Result for Internal Code, API Error Sanitization, Checked/Saturating Arithmetic and Euclidean Division, Always Add Context When Propagating, Log Errors at the Handling Point, Panic Avoidance in Runtime Paths, thiserror for Public API Error Types (+12 more)

### Community 60 - "Range Driver Chunking Tests"
Cohesion: 0.18
Nodes (15): a_budget_covering_the_whole_queue_attempts_each_position_once(), chunk_range(), chunk_range_clamps_a_zero_batch_size_to_one(), chunk_range_narrower_than_batch_size_yields_one_chunk(), chunk_range_splits_into_batch_size_pieces_with_a_narrower_last_chunk(), interval(), resume_index(), resume_index_re_attempts_the_chunk_a_cursor_lands_inside() (+7 more)

### Community 61 - "Range Driver Retry Ticks"
Cohesion: 0.15
Nodes (13): attributed_ranges(), BatchError, RangeDriver<P>, Error, Filter, From, Poll, Result (+5 more)

### Community 62 - "Observability Horizon ADR"
Cohesion: 0.16
Nodes (19): ADR-004: Stats Observability Horizon; Asset Identity As Union-Find, IndexedChains (Stats Eligibility), Runtime Verification Runbook, Stats Entrypoints, Countable / Deferred (Stats), IndexedChains (glossary), Observability Horizon, Projection (+11 more)

### Community 63 - "Incoming ICTT Reconstruction"
Cohesion: 0.19
Nodes (19): build_reconstructed_transfer(), ClassifiedPayload, classify_payload(), destination_arm(), destination_arm_amount(), DestinationArm, dst_token_address(), Message (+11 more)

### Community 64 - "Avalanche Transfer Building"
Cohesion: 0.18
Nodes (16): build_transfer(), AnnotatedEvent, ChainId, NaiveDateTime, ReceiveCrossChainMessage, Result, Self, SendCrossChainMessage (+8 more)

### Community 65 - "Avalanche Indexer Runtime"
Cohesion: 0.14
Nodes (13): AvalancheIndexer, BatchProcessContext, IndexerCleanupGuard, Arc, AtomicBool, AtomicU64, Drop, Error (+5 more)

### Community 66 - "Failure Retry Settings"
Cohesion: 0.20
Nodes (15): default_backoff_base(), default_backoff_cap(), default_enabled(), default_max_chunks_per_pass(), default_record_retry_attempts(), default_record_retry_initial_backoff(), default_scan_interval(), FailureRetrySettings (+7 more)

### Community 67 - "Transaction Skip & Testing Rules"
Cohesion: 0.12
Nodes (18): DecimalsConflict Domain Marker Type, Expected Skips Inside a Shared Transaction, Maintenance Transaction (messages, transfers, stats, cursor), Detect Conflicts with SELECT, Never a Failing INSERT, Never Assert a Delta on a Process-Wide Metric, STATS_EDGE_DECIMALS_CONFLICT_TOTAL (test-isolation case study), ADR-004 Stats Observability Horizon and Asset Union-Find, Asset-Identity Union-Find Merge (+10 more)

### Community 68 - "Block Interval Arithmetic"
Cohesion: 0.24
Nodes (14): pre_union_with_reason(), BlockRange, difference_produces_expected_pieces(), fold_adjacent(), merge_bounds(), overlaps(), overlaps_or_adjacent(), pre_union() (+6 more)

### Community 69 - "Failed Interval Backoff Policy"
Cohesion: 0.22
Nodes (16): FailedInterval, NaiveDateTime, Option, String, base_ts(), capped_backoff_does_not_overflow_at_extreme_attempts(), capped_backoff_secs(), capped_backoff_widens_strictly_until_the_cap() (+8 more)

### Community 70 - "Maintenance Plan & Hot Eviction"
Cohesion: 0.22
Nodes (11): classify_item(), ConsolidationOutcome, HotEvictionReason, MaintenancePlan, MessageBuffer<T>, BufferItemVersion, Option, Result (+3 more)

### Community 71 - "Stats Proto Mapping"
Cohesion: 0.14
Nodes (11): bridged_row_to_proto(), i64_to_u64_nonneg(), map_stats_error(), parse_optional_utc_date(), parse_optional_utc_date_rejects_malformed(), Error, NaiveDate, Option (+3 more)

### Community 72 - "Cursor Review & Issue Skills"
Cohesion: 0.16
Nodes (16): Codex Solution Review Agent Interface, Solution Review Guardrails, Codex Solution Review Skill, Task Folder Artifacts (tmp/tasks/<task-name>/), Cursor GitHub Bug Issue Skill, High-Level Suggested Fix Rule, tmp/gh-issues/YYMMDD-<name>.md Draft Convention, Conceptual Proposed-Changes Rule (+8 more)

### Community 73 - "Contract Version Windows ADR"
Cohesion: 0.15
Nodes (16): Explicit Human Confirmation Gate, Cursor Research Scope Skill, ADR-001: Message Buffer Tiered Storage, ADR-005: Failed-Range Ledger, Independent of Checkpoints, ADR-006: Contract Versioning Resolved By Block, bridge_contracts UNIQUE(bridge_id, chain_id, address, version), Contract Version Windows by started_at_block, adr/ Architectural Decision Records (+8 more)

### Community 74 - "Docker Compose Stack"
Cohesion: 0.19
Nodes (16): docker-compose.yml Full Stack, backend service (Blockscout API), caddy service, db service (postgres:17), db-init service, frontend service, interchain-indexer service, redis-db service (+8 more)

### Community 75 - "Canonical Write Path & Consolidate Contract"
Cohesion: 0.17
Nodes (15): Consolidate Trait, Maintenance Task, Operational Risks, Pre-Buffer Storage Gate, Canonical Indexing Write Path, indexer_checkpoints Semantics, Cursor Gap Bridging, Database Outage Followed By Recovery Leap (+7 more)

### Community 76 - "Avalanche Data API Client"
Cohesion: 0.28
Nodes (10): ClientWithMiddleware, AvalancheDataApiClient, AvalancheDataApiClientSettings, AvalancheDataApiNetwork, GetBlockchainByIdResponse, Option, Result, Self (+2 more)

### Community 77 - "AMB Dispatch Mock Service"
Cohesion: 0.14
Nodes (13): Id, AmbDispatchMockService, json_response(), Block, Error, Future, Poll, RequestPacket (+5 more)

### Community 78 - "Crosschain Messages Entity"
Cohesion: 0.15
Nodes (9): Model, Relation, DateTime, Option, Vec, MessageStatus, Model, Relation (+1 more)

### Community 79 - "AMB Event Ordering Gotchas"
Cohesion: 0.20
Nodes (14): A failed floor write stays a warn (today), Checkpoint, Gotcha: AMB Queued Events Must Preserve Their Emitting Chain, Gotcha: A Checkpoint Certifies Scanning, Not Correctness, Gotcha: Checkpoint Stall When All Events Are Perpetually Filtered, FailureLedger — in-memory open-hole cache over indexer_failures, Gotcha: The Failure Ledger's Healthy Path Is DB-Free Only Because One Process Owns A Bridge, indexer_failures — the failed-range ledger (+6 more)

### Community 80 - "Blockchain ID Resolution & DB Gotchas"
Cohesion: 0.15
Nodes (14): Database Schema, Unindexed-Chain Read Filter, batched_upsert / run_in_batches, Database Subsystem: Schema and DB Interaction Layer, Hybrid Database Layer, InterchainDatabase Facade, Table Families, Unindexed-Chain Read Filter (+6 more)

### Community 81 - "Two-Channel Config Model"
Cohesion: 0.15
Nodes (14): API Serving Entrypoints, Avalanche Indexing Entrypoints, Common Indexer Architecture Entrypoints, Config Loading Entrypoints, Database Schema and Migrations Entrypoints, Exploration Map, Whole-System Entrypoints, Configuration Model (+6 more)

### Community 82 - "ADR Index & AMB Token Reconstruction"
Cohesion: 0.17
Nodes (13): ADR-001: Message Buffer Tiered Storage, ADR-002: Primary Chain Filtering for Unknown Chains, ADR-003: AMB Transfers Reconstructed From Events; Nullable Transfer Sides, ADR-006: Contract Versioning Resolved By Block, At Decode Time, Architectural Decision Records Index, ADR Template, AMB / Omnibridge Token Transfer Reconstruction, build_transfer / build_destination_only_transfer (+5 more)

### Community 83 - "Bridges Entity"
Cohesion: 0.17
Nodes (8): Entity, Model, Relation, DateTime, Option, Related, RelationDef, String

### Community 84 - "Receipt Fetching"
Cohesion: 0.18
Nodes (12): fetch_receipts_for_transactions(), FetchedTransactionReceipt, Address, B256, Block, DynProvider, Ethereum, HashMap (+4 more)

### Community 85 - "Codex Task Skills"
Cohesion: 0.18
Nodes (12): Codex Task Analysis Agent Interface, Human Evaluation-Criteria Alignment, solution_N.md Option Files, Codex Task Analysis Skill, Codex Task To Code Agent Interface, coding-task-X.md Handoff, No-Invented-Scope Rule, Codex Task To Code Skill (+4 more)

### Community 86 - "Proto Build Script"
Cohesion: 0.32
Nodes (11): AsRef, compile(), dedupe_actix_duplicate_chain_info_internal(), main(), Box, Error, Path, Result (+3 more)

### Community 87 - "Bridge Proto Mapping"
Cohesion: 0.32
Nodes (11): Bridge, BridgeModel, bridge_model_to_proto(), model(), Result, test_bridge_model_to_proto_multi_chain_is_sorted(), test_bridge_model_to_proto_no_configured_chains_is_empty(), test_bridge_model_to_proto_ordering_is_deterministic_across_insertion_orders() (+3 more)

### Community 88 - "Mock Logs RPC Service"
Cohesion: 0.21
Nodes (10): build_logs_response(), MockLogsAction, MockLogsService, Future, Log, Mutex, RequestPacket, ResponsePacket (+2 more)

### Community 89 - "Bridge Contract Config"
Cohesion: 0.18
Nodes (11): BridgeContractConfig, ActiveModel, test_bridge_contract_config_to_active_model(), contract(), contract_with_address(), parse_contract_abi(), parse_contract_address(), Address (+3 more)

### Community 90 - "Research Scope Skill"
Cohesion: 0.22
Nodes (11): Claude Skill: research-scope, Plan Review Gate Before Coding Task, Codex Agent Interface: Research Scope, Explicit Human Confirmation Before Persisting Research, Codex Skill: research-scope, Memory Bank: exploration-map.md, Memory Bank: research/README.md, Scope Research Workflow (+3 more)

### Community 91 - "Health Check Service"
Cohesion: 0.18
Nodes (7): Health, HealthCheckRequest, HealthCheckResponse, HealthService, Request, Response, Result

### Community 92 - "Stats Assets Entity"
Cohesion: 0.20
Nodes (8): Entity, Model, Relation, DateTime, Option, Related, RelationDef, String

### Community 93 - "Bridged Tokens Pagination Cursor"
Cohesion: 0.36
Nodes (9): BridgedTokenAggDbRow, build_pagination_from_bridged_tokens(), count_column(), cursor_where_next(), cursor_where_prev(), String, Value, BridgedTokensPaginationLogic (+1 more)

### Community 94 - "Indexer Failure Merge Semantics"
Cohesion: 0.35
Nodes (9): indexer_failures_rows_for(), open_indexer_failures_is_a_pure_read_with_no_side_effects(), record_indexer_failures_disjointness_holds_after_mixed_merges(), record_indexer_failures_does_not_merge_across_a_real_gap(), record_indexer_failures_growth_bound_for_consecutive_realtime_failures(), record_indexer_failures_merges_overlapping_and_adjacent_ranges_into_one_row(), resolve_indexer_failures_resets_attempts_but_keeps_parents_updated_at_on_split(), resolve_indexer_failures_returns_true_only_when_the_set_becomes_empty() (+1 more)

### Community 95 - "Migration Runner"
Cohesion: 0.18
Nodes (9): from_sql(), Migrator, Box, DbErr, MigrationTrait, Result, SchemaManager, Vec (+1 more)

### Community 96 - "Primary Chain Filtering ADR"
Cohesion: 0.24
Nodes (10): ADR-002: Per-Bridge Chain Filtering, Fail-Fast Startup Validation of home_chain_id, Filter Order: Chain-Config Then Home-Chain, home_chain_id, process_unknown_chains, ADR-004: Observability Horizon and Asset Union-Find, Config Change Never Reinterprets Indexed History, include_unindexed_chains Read Filter (+2 more)

### Community 97 - "Event-Derived AMB Transfers ADR"
Cohesion: 0.20
Nodes (10): ADR-003: AMB Transfers From Events; Nullable Sides, build_transfer (amb/consolidation.rs), Calldata Token Directional Ambiguity, Event-Derived AMB Transfer Reconstruction, Nullable Transfer Sides (Never Mirrored), Removal of the payload_processor Calldata Subsystem, TokensBridged (Destination Side), TokensBridgingInitiated (Source Side) (+2 more)

### Community 98 - "Failed-Range Ledger Mechanism"
Cohesion: 0.22
Nodes (10): FailureLedger, indexer_failures Table, LogBatch (from_block, to_block, direction, logs), LogStream, One Scanner Per (bridge, chain), RangeDriver, RangeProcessor Trait, record Merges on Overlap or Adjacency (+2 more)

### Community 99 - "Blockchain ID Resolver Cache"
Cohesion: 0.29
Nodes (7): Cache, CacheKey, CacheValue, BlockchainIdResolver, resolves_native_id_to_chain_id_8021_and_persists_mapping(), Result, Self

### Community 100 - "AMB Indexer Settings"
Cohesion: 0.36
Nodes (8): AmbIndexerSettings, default_batch_size(), default_clock_skew_tolerance(), default_pull_interval(), default_receipt_concurrency(), Default, Duration, Self

### Community 101 - "Datetime Byte Encoding Utils"
Cohesion: 0.49
Nodes (8): bytes_to_naive_datetime(), naive_datetime_to_bytes(), naive_datetime_to_nanos(), nanos_to_naive_datetime(), NaiveDateTime, Result, test_naive_datetime_to_bytes_round_trip(), u64_from_hex_prefixed()

### Community 102 - "Failed-Range Ledger & Retry Starvation"
Cohesion: 0.25
Nodes (9): ADR-005: Failed-Range Ledger, Independent of Checkpoints, RangeDriver::run / run_retry_tick (indexer/range_driver.rs), Gotcha: The Retry Pass Starves The Forward Streams, And That Looks Like RPC Failure, AMB In-Memory Correlation Maps, FailureLedger / indexer_failures, Indexing Gaps, Retries, and Checkpoint Safety, LogBatch (Named Range), RangeDriver Retry Pass (+1 more)

### Community 103 - "Crosschain Transfers Entity"
Cohesion: 0.25
Nodes (8): Model, Relation, BigDecimal, DateTime, Decimal, Option, Vec, TransferType

### Community 104 - "Message Lifecycle & Async Rules"
Cohesion: 0.29
Nodes (8): CrosschainIndexer Trait, IndexerCleanupGuard Drop Guard, Async Patterns Rules, Graceful Shutdown and Cleanup Guards, Shared State (Arc RwLock), Start/Stop Invariants, Task Spawning and JoinHandle Rule, Async Trait Methods Rule

### Community 105 - "AMB Ordering & Terminal-Row Gotchas"
Cohesion: 0.25
Nodes (8): Gotcha: AMB Collision Replacement Must Delete Before Insert, Gotcha: AMB Source and Destination Events Can Arrive Out of Order, Gotcha: AMB Header Sender Is Not The Source Transaction Initiator, Gotcha: AMB Transfer Sides Are Nullable and Never Mirrored, crosschain_messages_on_conflict — keep_existing_if_terminal vs prefer_incoming (message_buffer/persistence.rs), Gotcha: recipient_address On A Terminal crosschain_messages Row Can Never Be Patched Later, Gotcha: Recoverable Message Fields Are Not A "Never Mirror" Case, SourceData::from_receive / from_execution (indexer/avalanche/consolidation.rs)

### Community 106 - "Testing Conventions"
Cohesion: 0.29
Nodes (8): Testing Rules, Feature-Flagged E2E Tests (avalanche-e2e), just test-with-db vs just test, fill_mock_interchain_database Fixtures, Test Attributes (tokio::test, ignore, rstest), Test Naming Format, TestDbGuard Isolated Database Tests, Prefer Repo-Native Verification Commands over cargo test

### Community 107 - "Bridge Contracts Entity"
Cohesion: 0.25
Nodes (7): Model, Relation, DateTime, Json, Option, String, Vec

### Community 108 - "Stats Asset Persistence"
Cohesion: 0.36
Nodes (6): stats_asset_delete_cascades(), stats_asset_insert_and_get(), stats_link_token_without_tokens_row(), stats_migration_applies(), stats_reject_same_token_two_assets(), stats_reject_two_tokens_same_chain_one_asset()

### Community 109 - "Avalanche Indexer Settings"
Cohesion: 0.39
Nodes (6): AvalancheIndexerSettings, default_batch_size(), default_pull_interval(), Default, Duration, Self

### Community 110 - "Message Buffer Settings"
Cohesion: 0.43
Nodes (6): default_hot_ttl(), default_maintenance_interval(), MessageBufferSettings, Default, Duration, Self

### Community 111 - "Initial Schema Migration"
Cohesion: 0.36
Nodes (5): Migration, DbErr, MigrationTrait, Result, SchemaManager

### Community 112 - "Stats Tables Migration"
Cohesion: 0.36
Nodes (5): Migration, DbErr, MigrationTrait, Result, SchemaManager

### Community 113 - "AMB Indexer Migration"
Cohesion: 0.36
Nodes (5): Migration, DbErr, MigrationTrait, Result, SchemaManager

### Community 114 - "Read Filters Migration"
Cohesion: 0.36
Nodes (5): Migration, DbErr, MigrationTrait, Result, SchemaManager

### Community 115 - "Message Buffer Tiering ADR"
Cohesion: 0.33
Nodes (7): Cold Tier (pending_messages table), Entry Versioning for Cursor Tracking, Hot Tier (In-Memory DashMap), Tiered Message Buffer Storage, TTL-Based Eviction and Cache-Miss Restoration, catchup_min_cursor, Checkpoint/Ledger Independence

### Community 116 - "Maintenance Consolidation Rules"
Cohesion: 0.29
Nodes (7): Maintenance Task and Consolidation Pass, Asset Identity as Union-Find, Eager Weighted Asset Merge, Fragmented Assets Defect, Conflicts Are Refusals, Never Transaction Errors, stats_processed Counting Marker, Drain Must Not Clear Its Queue Before Writes Succeed

### Community 117 - "Contract Version Resolution Metrics"
Cohesion: 0.29
Nodes (7): interchain_indexer_oldest_open_hole_age_seconds, Cyclic Retry-Pass Sweep with Shared Chunk Budget, AbiRegistry, AmbChainConfig (amb_proxies, mediators lists), resolve_log(chain_id, address, topic, block_number), topic0 Cannot Substitute for Block Resolution, interchain_indexer_amb_logs_dropped_wrong_version_total

### Community 118 - "Consolidate Trait Bounds"
Cohesion: 0.29
Nodes (7): Deserialize, TransferDummyMessage, Consolidate, Clone, Send, Sync, Serialize

### Community 119 - "AMB Message Anomalies Entity"
Cohesion: 0.29
Nodes (6): Model, Relation, DateTime, Option, String, Vec

### Community 120 - "Tokens Entity"
Cohesion: 0.29
Nodes (6): Model, Relation, DateTime, Option, String, Vec

### Community 121 - "Blockchain ID Resolver CLI"
Cohesion: 0.48
Nodes (5): main(), parse_args(), parse_blockchain_id(), Result, String

### Community 122 - "Total Counters & Joined Transfers"
Cohesion: 0.33
Nodes (6): InterchainTotalCounters, JoinedTransfer, BigDecimal, Decimal, NaiveDateTime, transfer_ids()

### Community 123 - "Stats Chains Recomputation"
Cohesion: 0.33
Nodes (5): recompute_stats_chains_distinct_users_and_merges_message_transfer_sides(), select_stats_chains_message_user_counts(), select_stats_chains_transfer_user_counts(), stats_chains_upsert(), SelectStatement

### Community 124 - "Transaction Log Grouping"
Cohesion: 0.38
Nodes (6): group_logs_by_transaction(), B256, HashMap, Log, Vec, test_group_logs_by_transaction_preserves_input_order()

### Community 125 - "Status REST Endpoints"
Cohesion: 0.33
Nodes (7): GET /api/v1/status/indexers, GET /api/v1/status/indexing, GET /api/v1/status/indexers/{indexer_name}, StatusService, v1FullStatus, v1GetIndexingProgressResponse, v1IndexerStatus

### Community 126 - "Worker Roles & Design Principles"
Cohesion: 0.40
Nodes (6): Project-Specific Naming Conventions, BridgeContractIndexer Worker, Common Design Principles, CrosschainIndexer Worker, MessageCollector Worker, TokenFetcher Worker

### Community 127 - "AMB Confirmations Entity"
Cohesion: 0.33
Nodes (5): Model, Relation, DateTime, Option, Vec

### Community 128 - "Avalanche ICM Blockchain IDs Entity"
Cohesion: 0.33
Nodes (5): Model, Relation, DateTime, Option, Vec

### Community 129 - "Bridge Txs Entity"
Cohesion: 0.33
Nodes (5): Model, Relation, DateTime, Option, Vec

### Community 130 - "Indexer Failures Entity"
Cohesion: 0.33
Nodes (5): Model, Relation, DateTime, Option, String

### Community 131 - "Pending Messages Entity"
Cohesion: 0.33
Nodes (5): Model, Relation, DateTime, Json, Option

### Community 132 - "Token Info Stats Enrichment"
Cohesion: 0.53
Nodes (4): stats_enrichment_propagate_does_not_overwrite_conflicting_decimals(), stats_enrichment_propagate_preserves_non_empty_asset_metadata(), stats_enrichment_propagate_skips_unrelated_destination_edge(), stats_enrichment_propagate_upsert_fills_asset_and_edge_decimals()

### Community 133 - "API Error Type"
Cohesion: 0.40
Nodes (4): ApiError, Error, Self, String

### Community 135 - "Stats Messages Days Entity"
Cohesion: 0.40
Nodes (4): Date, Model, Relation, DateTime

### Community 136 - "Stats Asset Tokens Entity"
Cohesion: 0.40
Nodes (4): Model, Relation, DateTime, Vec

### Community 137 - "Bulk Upsert & SeaORM Gotchas"
Cohesion: 0.50
Nodes (4): batched_upsert() / run_in_batches() (bulk.rs), Gotcha: PostgreSQL Bind Parameter Limit (65535 per statement), Gotcha: SeaORM Entity Regeneration Overwrites Manual Changes (codegen/ vs manual/), Gotcha: SeaORM insert_many Cannot Mix Set and NotSet for the Same Column

### Community 138 - "Token Info Consistency Gotchas"
Cohesion: 0.50
Nodes (4): Gotcha: Stats Edge Amount Side Must Follow Indexed Source Presence, Gotcha: Token Info Caches Errors (TokenInfoService negative TTL), Gotcha: Token Info Is Eventually Consistent and Reads Can Write Back, TokenInfoService (token_info/service.rs)

### Community 139 - "Chain Info Proto Mapping"
Cohesion: 0.67
Nodes (3): ChainModel, chain_model_to_proto(), ChainInfo

### Community 140 - "Tmp Mkdir Permission Hook"
Cohesion: 0.67
Nodes (3): is_tmp_mkdir_command(), main(), Check if the Bash command is creating directories within the tmp/ directory.…

### Community 141 - "Tmp Write Permission Hook"
Cohesion: 0.67
Nodes (3): is_tmp_path(), main(), Check if the file path is within the tmp/ directory. Handles various path…

### Community 142 - "Entity Relation Defs (a)"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 143 - "Entity Relation Defs (b)"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 144 - "Entity Relation Defs (c)"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 145 - "Entity Relation Defs (d)"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 146 - "Entity Relation Defs (e)"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 147 - "Entity Relation Defs (f)"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 148 - "Entity Relation Defs (g)"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 149 - "Entity Relation Defs (h)"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 150 - "Entity Relation Defs (i)"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 151 - "Entity Relation Defs (j)"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 152 - "Entity Relation Defs (k)"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 153 - "Entity Relation Defs (l)"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 154 - "Stats Messages Entity"
Cohesion: 0.50
Nodes (3): Model, Relation, DateTime

### Community 155 - "Entity Relation Defs (m)"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 156 - "Entity Relation Defs (n)"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 157 - "Entity Relation Defs (o)"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 159 - "ActiveValue Helper"
Cohesion: 0.67
Nodes (3): ActiveValue, T, set_value()

## Knowledge Gaps
- **176 isolated node(s):** `gh-issue-publish.sh script`, `Relation`, `ActiveModel`, `Relation`, `ActiveModel` (+171 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **27 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `InterchainDatabase` connect `Interchain Database Facade` to `Stats Read Service`, `Counters & Checkpoint Queries`, `Token Info Stats Enrichment`, `Batched Upsert Persistence`, `Database Test Fixtures`, `Indexer Status & Progress Entities`, `Indexer Lifecycle & Cleanup Guard`, `Log Stream & Batch Fetching`, `Chains Entity & Chain Info Service`, `Message Paths Stats Queries`, `Interchain gRPC Service`, `Indexer Target Enumeration`, `Tiered Message Buffer`, `Interchain Proto Request Types`, `AMB Indexer Runtime`, `Failure Ledger Core`, `Avalanche Indexer Runtime`, `Indexer Failure Merge Semantics`, `Blockchain ID Resolver Cache`, `Stats Asset Persistence`, `Total Counters & Joined Transfers`, `Stats Chains Recomputation`?**
  _High betweenness centrality (0.218) - this node is a cross-community bridge._
- **Why does `Key` connect `AMB Message Consolidation` to `Avalanche Transfer Building`, `AMB Indexer Runtime`, `Batched Upsert Persistence`, `Maintenance Plan & Hot Eviction`, `Maintenance Pass & Buffer Eviction`, `Failure Ledger Core`, `Avalanche ICM Consolidation`, `Tiered Message Buffer`, `Avalanche Log Handlers`, `Incoming ICTT Reconstruction`?**
  _High betweenness centrality (0.067) - this node is a cross-community bridge._
- **Why does `IndexedChains` connect `IndexedChains Eligibility Set` to `Interchain Proto Request Types`, `Statistics gRPC Service`, `Stats Read Service`, `Counters & Checkpoint Queries`, `Interchain Database Facade`, `Database Test Fixtures`, `Chain-Bridge Filter Helpers`, `Stats Asset Edge Projection`, `Bridged Tokens Query`, `Message Paths Stats Queries`, `Interchain gRPC Service`, `Bridge Proto Mapping`?**
  _High betweenness centrality (0.064) - this node is a cross-community bridge._
- **Are the 229 inferred relationships involving `init_db()` (e.g. with `bridged_tokens_aggregation_input_output_total()` and `bridged_tokens_default_excludes_edge_unindexed_for_its_bridge()`) actually correct?**
  _`init_db()` has 229 INFERRED edges - model-reasoned connections that need verification._
- **Are the 79 inferred relationships involving `fill_mock_interchain_database()` (e.g. with `counters_cover_all_filters()` and `get_crosschain_message_native_collision_is_ambiguous_until_qualified()`) actually correct?**
  _`fill_mock_interchain_database()` has 79 INFERRED edges - model-reasoned connections that need verification._
- **What connects `gh-issue-publish.sh script`, `Relation`, `ActiveModel` to the rest of the system?**
  _176 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `AMB Message Consolidation` be split into smaller, more focused modules?**
  _Cohesion score 0.05428796223446106 - nodes in this community are weakly interconnected._