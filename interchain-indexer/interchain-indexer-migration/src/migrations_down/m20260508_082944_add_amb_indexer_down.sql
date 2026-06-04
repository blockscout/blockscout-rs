DROP TABLE IF EXISTS amb_message_anomalies;
DROP TABLE amb_messages_confirmations;
ALTER TABLE bridge_contracts DROP COLUMN IF EXISTS kind;
