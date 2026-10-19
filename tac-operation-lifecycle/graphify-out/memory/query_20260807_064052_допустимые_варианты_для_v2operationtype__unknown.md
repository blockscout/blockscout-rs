---
type: "query"
date: "2026-08-07T06:40:52.860337+00:00"
question: "Допустимые варианты для V2OperationType: UNKNOWN, TON_TAC_TON, TAC_TON, TON_TAC. Других вариантов для v2 быть не может по условиям таска. Другие варианты возможны только в v1"
contributor: "graphify"
outcome: "corrected"
correction: "V2OperationType has exactly four variants: UNKNOWN, TON_TAC_TON, TAC_TON, TON_TAC. PENDING, ROLLBACK, INSUFFICIENT_FEE, and ERROR are v1-only and must not appear in the v2 enum."
source_nodes: ["V2OperationBriefDetails", "V2OperationDetails", ".convert_short_v2()", ".convert_full_v2()", ".v2_lifecycle()"]
---

# Q: Допустимые варианты для V2OperationType: UNKNOWN, TON_TAC_TON, TAC_TON, TON_TAC. Других вариантов для v2 быть не может по условиям таска. Другие варианты возможны только в v1

## Answer

Expanded from original query via graph vocab: [operation, type, enum, unknown, brief, details, convert, projection, status, api, proto]. The v2 DTOs are both produced through the shared v2_lifecycle projection in services/operations.rs. V2OperationType must contain only UNKNOWN, TON_TAC_TON, TAC_TON, and TON_TAC; every legacy-only or unrecognized stored type projects to UNKNOWN while status and rollback continue to use their independent source fields.

## Outcome

- Signal: corrected
- Correction: V2OperationType has exactly four variants: UNKNOWN, TON_TAC_TON, TAC_TON, TON_TAC. PENDING, ROLLBACK, INSUFFICIENT_FEE, and ERROR are v1-only and must not appear in the v2 enum.

## Source Nodes

- V2OperationBriefDetails
- V2OperationDetails
- .convert_short_v2()
- .convert_full_v2()
- .v2_lifecycle()