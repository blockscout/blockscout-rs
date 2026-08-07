# Graph Report - .  (2026-08-07)

## Corpus Check
- Corpus is ~24,769 words - fits in a single context window. You may not need a graph.

## Summary
- 655 nodes · 1227 edges · 49 communities (36 shown, 13 thin omitted)
- Extraction: 98% EXTRACTED · 2% INFERRED · 0% AMBIGUOUS · INFERRED: 24 edges (avg confidence: 0.83)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- Database Query Mapping
- Indexer Job Processing
- Read API Services
- Profiling API Models
- Upstream HTTP Client
- Server Routing
- Operation Response Models
- Insufficient Fee Migration
- Lifecycle Projection
- TypeScript Package
- Task Delivery Skills
- Indexer Configuration
- Metadata Migration
- Proto Code Generation
- Entity Prelude Types
- Initial Schema Migration
- Profiler V2 Migration
- Operation Metadata Entity
- Migration Registry
- Operation Entity
- Schema Migration
- Interval Entity
- Operation Stage Entity
- Transaction Entity
- Entity Relationship
- Entity Relationship
- Entity Relationship
- Entity Relationship
- Entity Relationship
- Watermark Entity
- Server Entrypoint
- Research Scope Skill
- Implementation Plan Skill
- PR Description Skill
- Research Scope Skill
- Implementation Plan Skill
- PR Description Skill
- Research Scope Skill
- SeaORM Active Model
- SeaORM Active Model
- SeaORM Active Model
- SeaORM Active Model
- SeaORM Active Model
- SeaORM Active Model
- SeaORM Active Model

## God Nodes (most connected - your core abstractions)
1. `TacDatabase` - 56 edges
2. `Indexer` - 28 edges
3. `Client` - 27 edges
4. `OperationsService` - 20 edges
5. `Settings` - 17 edges
6. `V1OperationData` - 13 edges
7. `V2OperationData` - 13 edges
8. `SourceOperationData` - 13 edges
9. `ProfilingError` - 12 edges
10. `Stage` - 10 edges

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

## Communities (49 total, 13 thin omitted)

### Community 0 - "Database Query Mapping"
Cohesion: 0.07
Nodes (38): ApiOperations, DatabaseConnection, DatabaseTransaction, Entity, Select, EntityStatus, IntervalDbStatistic, JoinedRow (+30 more)

### Community 1 - "Indexer Job Processing"
Cohesion: 0.06
Nodes (33): BoxStream, JoinHandle, PollNext, BlockchainType, Indexer, IndexerJob, Job, LegacyOperationType (+25 more)

### Community 2 - "Read API Services"
Cohesion: 0.08
Nodes (41): GetOperationByTxHashRequest, GetOperationDetailsRequest, GetOperationsRequest, OperationBriefDetails, OperationDetails, OperationsFullResponse, OperationsResponse, OperationWithStages (+33 more)

### Community 3 - "Profiling API Models"
Cohesion: 0.11
Nodes (37): BlockchainType, D, Address, BlockchainType, deserialize_fee_info(), deserialize_note_to_string(), deserialize_valid_executors(), FeeValue (+29 more)

### Community 4 - "Upstream HTTP Client"
Cohesion: 0.09
Nodes (28): HttpClient, Instant, Limiter, Mutex, StatusCode, T, ApiResponse, Client (+20 more)

### Community 5 - "Server Routing"
Cohesion: 0.06
Nodes (34): GetFullStatisticRequest, GetFullStatisticResponse, GetIntervalStatisticsRequest, GetIntervalStatisticsResponse, GetOperationStatisticsRequest, GetOperationStatisticsResponse, Health, HealthCheckRequest (+26 more)

### Community 6 - "Operation Response Models"
Cohesion: 0.07
Nodes (28): ConfigSettings, DatabaseSettings, Deserialize, JaegerSettings, MetricsSettings, ServerSettings, Operation, OperationIdsApiResponse (+20 more)

### Community 7 - "Insufficient Fee Migration"
Cohesion: 0.11
Nodes (22): ConnectionTrait, F, fetch_op_type(), Migration, migration_marks_and_reverts_insufficient_fee_operations(), DbErr, MigrationTrait, Option (+14 more)

### Community 8 - "Lifecycle Projection"
Cohesion: 0.14
Nodes (18): derive_operation_error_reason(), derive_v1_source_type(), error_reason_from_note(), error_reason_supports_content_and_plain_text_notes(), error_reason_uses_latest_failed_stage_and_best_note_field(), failed_stage(), has_insufficient_fee_stages(), insufficient_fee_has_priority_over_other_failed_stage_reasons() (+10 more)

### Community 9 - "TypeScript Package"
Cohesion: 0.09
Nodes (22): ts-proto, bugs, url, description, devDependencies, ts-proto, typescript, homepage (+14 more)

### Community 10 - "Task Delivery Skills"
Cohesion: 0.10
Nodes (21): Solution Review Skill, Solution Review Workflow, Task Analysis Skill, Task Analysis Workflow, Task To Code Skill, Task To Code Workflow, Implementation Plan Agent Interface, Implementation Plan Skill (+13 more)

### Community 11 - "Indexer Configuration"
Cohesion: 0.22
Nodes (17): default_catchup_interval(), default_concurrency(), default_enabled(), default_forever_pending_operations_age_sec(), default_intervals_loop_delay_ms(), default_intervals_query_batch(), default_intervals_retry_batch(), default_operations_loop_delay_ms() (+9 more)

### Community 12 - "Metadata Migration"
Cohesion: 0.27
Nodes (7): Migration, Operation, OperationMetaInfo, DbErr, MigrationTrait, Result, SchemaManager

### Community 13 - "Proto Code Generation"
Cohesion: 0.36
Nodes (8): AsRef, Path, ServiceGenerator, compile(), main(), Box, Error, Result

### Community 14 - "Entity Prelude Types"
Cohesion: 0.22
Nodes (4): StatusEnum, Model, Relation, String

### Community 15 - "Initial Schema Migration"
Cohesion: 0.22
Nodes (8): Interval, Operation, OperationStage, StageType, StatusEnum, StatusVariants, Transaction, WaterMark

### Community 16 - "Profiler V2 Migration"
Cohesion: 0.36
Nodes (6): Migration, preserves_legacy_rows_and_projects_v2_on_down(), DbErr, MigrationTrait, Result, SchemaManager

### Community 17 - "Operation Metadata Entity"
Cohesion: 0.29
Nodes (6): Decimal, Model, Relation, Option, String, Vec

### Community 18 - "Migration Registry"
Cohesion: 0.29
Nodes (5): MigratorTrait, Migrator, Box, MigrationTrait, Vec

### Community 19 - "Operation Entity"
Cohesion: 0.33
Nodes (6): Model, Relation, DateTime, Option, StatusEnum, String

### Community 20 - "Schema Migration"
Cohesion: 0.43
Nodes (5): Migration, DbErr, MigrationTrait, Result, SchemaManager

### Community 21 - "Interval Entity"
Cohesion: 0.40
Nodes (5): Model, Relation, DateTime, Option, StatusEnum

### Community 22 - "Operation Stage Entity"
Cohesion: 0.33
Nodes (5): Model, Relation, DateTime, Option, String

### Community 23 - "Transaction Entity"
Cohesion: 0.40
Nodes (4): Model, Relation, DateTime, String

### Community 24 - "Entity Relationship"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 25 - "Entity Relationship"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 26 - "Entity Relationship"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 27 - "Entity Relationship"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 28 - "Entity Relationship"
Cohesion: 0.50
Nodes (3): Entity, Related, RelationDef

### Community 29 - "Watermark Entity"
Cohesion: 0.50
Nodes (3): Model, Relation, DateTime

### Community 30 - "Server Entrypoint"
Cohesion: 0.50
Nodes (3): main(), Error, Result

### Community 31 - "Research Scope Skill"
Cohesion: 0.67
Nodes (3): Research Scope Agent Interface, Research Scope Skill, Research Scope Workflow

## Knowledge Gaps
- **70 isolated node(s):** `Relation`, `ActiveModel`, `Relation`, `ActiveModel`, `Relation` (+65 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **13 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `TacDatabase` connect `Database Query Mapping` to `Indexer Job Processing`, `Read API Services`, `Server Routing`?**
  _High betweenness centrality (0.357) - this node is a cross-community bridge._
- **Why does `Indexer` connect `Indexer Job Processing` to `Database Query Mapping`, `Indexer Configuration`, `Upstream HTTP Client`?**
  _High betweenness centrality (0.132) - this node is a cross-community bridge._
- **Why does `Client` connect `Upstream HTTP Client` to `Indexer Job Processing`, `Server Routing`, `Operation Response Models`?**
  _High betweenness centrality (0.103) - this node is a cross-community bridge._
- **What connects `Relation`, `ActiveModel`, `Relation` to the rest of the system?**
  _70 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Database Query Mapping` be split into smaller, more focused modules?**
  _Cohesion score 0.07016229712858926 - nodes in this community are weakly interconnected._
- **Should `Indexer Job Processing` be split into smaller, more focused modules?**
  _Cohesion score 0.058823529411764705 - nodes in this community are weakly interconnected._
- **Should `Read API Services` be split into smaller, more focused modules?**
  _Cohesion score 0.07662337662337662 - nodes in this community are weakly interconnected._