ALTER TYPE bridge_type   ADD VALUE IF NOT EXISTS 'xdai';
ALTER TYPE transfer_type ADD VALUE IF NOT EXISTS 'erc20_to_native';
ALTER TYPE transfer_type ADD VALUE IF NOT EXISTS 'native_to_erc20';

-- The table name is AMB-flavoured; its contents are not. Record the shared
-- ownership in the database itself, where the next reader of \d+ will see it.
COMMENT ON TABLE amb_messages_confirmations IS
  'Per-validator signature confirmations. Shared by the AMB/Omnibridge and '
  'xDai indexers: every column is protocol-agnostic and rows are keyed by '
  '(message_id, bridge_id), so bridge_id disambiguates. The amb_ prefix is '
  'historical.';
