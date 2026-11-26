DROP TABLE IF EXISTS op_interop_events;
-- PostgreSQL cannot remove an enum value; 'op_interop' stays in bridge_type,
-- 'pending_finality' and 'invalidated' stay in message_status.
