-- ENSv2 parts of ensdomains/ens-subgraph 2.3+ that BENS reads (graph-node
-- column layout, only the columns BENS uses) and rows for the ENSv2 tests:
--  * ensv2live.eth: a name live in ENSv2, whose Domain.registryToken BENS
--    links (registry address ++ uint256 token id 123456789);
--  * a default (ENSIP-19) primary name for vitalik's address pointing at
--    sashaxyz.eth, which must lose to vitalik.eth on addr.reverse;
--  * a default primary name for 0x2222..2222 -> ensv2live.eth, its only one;
--  * 0x4444..4444: addr.reverse names the expired expired.eth, and a default
--    primary name points at the live liveforexpired.eth. ENSIP-19: the
--    non-empty addr.reverse name wins, fails verification, no label.

ALTER TABLE sgd1.domain ADD COLUMN registry_token bytea;

CREATE TABLE sgd1.en_sv_2_primary_name (
    vid bigserial PRIMARY KEY,
    block_range int4range NOT NULL,
    id bytea NOT NULL,
    account text NOT NULL,
    name text,
    block_number numeric NOT NULL,
    log_index numeric NOT NULL,
    reverse_node bytea
);

INSERT INTO sgd1.domain (vid, block_range, id, name, label_name, labelhash, parent, subdomain_count, resolved_address, resolver, ttl, is_migrated, created_at, owner, registrant, wrapped_owner, expiry_date, registry_token) VALUES
(900001, '[26300000,)', '0xd65adc62b2686960c04d8624f14b8c3030126b3be26ac6d980f12a27c1c110e3', 'ensv2live.eth', 'ensv2live', '\x881e26e6c45a3de6a8cff5083c12381e6c2e9a17a5a3b5dcb6970f16861f099c', '0x93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae', 0, '0x2222222222222222222222222222222222222222', NULL, NULL, true, 1793213027, '0x2222222222222222222222222222222222222222', '0x2222222222222222222222222222222222222222', NULL, 1826749027, '\xf60aba43e1f7f3dab1186ff03b561401c9df85e200000000000000000000000000000000000000000000000000000000075bcd15');

INSERT INTO sgd1.en_sv_2_primary_name (block_range, id, account, name, block_number, log_index, reverse_node) VALUES
('[26300001,)', '\xd8da6bf26964af9d7eed9e03e53415d37aa96045', '0xd8da6bf26964af9d7eed9e03e53415d37aa96045', 'sashaxyz.eth', 26300001, 1, '\x7aef81fbd30c83431369026d62ee533af8b69f246b63d75b40fe223346e6fa9a'),
('[26300002,)', '\x2222222222222222222222222222222222222222', '0x2222222222222222222222222222222222222222', 'ensv2live.eth', 26300002, 1, '\x22ed692c052e5dfd9529c08ac2bb48bd230f6133289d272dd3a0961697ef3f26');

INSERT INTO sgd1.domain (vid, block_range, id, name, label_name, labelhash, parent, subdomain_count, resolved_address, resolver, ttl, is_migrated, created_at, owner, registrant, wrapped_owner, expiry_date) VALUES
(900002, '[26300000,)', '0x98ceb03f609d49cc1ca016273fc651576efc7f306ce3b3ad55c53553623aa09a', '4444444444444444444444444444444444444444.addr.reverse', '4444444444444444444444444444444444444444', NULL, '0x91d1777781884d03a6757a803996e38de2a42967fb37eeaca72729271025a9e2', 0, NULL, '0x4976fb03c32e5b8cfe2b6ccb31c09ba78ebaba41-0x98ceb03f609d49cc1ca016273fc651576efc7f306ce3b3ad55c53553623aa09a', NULL, true, 1600000000, '0x4444444444444444444444444444444444444444', NULL, NULL, NULL),
(900003, '[26300000,)', '0xf212d0bf2d80874cf34e3045ecae973e16c6721c3ff214b2d90861bb900c2ae7', 'liveforexpired.eth', 'liveforexpired', '\x45a331862b3e8dfa6a1ad43b8cf1cefd9e0e17045fbb1fd7787d2c0bc3df5f9a', '0x93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae', 0, '0x4444444444444444444444444444444444444444', NULL, NULL, true, 1793213027, '0x4444444444444444444444444444444444444444', '0x4444444444444444444444444444444444444444', NULL, 1826749027);

INSERT INTO sgd1.name_changed (vid, block_range, id, resolver, block_number, transaction_id, name) VALUES
(900001, '[13000000,)', '13000000-0-900001', '0x4976fb03c32e5b8cfe2b6ccb31c09ba78ebaba41-0x98ceb03f609d49cc1ca016273fc651576efc7f306ce3b3ad55c53553623aa09a', 13000000, '\x00', 'expired.eth');

INSERT INTO sgd1.en_sv_2_primary_name (block_range, id, account, name, block_number, log_index, reverse_node) VALUES
('[26300003,)', '\x4444444444444444444444444444444444444444', '0x4444444444444444444444444444444444444444', 'liveforexpired.eth', 26300003, 1, '\x98ceb03f609d49cc1ca016273fc651576efc7f306ce3b3ad55c53553623aa09a');
