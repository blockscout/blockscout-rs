---
type: "query"
date: "2026-08-07T06:27:10.168046+00:00"
question: "Переименовать Pending в UNKNOWN; сделать type и status enum и rollback обязательным bool в V2OperationBriefDetails/V2OperationDetails."
contributor: "graphify"
outcome: "useful"
source_nodes: ["V2OperationBriefDetails", "V2OperationDetails", ".convert_full_v2()", "services/operations.rs"]
---

# Q: Переименовать Pending в UNKNOWN; сделать type и status enum и rollback обязательным bool в V2OperationBriefDetails/V2OperationDetails.

## Answer

Expanded from original query via graph vocab: [enum, type, status, operation, brief, details, proto, api, unknown, convert, projection, field]. Traversal located the v2 DTO definitions and both conversion paths in services/operations.rs, confirming the change belongs in the API projection and proto schema rather than persistence or lifecycle write logic.

## Outcome

- Signal: useful

## Source Nodes

- V2OperationBriefDetails
- V2OperationDetails
- .convert_full_v2()
- services/operations.rs