-- Two NameChanged events for one addr.reverse resolver in the same block,
-- whose vids are in the opposite order to their log indexes: graph-node
-- assigns vid in write order, not log order, below specVersion 1.3.0.
-- 0x5555..5555 sets fivefive.eth (log 10), then clears it (log 20). The
-- clear is the current state, so the address has no addr.reverse name and
-- gets no label, although fivefive.eth resolves to it.

INSERT INTO sgd1.domain (vid, block_range, id, name, label_name, labelhash, parent, subdomain_count, resolved_address, resolver, ttl, is_migrated, created_at, owner, registrant, wrapped_owner, expiry_date) VALUES
(900004, '[13000000,)', '0x622e3701519423a39b5b95cfcfde55e36cde654584efc0db243e09d879491152', '5555555555555555555555555555555555555555.addr.reverse', '5555555555555555555555555555555555555555', NULL, '0x91d1777781884d03a6757a803996e38de2a42967fb37eeaca72729271025a9e2', 0, NULL, '0x4976fb03c32e5b8cfe2b6ccb31c09ba78ebaba41-0x622e3701519423a39b5b95cfcfde55e36cde654584efc0db243e09d879491152', NULL, true, 1500000000, '0x5555555555555555555555555555555555555555', NULL, NULL, NULL),
(900005, '[13000000,)', '0xf69902eda9008114d6783d132742bea47378a72a3383082410d5289c33649dab', 'fivefive.eth', 'fivefive', '\xc3d566169ce07f3591c78b61812c1471137ab362d8aa3f021c981400f2ad0cd9', '0x93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae', 0, '0x5555555555555555555555555555555555555555', NULL, NULL, true, 1500000000, '0x5555555555555555555555555555555555555555', '0x5555555555555555555555555555555555555555', NULL, 253402300799);

INSERT INTO sgd1.name_changed (vid, block_range, id, resolver, block_number, transaction_id, name) VALUES
(900010, '[13000001,)', '13000001-20', '0x4976fb03c32e5b8cfe2b6ccb31c09ba78ebaba41-0x622e3701519423a39b5b95cfcfde55e36cde654584efc0db243e09d879491152', 13000001, '\x00', ''),
(900011, '[13000001,)', '13000001-10', '0x4976fb03c32e5b8cfe2b6ccb31c09ba78ebaba41-0x622e3701519423a39b5b95cfcfde55e36cde654584efc0db243e09d879491152', 13000001, '\x00', 'fivefive.eth');
