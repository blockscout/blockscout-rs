---
type: "query"
date: "2026-08-06T23:31:06.888964+00:00"
question: "Где формируются V2OperationBriefDetails и V2OperationDetails и как на уровне API спроецировать status/type по profiling_version?"
contributor: "graphify"
outcome: "useful"
source_nodes: [".convert_full_v2()", "V2OperationDetails", "services/operations.rs", "V2OperationBriefDetails"]
---

# Q: Где формируются V2OperationBriefDetails и V2OperationDetails и как на уровне API спроецировать status/type по profiling_version?

## Answer

Expanded from original query via graph vocab: [api, brief, details, operation, lifecycle, status, profiling, version, finalized, pending, type, projection]. The public v2 DTOs are built in OperationsService::convert_short_v2 and OperationsService::convert_full_v2; both consume the local OperationsService::v2_lifecycle API projection in tac-operation-lifecycle-server/src/services/operations.rs. The protobuf contract is generated from proto/v2/tac-operation-lifecycle.proto. Database entities and lifecycle/indexer business logic are upstream of this boundary and need no changes.

## Outcome

- Signal: useful

## Source Nodes

- .convert_full_v2()
- V2OperationDetails
- services/operations.rs
- V2OperationBriefDetails