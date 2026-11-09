// Graveyard.clear() (ensdomains/contracts-v2) retires an expired ENSv1 .eth
// name by re-registering it to the Graveyard for ~uint64 max. On chain that
// is: Transfer(holder -> 0x0) [burn], Transfer(0x0 -> Graveyard) [mint],
// registry NewOwner(eth, label, Graveyard), NameRegistered(id, Graveyard,
// ~2^64), then NewResolver(node, 0x0). The subgraph must keep the lapsed
// registrant/expiry (the name is expired and available, not owned by the
// Graveyard) while recording the events, and must still treat an ordinary
// re-registration of an expired name as a new registration.
import { Address, BigInt, Bytes, crypto, ethereum } from "@graphprotocol/graph-ts";
import {
  assert,
  beforeEach,
  clearStore,
  dataSourceMock,
  describe,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import { handleNewOwner, handleNewResolver } from "../src/ensRegistry";
import { handleNameRegistered, handleNameTransferred } from "../src/ethRegistrar";
import {
  NameRegistered,
  Transfer,
} from "../src/types/BaseRegistrar/BaseRegistrar";
import { NewResolver } from "../src/types/ENSRegistry/EnsRegistry";
import { Domain, WrappedDomain } from "../src/types/schema";
import { concat, ETH_NODE, MAX_LEGACY_EXPIRY, uint256ToByteArray } from "../src/utils";
import { createNewOwnerEvent, setEthOwner } from "./testUtils";

const MAINNET_GRAVEYARD = "0xda021a2c1c2f99574d861ef8c6fa745851beb7f6";
const ZERO = "0x0000000000000000000000000000000000000000";
const HOLDER = "0x89205a3a3b2a69de6dbf7f01ed13b2108b2c43e7";
const NEW_REGISTRANT = "0xf9e21edb4dfc9ff648da6ded3864b79e626e578a";
const NAME_WRAPPER = "0xd4416b13d2b3a9abae7acd5d6c2bbdbe25686401";

// type(uint64).max - block.timestamp - GRACE_PERIOD, as emitted on mainnet
// (block 26,297,075 onwards): 18446744073701775615.
const RETIREMENT_EXPIRY = "18446744073701775615";
const ORIGINAL_EXPIRY = "1600000000";
const ORIGINAL_DOMAIN_EXPIRY = "1607776000"; // + 90 day grace
const SWEEP_TIMESTAMP = "1793000000";

const TOKEN_ID = BigInt.fromI32(89056);
const LABEL = Bytes.fromByteArray(uint256ToByteArray(TOKEN_ID));
const LABEL_HEX = LABEL.toHexString();
const DOMAIN_ID = Bytes.fromByteArray(
  crypto.keccak256(concat(ETH_NODE, LABEL))
).toHexString();

// Legacy event IDs are tx hash + log index; give every mock event its own
// log index so history rows do not overwrite each other.
let nextLogIndex = 0;
function withLogIndex<T extends ethereum.Event>(event: T): T {
  event.logIndex = BigInt.fromI32(++nextLogIndex);
  return event;
}

function nameRegistered(owner: string, expires: string, timestamp: string): NameRegistered {
  let mockEvent = newMockEvent();
  let event = new NameRegistered(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    new Array<ethereum.EventParam>(),
    mockEvent.receipt
  );
  withLogIndex(event);
  event.block.timestamp = BigInt.fromString(timestamp);
  event.parameters.push(
    new ethereum.EventParam("id", ethereum.Value.fromUnsignedBigInt(TOKEN_ID))
  );
  event.parameters.push(
    new ethereum.EventParam("owner", ethereum.Value.fromAddress(Address.fromString(owner)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "expires",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromString(expires))
    )
  );
  return event;
}

function transfer(from: string, to: string): Transfer {
  let mockEvent = newMockEvent();
  let event = new Transfer(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    new Array<ethereum.EventParam>(),
    mockEvent.receipt
  );
  withLogIndex(event);
  event.parameters.push(
    new ethereum.EventParam("from", ethereum.Value.fromAddress(Address.fromString(from)))
  );
  event.parameters.push(
    new ethereum.EventParam("to", ethereum.Value.fromAddress(Address.fromString(to)))
  );
  event.parameters.push(
    new ethereum.EventParam("tokenId", ethereum.Value.fromUnsignedBigInt(TOKEN_ID))
  );
  return event;
}

function newResolver(node: string, resolver: string): NewResolver {
  let mockEvent = newMockEvent();
  let event = new NewResolver(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    new Array<ethereum.EventParam>(),
    mockEvent.receipt
  );
  event.parameters.push(
    new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(Bytes.fromHexString(node)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "resolver",
      ethereum.Value.fromAddress(Address.fromString(resolver))
    )
  );
  return event;
}

// A .eth name registered by HOLDER in 2019-ish and long expired.
function seedLapsedRegistration(): void {
  handleNewOwner(createNewOwnerEvent(ETH_NODE.toHexString(), LABEL_HEX, HOLDER));
  handleNameTransferred(transfer(ZERO, HOLDER));
  handleNameRegistered(nameRegistered(HOLDER, ORIGINAL_EXPIRY, "1500000000"));
  assert.fieldEquals("Registration", LABEL_HEX, "registrant", HOLDER);
  assert.fieldEquals("Registration", LABEL_HEX, "expiryDate", ORIGINAL_EXPIRY);
  assert.fieldEquals("Domain", DOMAIN_ID, "expiryDate", ORIGINAL_DOMAIN_EXPIRY);
}

// Exactly the event sequence of one name inside a Graveyard.clear() tx.
function sweep(): void {
  handleNameTransferred(transfer(HOLDER, ZERO));
  handleNameTransferred(transfer(ZERO, MAINNET_GRAVEYARD));
  handleNewOwner(createNewOwnerEvent(ETH_NODE.toHexString(), LABEL_HEX, MAINNET_GRAVEYARD));
  handleNameRegistered(nameRegistered(MAINNET_GRAVEYARD, RETIREMENT_EXPIRY, SWEEP_TIMESTAMP));
  handleNewResolver(newResolver(DOMAIN_ID, ZERO));
}

describe("Graveyard retirement of expired ENSv1 names", () => {
  beforeEach(() => {
    clearStore();
    dataSourceMock.setNetwork("mainnet");
    setEthOwner();
  });

  test("keeps the lapsed registrant and expiry instead of a Graveyard-owned registration", () => {
    seedLapsedRegistration();

    sweep();

    assert.fieldEquals("Registration", LABEL_HEX, "registrant", HOLDER);
    assert.fieldEquals("Registration", LABEL_HEX, "expiryDate", ORIGINAL_EXPIRY);
    assert.fieldEquals("Registration", LABEL_HEX, "registrationDate", "1500000000");
    assert.fieldEquals("Domain", DOMAIN_ID, "registrant", HOLDER);
    assert.fieldEquals("Domain", DOMAIN_ID, "expiryDate", ORIGINAL_DOMAIN_EXPIRY);
    // The registry really did move the node to the Graveyard, as for
    // migrated names; consumers already treat that address as "no holder".
    assert.fieldEquals("Domain", DOMAIN_ID, "owner", MAINNET_GRAVEYARD);
    let domain = Domain.load(DOMAIN_ID)!;
    assert.assertTrue(!domain.resolver);
    assert.assertTrue(domain.expiryDate!.lt(MAX_LEGACY_EXPIRY));
  });

  test("records the on-chain events as history", () => {
    seedLapsedRegistration();
    // The original mint precedes the Registration, so (as always) it leaves
    // no NameTransferred row.
    assert.entityCount("NameRegistered", 1);
    assert.entityCount("NameTransferred", 0);

    sweep();

    // burn + mint, and the retirement registration on top of the original
    // one. Event rows keep the raw on-chain values.
    assert.entityCount("NameTransferred", 2);
    assert.entityCount("NameRegistered", 2);
  });

  test("clears the stale wrapped state of an expired wrapped name", () => {
    seedLapsedRegistration();
    let domain = Domain.load(DOMAIN_ID)!;
    domain.owner = NAME_WRAPPER;
    domain.wrappedOwner = HOLDER;
    domain.save();
    let wrapped = new WrappedDomain(DOMAIN_ID);
    wrapped.domain = DOMAIN_ID;
    wrapped.owner = HOLDER;
    wrapped.fuses = 0;
    wrapped.expiryDate = BigInt.fromString(ORIGINAL_DOMAIN_EXPIRY);
    wrapped.save();

    sweep();

    assert.entityCount("WrappedDomain", 0);
    assert.fieldEquals("Domain", DOMAIN_ID, "owner", MAINNET_GRAVEYARD);
    assert.fieldEquals("Domain", DOMAIN_ID, "registrant", HOLDER);
    domain = Domain.load(DOMAIN_ID)!;
    assert.assertTrue(!domain.wrappedOwner);
  });

  test("an ordinary re-registration of an expired name is still a new registration", () => {
    seedLapsedRegistration();

    // Same burn/mint/NewOwner/NameRegistered shape, but to a real registrant.
    handleNameTransferred(transfer(HOLDER, ZERO));
    handleNameTransferred(transfer(ZERO, NEW_REGISTRANT));
    handleNewOwner(createNewOwnerEvent(ETH_NODE.toHexString(), LABEL_HEX, NEW_REGISTRANT));
    handleNameRegistered(nameRegistered(NEW_REGISTRANT, "1900000000", SWEEP_TIMESTAMP));

    assert.fieldEquals("Registration", LABEL_HEX, "registrant", NEW_REGISTRANT);
    assert.fieldEquals("Registration", LABEL_HEX, "expiryDate", "1900000000");
    assert.fieldEquals("Registration", LABEL_HEX, "registrationDate", SWEEP_TIMESTAMP);
    assert.fieldEquals("Domain", DOMAIN_ID, "registrant", NEW_REGISTRANT);
    assert.fieldEquals("Domain", DOMAIN_ID, "owner", NEW_REGISTRANT);
    assert.fieldEquals("Domain", DOMAIN_ID, "expiryDate", "1907776000");
  });

  test("a migration transfer of a live name to the Graveyard is unchanged (registrant follows the token until corrected)", () => {
    seedLapsedRegistration();

    // LockedMigrationController/UnlockedMigrationController move the live
    // token with a plain transfer from the holder: no burn, no mint.
    handleNameTransferred(transfer(HOLDER, MAINNET_GRAVEYARD));

    assert.fieldEquals("Registration", LABEL_HEX, "registrant", MAINNET_GRAVEYARD);
    assert.fieldEquals("Domain", DOMAIN_ID, "registrant", MAINNET_GRAVEYARD);
  });

  test("a Graveyard registration with no prior Registration is skipped rather than invented", () => {
    handleNewOwner(createNewOwnerEvent(ETH_NODE.toHexString(), LABEL_HEX, MAINNET_GRAVEYARD));
    handleNameTransferred(transfer(ZERO, MAINNET_GRAVEYARD));
    handleNameRegistered(nameRegistered(MAINNET_GRAVEYARD, RETIREMENT_EXPIRY, SWEEP_TIMESTAMP));

    assert.entityCount("Registration", 0);
    assert.entityCount("NameRegistered", 0);
    let domain = Domain.load(DOMAIN_ID)!;
    assert.assertTrue(!domain.registrant);
    assert.assertTrue(!domain.expiryDate);
  });
});
