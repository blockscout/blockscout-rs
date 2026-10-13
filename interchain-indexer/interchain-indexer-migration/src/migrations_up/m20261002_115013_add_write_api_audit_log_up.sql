-- Operator Write API audit log: one row per APPLIED change, committed in the
-- same transaction as the change. No foreign keys to target tables by design:
-- targets are stored in JSONB, and an FK to stats_assets would turn every
-- later merge of an audited asset into a failing maintenance transaction.
CREATE TABLE write_api_audit_log (
  id          BIGSERIAL PRIMARY KEY,
  occurred_at TIMESTAMP NOT NULL DEFAULT now(),
  actor       TEXT      NOT NULL,
  method      TEXT      NOT NULL,
  reason      TEXT      NOT NULL,
  request     JSONB     NOT NULL,
  result      JSONB     NOT NULL
);
CREATE INDEX write_api_audit_log_occurred_at_idx ON write_api_audit_log (occurred_at);
