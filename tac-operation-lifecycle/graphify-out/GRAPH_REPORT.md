# Graph Report - tac-operation-lifecycle  (2026-08-09)

## Corpus Check
- 81 files · ~30,569 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 812 nodes · 1397 edges · 71 communities (52 shown, 19 thin omitted)
- Extraction: 98% EXTRACTED · 2% INFERRED · 0% AMBIGUOUS · INFERRED: 25 edges (avg confidence: 0.83)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `145743ed`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- TacDatabase
- Indexer
- OperationsService
- profiling.rs
- Client
- StatisticService
- Settings
- m20260304_204118_mark_insufficient_fee_operations.rs
- tests/mod.rs
- package.json
- Solution Review Skill
- Migration
- Migration
- compile
- prelude.rs
- m20220101_000001_create_table.rs
- Gotchas & Edge Cases
- Model
- .migrations
- Model
- Stage Profiler v2 Lifecycle Model
- interval.rs
- Model
- transaction.rs
- Entity
- Entity
- Entity
- Entity
- Entity
- watermark.rs
- main
- Research Scope Skill
- Implementation Plan Skill
- PR Description Skill
- Research Scope Skill
- Implementation Plan Skill
- PR Description Skill
- Research Scope Skill
- ActiveModel
- ActiveModel
- ActiveModel
- ActiveModel
- ActiveModel
- ActiveModel
- ActiveModel
- Sync Architecture
- API Surface
- .memory-bank/README.md
- Operation Lifecycle: Status Machine & Terminal-State Detection
- Implementation Plan Workflow
- Scope Research Workflow
- Task Analysis Workflow
- Interpretation of specific states
- PR Description Workflow
- Solution Review Workflow
- Q: Где формируются V2OperationBriefDetails и V2OperationDetails и как на уровне API спроецировать status/type по profiling_version?
- Q: Переименовать Pending в UNKNOWN; сделать type и status enum и rollback обязательным bool в V2OperationBriefDetails/V2OperationDetails.
- Q: Допустимые варианты для V2OperationType: UNKNOWN, TON_TAC_TON, TAC_TON, TON_TAC. Других вариантов для v2 быть не может по условиям таска. Другие варианты возможны только в v1
- Task To Code Workflow
- README.md
- Project Brief
- stage_type.rs
- AGENTS.md
- CLAUDE.md
- solution-review/SKILL.md
- task-analysis/SKILL.md
- task-to-code/SKILL.md

## God Nodes (most connected - your core abstractions)
1. `TacDatabase` - 58 edges
2. `Indexer` - 29 edges
3. `Client` - 28 edges
4. `OperationsService` - 20 edges
5. `Gotchas & Edge Cases` - 18 edges
6. `Settings` - 17 edges
7. `V1OperationData` - 13 edges
8. `V2OperationData` - 13 edges
9. `SourceOperationData` - 13 edges
10. `ProfilingError` - 12 edges

## Surprising Connections (you probably didn't know these)
- `Solution Review Skill` --semantically_similar_to--> `Solution Review Skill`  [INFERRED] [semantically similar]
  .claude/skills/solution-review/SKILL.md → .codex/skills/solution-review/SKILL.md
- `Task To Code Skill` --semantically_similar_to--> `Task To Code Skill`  [INFERRED] [semantically similar]
  .claude/skills/task-to-code/SKILL.md → .codex/skills/task-to-code/SKILL.md
- `Task Analysis Skill` --semantically_similar_to--> `Task Analysis Skill`  [INFERRED] [semantically similar]
  .claude/skills/task-analysis/SKILL.md → .codex/skills/task-analysis/SKILL.md
- `run()` --references--> `Client`  [EXTRACTED]
  tac-operation-lifecycle-server/src/server.rs → tac-operation-lifecycle-logic/src/client/mod.rs
- `prepare_intervals_resp()` --references--> `IntervalDbStatistic`  [EXTRACTED]
  tac-operation-lifecycle-server/src/services/statistic.rs → tac-operation-lifecycle-logic/src/database.rs

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **Implementation Plan Skill Wrappers** — _claude_skills_implementation_plan_skill_implementation_plan, _codex_skills_implementation_plan_skill_implementation_plan, _cursor_skills_implementation_plan_skill_implementation_plan [INFERRED 0.95]
- **PR Description Skill Wrappers** — _claude_skills_pr_description_skill_pr_description, _codex_skills_pr_description_skill_pr_description, _cursor_skills_pr_description_skill_pr_description [INFERRED 0.95]
- **Research Scope Skill Wrappers** — _claude_skills_research_scope_skill_research_scope, _codex_skills_research_scope_skill_research_scope, _cursor_skills_research_scope_skill_research_scope [INFERRED 0.95]

## Communities (71 total, 19 thin omitted)

### Community 0 - "TacDatabase"
Cohesion: 0.07
Nodes (40): ApiOperations, DatabaseConnection, DatabaseTransaction, Entity, Select, backfill_counters_split_claimable_from_awaiting_retry(), EntityStatus, IntervalDbStatistic (+32 more)

### Community 1 - "Indexer"
Cohesion: 0.06
Nodes (33): BoxStream, JoinHandle, PollNext, BlockchainType, Indexer, IndexerJob, Job, LegacyOperationType (+25 more)

### Community 2 - "OperationsService"
Cohesion: 0.07
Nodes (46): GetOperationByTxHashRequest, GetOperationDetailsRequest, GetOperationsRequest, OperationBriefDetails, OperationDetails, OperationsFullResponse, OperationsResponse, OperationWithStages (+38 more)

### Community 3 - "profiling.rs"
Cohesion: 0.07
Nodes (56): BlockchainType, D, Address, BlockchainType, deserialize_fee_info(), deserialize_note_to_string(), deserialize_valid_executors(), FeeValue (+48 more)

### Community 4 - "Client"
Cohesion: 0.09
Nodes (29): HttpClient, Instant, Limiter, Mutex, StatusCode, T, ApiResponse, Client (+21 more)

### Community 5 - "StatisticService"
Cohesion: 0.06
Nodes (34): GetFullStatisticRequest, GetFullStatisticResponse, GetIntervalStatisticsRequest, GetIntervalStatisticsResponse, GetOperationStatisticsRequest, GetOperationStatisticsResponse, Health, HealthCheckRequest (+26 more)

### Community 6 - "Settings"
Cohesion: 0.06
Nodes (45): ConfigSettings, DatabaseSettings, Deserialize, JaegerSettings, MetricsSettings, ServerSettings, Operation, OperationIdsApiResponse (+37 more)

### Community 7 - "m20260304_204118_mark_insufficient_fee_operations.rs"
Cohesion: 0.10
Nodes (21): fetch_op_type(), Migration, migration_marks_and_reverts_insufficient_fee_operations(), ConnectionTrait, DbErr, MigrationTrait, Option, Result (+13 more)

### Community 8 - "tests/mod.rs"
Cohesion: 0.27
Nodes (11): F, init_db(), init_tac_operation_lifecycle_server(), NaiveDateTime, String, test_job_stream(), test_operation_lifecycle_indexing(), test_save_intervals() (+3 more)

### Community 9 - "package.json"
Cohesion: 0.09
Nodes (22): ts-proto, bugs, url, description, devDependencies, ts-proto, typescript, homepage (+14 more)

### Community 10 - "Solution Review Skill"
Cohesion: 0.10
Nodes (21): Solution Review Skill, Solution Review Workflow, Task Analysis Skill, Task Analysis Workflow, Task To Code Skill, Task To Code Workflow, Implementation Plan Agent Interface, Implementation Plan Skill (+13 more)

### Community 11 - "Migration"
Cohesion: 0.43
Nodes (5): Migration, DbErr, MigrationTrait, Result, SchemaManager

### Community 12 - "Migration"
Cohesion: 0.43
Nodes (5): Migration, DbErr, MigrationTrait, Result, SchemaManager

### Community 13 - "compile"
Cohesion: 0.36
Nodes (8): AsRef, Path, ServiceGenerator, compile(), main(), Box, Error, Result

### Community 14 - "prelude.rs"
Cohesion: 0.25
Nodes (3): StatusEnum, Operation, OperationMetaInfo

### Community 15 - "m20220101_000001_create_table.rs"
Cohesion: 0.22
Nodes (8): Interval, Operation, OperationStage, StageType, StatusEnum, StatusVariants, Transaction, WaterMark

### Community 16 - "Gotchas & Edge Cases"
Cohesion: 0.11
Nodes (18): 10. Down migration requires the new binary to be stopped, 11. README/env-docs defaults drift from code, 12. Realtime thread starts from watermark, not from `Indexer::realtime_boundary`, 13. Realtime boundary only advances on non-empty responses, 14. Stage rewrite is destructive, 15. Raw-SQL claim queries interpolate strings, 16. `error_reason` in the DB and in the API can legitimately differ, 1. `op_type` has a version-dependent meaning — never parse it alone (+10 more)

### Community 17 - "Model"
Cohesion: 0.29
Nodes (6): Decimal, Model, Relation, Option, String, Vec

### Community 18 - ".migrations"
Cohesion: 0.29
Nodes (5): MigratorTrait, Migrator, Box, MigrationTrait, Vec

### Community 19 - "Model"
Cohesion: 0.33
Nodes (6): Model, Relation, DateTime, Option, StatusEnum, String

### Community 20 - "Stage Profiler v2 Lifecycle Model"
Cohesion: 0.14
Nodes (13): Adoption Note, Affected Boundaries, Codebase Behavior Before Adoption (historical), Confirmed Interpretation Rules, Contract Facts, Observed Examples, Open Question, Previous Stage Profiler Model (+5 more)

### Community 21 - "interval.rs"
Cohesion: 0.40
Nodes (5): Model, Relation, DateTime, Option, StatusEnum

### Community 22 - "Model"
Cohesion: 0.33
Nodes (5): Model, Relation, DateTime, Option, String

### Community 23 - "transaction.rs"
Cohesion: 0.40
Nodes (4): Model, Relation, DateTime, String

### Community 24 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 25 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 26 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 27 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 28 - "Entity"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 29 - "watermark.rs"
Cohesion: 0.50
Nodes (3): Model, Relation, DateTime

### Community 30 - "main"
Cohesion: 0.50
Nodes (3): main(), Error, Result

### Community 31 - "Research Scope Skill"
Cohesion: 0.67
Nodes (3): Research Scope Agent Interface, Research Scope Skill, Research Scope Workflow

### Community 49 - "Sync Architecture"
Cohesion: 0.22
Nodes (9): Failure handling & retry, Job streams (all infinite async streams polling the DB), Realtime thread (`create_realtime_thread`), Stage Profiler source selection & circuit breaker, Startup sequence (`Indexer::start`), Stream priority (`select_with_strategy`, left-biased), Sync Architecture, Timeline dissection: intervals + watermark (+1 more)

### Community 50 - "API Surface"
Cohesion: 0.29
Nodes (7): API Surface, Ordering quirk, Served API (proto v1, `tac-operation-lifecycle.proto`), Served API (proto v2, `proto/v2/tac-operation-lifecycle.proto`), Type mapping (server/src/services/operations.rs), Upstream (consumed): TAC data API (`client/mod.rs`), v2 `status` is a product projection — accepted design, do not "fix"

### Community 52 - "Operation Lifecycle: Status Machine & Terminal-State Detection"
Cohesion: 0.29
Nodes (7): Claim predicates (database.rs), Forever-pending cap — a local stop, not a finality rewrite, Full status flow, Operation Lifecycle: Status Machine & Terminal-State Detection, Public projections, The terminal-state decision (`Indexer::operation_work_status`), Three orthogonal dimensions

### Community 53 - "Implementation Plan Workflow"
Cohesion: 0.29
Nodes (6): Implementation Plan Workflow, Preconditions, Process, Starting Context, Templates, Validation Policy

### Community 54 - "Scope Research Workflow"
Cohesion: 0.29
Nodes (6): Process, Quality Bar, Scope Research Workflow, Starting Context, Stop Conditions, Use for

### Community 55 - "Task Analysis Workflow"
Cohesion: 0.29
Nodes (6): Inputs and Artifacts, Process, Starting Context, Task Analysis Workflow, Templates, Use for

### Community 56 - "Interpretation of specific states"
Cohesion: 0.33
Nodes (6): `error_reason`, "Failed", "Insufficient fee", Interpretation of specific states, "Pending", "Rollbacked"

### Community 57 - "PR Description Workflow"
Cohesion: 0.33
Nodes (5): Inputs and Output, PR Description Workflow, Process, Rules, Template

### Community 58 - "Solution Review Workflow"
Cohesion: 0.33
Nodes (5): Inputs and Output, Process, Solution Review Workflow, Starting Context, Template

### Community 59 - "Q: Где формируются V2OperationBriefDetails и V2OperationDetails и как на уровне API спроецировать status/type по profiling_version?"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Где формируются V2OperationBriefDetails и V2OperationDetails и как на уровне API спроецировать status/type по profiling_version?, Source Nodes

### Community 60 - "Q: Переименовать Pending в UNKNOWN; сделать type и status enum и rollback обязательным bool в V2OperationBriefDetails/V2OperationDetails."
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Переименовать Pending в UNKNOWN; сделать type и status enum и rollback обязательным bool в V2OperationBriefDetails/V2OperationDetails., Source Nodes

### Community 61 - "Q: Допустимые варианты для V2OperationType: UNKNOWN, TON_TAC_TON, TAC_TON, TON_TAC. Других вариантов для v2 быть не может по условиям таска. Другие варианты возможны только в v1"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Допустимые варианты для V2OperationType: UNKNOWN, TON_TAC_TON, TAC_TON, TON_TAC. Других вариантов для v2 быть не может по условиям таска. Другие варианты возможны только в v1, Source Nodes

### Community 62 - "Task To Code Workflow"
Cohesion: 0.40
Nodes (4): Preconditions, Process, Rules, Task To Code Workflow

### Community 63 - "README.md"
Cohesion: 0.40
Nodes (4): Configuration Parameters, Dev, Improvements, Intro

### Community 64 - "Project Brief"
Cohesion: 0.50
Nodes (4): Data model (Postgres), Project Brief, Two-phase sync, Workspace layout

### Community 65 - "stage_type.rs"
Cohesion: 0.50
Nodes (3): Model, Relation, String

## Knowledge Gaps
- **168 isolated node(s):** `Relation`, `ActiveModel`, `Relation`, `ActiveModel`, `Relation` (+163 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **19 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Work-memory lessons

**Preferred sources** — corroborated by past sessions; start here.
- `services/operations.rs` (2× useful, score=1.992956604) _(code changed — re-verify)_
- `.convert_full_v2()` (2× useful, score=1.992956604) _(code changed — re-verify)_
- `V2OperationBriefDetails` (2× useful, score=1.992956604)
- `V2OperationDetails` (2× useful, score=1.992956604)

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `TacDatabase` connect `TacDatabase` to `Indexer`, `OperationsService`, `StatisticService`?**
  _High betweenness centrality (0.250) - this node is a cross-community bridge._
- **Why does `Indexer` connect `Indexer` to `TacDatabase`, `Client`, `Settings`?**
  _High betweenness centrality (0.091) - this node is a cross-community bridge._
- **Why does `Client` connect `Client` to `Indexer`, `StatisticService`, `Settings`?**
  _High betweenness centrality (0.071) - this node is a cross-community bridge._
- **What connects `Relation`, `ActiveModel`, `Relation` to the rest of the system?**
  _168 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `TacDatabase` be split into smaller, more focused modules?**
  _Cohesion score 0.06726149622512011 - nodes in this community are weakly interconnected._
- **Should `Indexer` be split into smaller, more focused modules?**
  _Cohesion score 0.058823529411764705 - nodes in this community are weakly interconnected._
- **Should `OperationsService` be split into smaller, more focused modules?**
  _Cohesion score 0.07103825136612021 - nodes in this community are weakly interconnected._