import { Address, BigInt, Bytes, ethereum } from "@graphprotocol/graph-ts";
import {
  assert,
  beforeAll,
  describe,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import {
  handleExpiryExtended,
  handleFusesSet,
  handleNameUnwrapped,
  handleTransferBatch,
  handleTransferSingle,
} from "../src/nameWrapper";
import {
  ExpiryExtended,
  FusesSet,
  NameUnwrapped,
  TransferBatch,
  TransferSingle,
} from "../src/types/NameWrapper/NameWrapper";
import { Domain, WrappedDomain } from "../src/types/schema";
import { createLegacyEventID, ETH_NODE } from "../src/utils";
import { DEFAULT_OWNER, setEthOwner } from "./testUtils";

beforeAll(() => {
  setEthOwner();
});

const NAME_WRAPPER_ADDRESS = "0xD4416b13d2b3a9aBae7AcD5D6C2BbDBE25686401";
// test.eth
const testEthNamehash =
  "0xeb4f647bea6caa36333c816d7b46fdcb05f9466ecacc140ea8c66faf15b3d9f1";

const createNameUnwrappedEvent = (
  node: string,
  owner: string
): NameUnwrapped => {
  let mockEvent = newMockEvent();
  let newNameUnwrappedEvent = new NameUnwrapped(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  newNameUnwrappedEvent.parameters = new Array();
  let nodeParam = new ethereum.EventParam(
    "node",
    ethereum.Value.fromBytes(Bytes.fromHexString(node))
  );
  let ownerParam = new ethereum.EventParam(
    "owner",
    ethereum.Value.fromAddress(Address.fromString(owner))
  );
  newNameUnwrappedEvent.parameters.push(nodeParam);
  newNameUnwrappedEvent.parameters.push(ownerParam);
  return newNameUnwrappedEvent;
};

describe("handleNameUnwrapped", () => {
  test("does not set expiryDate to null if name is .eth", () => {
    // test
    const labelhash =
      "0x9c22ff5f21f0b81b113e63f7db6da94fedef11b2119b4088b89664fb9a3cb658";

    let domain = new Domain(testEthNamehash);
    domain.storedOffchain = false; // Blockscout/BENS extension
    domain.resolvedWithWildcard = false; // Blockscout/BENS extension
    domain.name = "test.eth";
    domain.labelName = "test";
    domain.labelhash = Bytes.fromHexString(labelhash);
    domain.parent = ETH_NODE.toHexString();
    domain.subdomainCount = 0;
    domain.isMigrated = true;
    domain.createdAt = BigInt.fromI32(0);
    domain.owner = NAME_WRAPPER_ADDRESS;
    domain.registrant = NAME_WRAPPER_ADDRESS;
    domain.wrappedOwner = DEFAULT_OWNER;
    domain.expiryDate = BigInt.fromI32(123456789);
    domain.save();

    const wrappedDomain = new WrappedDomain(testEthNamehash);
    wrappedDomain.domain = testEthNamehash;
    wrappedDomain.expiryDate = BigInt.fromI32(123456789);
    wrappedDomain.fuses = 0;
    wrappedDomain.owner = DEFAULT_OWNER;
    wrappedDomain.name = "test.eth";
    wrappedDomain.save();

    const nameUnwrappedEvent = createNameUnwrappedEvent(
      testEthNamehash,
      DEFAULT_OWNER
    );

    handleNameUnwrapped(nameUnwrappedEvent);

    assert.fieldEquals("Domain", testEthNamehash, "expiryDate", "123456789");
  });
  test("sets expiryDate to null if name is not .eth", () => {
    // cool.test.eth
    const subNamehash =
      "0x85c47d906feeeed4795f21773ab20983af35e85837d2de39549f650c8fb50c0f";
    // cool
    const labelhash =
      "0x678c189fde5058554d934d6af17e41750fa2a94b61371c5ea958a7595e146324";

    let domain = new Domain(subNamehash);
    domain.storedOffchain = false; // Blockscout/BENS extension
    domain.resolvedWithWildcard = false; // Blockscout/BENS extension
    domain.name = "cool.test.eth";
    domain.labelName = "cool";
    domain.labelhash = Bytes.fromHexString(labelhash);
    domain.parent = testEthNamehash;
    domain.subdomainCount = 0;
    domain.isMigrated = true;
    domain.createdAt = BigInt.fromI32(0);
    domain.owner = NAME_WRAPPER_ADDRESS;
    domain.registrant = NAME_WRAPPER_ADDRESS;
    domain.wrappedOwner = DEFAULT_OWNER;
    domain.expiryDate = BigInt.fromI32(123456789);
    domain.save();

    const wrappedDomain = new WrappedDomain(subNamehash);
    wrappedDomain.domain = subNamehash;
    wrappedDomain.expiryDate = BigInt.fromI32(123456789);
    wrappedDomain.fuses = 0;
    wrappedDomain.owner = DEFAULT_OWNER;
    wrappedDomain.name = "test.eth";
    wrappedDomain.save();

    const nameUnwrappedEvent = createNameUnwrappedEvent(
      subNamehash,
      DEFAULT_OWNER
    );

    handleNameUnwrapped(nameUnwrappedEvent);

    assert.fieldEquals("Domain", subNamehash, "expiryDate", "null");
  });
});

const createFusesSetEvent = (node: string, fuses: i32): FusesSet => {
  let mockEvent = newMockEvent();
  let event = new FusesSet(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(Bytes.fromHexString(node)))
  );
  event.parameters.push(
    new ethereum.EventParam("fuses", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(fuses)))
  );
  return event;
};

const createExpiryExtendedEvent = (node: string, expiry: BigInt): ExpiryExtended => {
  let mockEvent = newMockEvent();
  let event = new ExpiryExtended(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(Bytes.fromHexString(node)))
  );
  event.parameters.push(
    new ethereum.EventParam("expiry", ethereum.Value.fromUnsignedBigInt(expiry))
  );
  return event;
};

const createTransferSingleEvent = (
  from: string,
  to: string,
  id: BigInt
): TransferSingle => {
  let mockEvent = newMockEvent();
  let event = new TransferSingle(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("operator", ethereum.Value.fromAddress(Address.fromString(from)))
  );
  event.parameters.push(
    new ethereum.EventParam("from", ethereum.Value.fromAddress(Address.fromString(from)))
  );
  event.parameters.push(
    new ethereum.EventParam("to", ethereum.Value.fromAddress(Address.fromString(to)))
  );
  event.parameters.push(
    new ethereum.EventParam("id", ethereum.Value.fromUnsignedBigInt(id))
  );
  event.parameters.push(
    new ethereum.EventParam("value", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(1)))
  );
  return event;
};

const createTransferBatchEvent = (
  from: string,
  to: string,
  ids: Array<BigInt>
): TransferBatch => {
  let mockEvent = newMockEvent();
  let event = new TransferBatch(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  let values = new Array<BigInt>();
  for (let i = 0; i < ids.length; i++) {
    values.push(BigInt.fromI32(1));
  }
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("operator", ethereum.Value.fromAddress(Address.fromString(from)))
  );
  event.parameters.push(
    new ethereum.EventParam("from", ethereum.Value.fromAddress(Address.fromString(from)))
  );
  event.parameters.push(
    new ethereum.EventParam("to", ethereum.Value.fromAddress(Address.fromString(to)))
  );
  event.parameters.push(
    new ethereum.EventParam("ids", ethereum.Value.fromUnsignedBigIntArray(ids))
  );
  event.parameters.push(
    new ethereum.EventParam("values", ethereum.Value.fromUnsignedBigIntArray(values))
  );
  return event;
};

// Matches nameWrapper.ts::checkPccBurned's own constant.
const PARENT_CANNOT_CONTROL: i32 = 65536;

describe("handleFusesSet", () => {
  test("writes history but touches no Domain when no WrappedDomain exists yet", () => {
    const node =
      "0x1111111111111111111111111111111111111111111111111111111111111111";
    const event = createFusesSetEvent(node, 1);
    handleFusesSet(event);

    assert.notInStore("WrappedDomain", node);
    assert.notInStore("Domain", node);

    let eventId = createLegacyEventID(event);
    assert.fieldEquals("FusesSet", eventId, "domain", node);
    assert.fieldEquals("FusesSet", eventId, "fuses", "1");
  });

  test("bumps Domain.expiryDate to match once PARENT_CANNOT_CONTROL is burned", () => {
    const node =
      "0x2222222222222222222222222222222222222222222222222222222222222222";

    let domain = new Domain(node);
    domain.storedOffchain = false; // Blockscout/BENS extension
    domain.resolvedWithWildcard = false; // Blockscout/BENS extension
    domain.owner = DEFAULT_OWNER;
    domain.isMigrated = true;
    domain.subdomainCount = 0;
    domain.createdAt = BigInt.fromI32(0);
    domain.expiryDate = BigInt.fromI32(100);
    domain.save();

    let wrappedDomain = new WrappedDomain(node);
    wrappedDomain.domain = node;
    wrappedDomain.expiryDate = BigInt.fromI32(500);
    wrappedDomain.fuses = 0;
    wrappedDomain.owner = DEFAULT_OWNER;
    wrappedDomain.save();

    const event = createFusesSetEvent(node, PARENT_CANNOT_CONTROL);
    handleFusesSet(event);

    assert.fieldEquals("WrappedDomain", node, "fuses", PARENT_CANNOT_CONTROL.toString());
    // wrappedDomain.expiryDate (500) is later than the pre-seeded
    // domain.expiryDate (100), so once PCC is burned it takes over.
    assert.fieldEquals("Domain", node, "expiryDate", "500");
  });

  test("does not downgrade Domain.expiryDate when the wrapped expiry is earlier", () => {
    const node =
      "0x6666666666666666666666666666666666666666666666666666666666666666";

    let domain = new Domain(node);
    domain.storedOffchain = false; // Blockscout/BENS extension
    domain.resolvedWithWildcard = false; // Blockscout/BENS extension
    domain.owner = DEFAULT_OWNER;
    domain.isMigrated = true;
    domain.subdomainCount = 0;
    domain.createdAt = BigInt.fromI32(0);
    domain.expiryDate = BigInt.fromI32(99999);
    domain.save();

    let wrappedDomain = new WrappedDomain(node);
    wrappedDomain.domain = node;
    wrappedDomain.expiryDate = BigInt.fromI32(100);
    wrappedDomain.fuses = 0;
    wrappedDomain.owner = DEFAULT_OWNER;
    wrappedDomain.save();

    const event = createFusesSetEvent(node, PARENT_CANNOT_CONTROL);
    handleFusesSet(event);

    // fuses always updates unconditionally...
    assert.fieldEquals("WrappedDomain", node, "fuses", PARENT_CANNOT_CONTROL.toString());
    // ...but wrappedDomain.expiryDate (100) is NOT later than the existing
    // domain.expiryDate (99999), so the domain-side bump must be skipped.
    assert.fieldEquals("Domain", node, "expiryDate", "99999");
  });
});

describe("handleExpiryExtended", () => {
  test("writes history but touches no Domain when no WrappedDomain exists yet", () => {
    const node =
      "0x3333333333333333333333333333333333333333333333333333333333333333";
    const event = createExpiryExtendedEvent(node, BigInt.fromI32(999));
    handleExpiryExtended(event);

    assert.notInStore("WrappedDomain", node);
    assert.notInStore("Domain", node);

    let eventId = createLegacyEventID(event);
    assert.fieldEquals("ExpiryExtended", eventId, "domain", node);
    assert.fieldEquals("ExpiryExtended", eventId, "expiryDate", "999");
  });

  test("bumps Domain.expiryDate for an already-PCC-burned WrappedDomain", () => {
    const node =
      "0x4444444444444444444444444444444444444444444444444444444444444444";

    let domain = new Domain(node);
    domain.storedOffchain = false; // Blockscout/BENS extension
    domain.resolvedWithWildcard = false; // Blockscout/BENS extension
    domain.owner = DEFAULT_OWNER;
    domain.isMigrated = true;
    domain.subdomainCount = 0;
    domain.createdAt = BigInt.fromI32(0);
    domain.expiryDate = BigInt.fromI32(100);
    domain.save();

    let wrappedDomain = new WrappedDomain(node);
    wrappedDomain.domain = node;
    wrappedDomain.expiryDate = BigInt.fromI32(200);
    wrappedDomain.fuses = PARENT_CANNOT_CONTROL;
    wrappedDomain.owner = DEFAULT_OWNER;
    wrappedDomain.save();

    const event = createExpiryExtendedEvent(node, BigInt.fromI32(1000));
    handleExpiryExtended(event);

    assert.fieldEquals("WrappedDomain", node, "expiryDate", "1000");
    assert.fieldEquals("Domain", node, "expiryDate", "1000");
  });

  test("still updates WrappedDomain.expiryDate but does not downgrade Domain.expiryDate", () => {
    const node =
      "0x7777777777777777777777777777777777777777777777777777777777777777";

    let domain = new Domain(node);
    domain.storedOffchain = false; // Blockscout/BENS extension
    domain.resolvedWithWildcard = false; // Blockscout/BENS extension
    domain.owner = DEFAULT_OWNER;
    domain.isMigrated = true;
    domain.subdomainCount = 0;
    domain.createdAt = BigInt.fromI32(0);
    domain.expiryDate = BigInt.fromI32(99999);
    domain.save();

    let wrappedDomain = new WrappedDomain(node);
    wrappedDomain.domain = node;
    wrappedDomain.expiryDate = BigInt.fromI32(100);
    wrappedDomain.fuses = PARENT_CANNOT_CONTROL;
    wrappedDomain.owner = DEFAULT_OWNER;
    wrappedDomain.save();

    const event = createExpiryExtendedEvent(node, BigInt.fromI32(500));
    handleExpiryExtended(event);

    // wrappedDomain.expiryDate always updates unconditionally, regardless
    // of whether it would be a downgrade for the Domain side...
    assert.fieldEquals("WrappedDomain", node, "expiryDate", "500");
    // ...but 500 is NOT later than the existing domain.expiryDate (99999),
    // so the domain-side bump must be skipped.
    assert.fieldEquals("Domain", node, "expiryDate", "99999");
  });
});

const ZERO_ADDRESS = "0x0000000000000000000000000000000000000000";

describe("handleTransferSingle / handleTransferBatch", () => {
  test("handleTransferSingle of a detached token (name no longer wrapped) writes history only", () => {
    const tokenId = BigInt.fromI32(333444555);
    const node =
      "0x0000000000000000000000000000000000000000000000000000000013dff5cb";

    // The registry moved this name away from the NameWrapper earlier
    // (parent setSubnodeOwner / registrar reclaim), which cleared the
    // wrapped state. The ERC-1155 token still exists and can be transferred.
    let domain = new Domain(node);
    domain.storedOffchain = false; // Blockscout/BENS extension
    domain.resolvedWithWildcard = false; // Blockscout/BENS extension
    domain.owner = DEFAULT_OWNER;
    domain.isMigrated = true;
    domain.subdomainCount = 0;
    domain.createdAt = BigInt.fromI32(0);
    domain.save();

    const newOwner = "0xF0205A3A3b2A69De6Dbf7f01ED13B2108B2c4321";
    const event = createTransferSingleEvent(DEFAULT_OWNER, newOwner, tokenId);
    handleTransferSingle(event);

    assert.notInStore("WrappedDomain", node);
    // Never set on this row, so it is absent from the store rather than
    // "null": check through the entity.
    assert.assertNull(Domain.load(node)!.wrappedOwner);
    assert.fieldEquals("Domain", node, "owner", DEFAULT_OWNER);
    let eventId = createLegacyEventID(event).concat("-0");
    assert.fieldEquals(
      "WrappedTransfer",
      eventId,
      "owner",
      Address.fromString(newOwner).toHexString()
    );
  });

  test("handleTransferSingle creates a placeholder WrappedDomain, sets domain.wrappedOwner, and writes history", () => {
    const tokenId = BigInt.fromI32(111222333);
    const node =
      "0x0000000000000000000000000000000000000000000000000000000006a11e3d";

    let domain = new Domain(node);
    domain.storedOffchain = false; // Blockscout/BENS extension
    domain.resolvedWithWildcard = false; // Blockscout/BENS extension
    domain.owner = DEFAULT_OWNER;
    domain.isMigrated = true;
    domain.subdomainCount = 0;
    domain.createdAt = BigInt.fromI32(0);
    domain.save();

    const newOwner = "0xF0205A3A3b2A69De6Dbf7f01ED13B2108B2c4321";
    // A fresh wrap mints the token: from is the zero address.
    const event = createTransferSingleEvent(ZERO_ADDRESS, newOwner, tokenId);
    handleTransferSingle(event);

    assert.fieldEquals(
      "WrappedDomain",
      node,
      "owner",
      Address.fromString(newOwner).toHexString()
    );
    // Placeholder values until the real NameWrapped event arrives.
    assert.fieldEquals("WrappedDomain", node, "expiryDate", "0");
    assert.fieldEquals("WrappedDomain", node, "fuses", "0");
    assert.fieldEquals(
      "Domain",
      node,
      "wrappedOwner",
      Address.fromString(newOwner).toHexString()
    );

    let eventId = createLegacyEventID(event).concat("-0");
    assert.fieldEquals(
      "WrappedTransfer",
      eventId,
      "owner",
      Address.fromString(newOwner).toHexString()
    );
  });

  test("handleTransferSingle on an already-wrapped name updates owner but preserves fuses/expiryDate", () => {
    const tokenId = BigInt.fromI32(222333444);
    const node =
      "0x000000000000000000000000000000000000000000000000000000000d408a04";

    let domain = new Domain(node);
    domain.storedOffchain = false; // Blockscout/BENS extension
    domain.resolvedWithWildcard = false; // Blockscout/BENS extension
    domain.owner = DEFAULT_OWNER;
    domain.isMigrated = true;
    domain.subdomainCount = 0;
    domain.createdAt = BigInt.fromI32(0);
    domain.save();

    // Real wrapped state, not the placeholder zeros makeWrappedTransfer uses
    // for a fresh mint -- this is what a real, already-wrapped name looks
    // like by the time it gets transferred again.
    let wrappedDomain = new WrappedDomain(node);
    wrappedDomain.domain = node;
    wrappedDomain.expiryDate = BigInt.fromI32(999999);
    wrappedDomain.fuses = PARENT_CANNOT_CONTROL;
    wrappedDomain.owner = DEFAULT_OWNER;
    wrappedDomain.save();

    const newOwner = "0xF0205A3A3b2A69De6Dbf7f01ED13B2108B2c4321";
    const event = createTransferSingleEvent(DEFAULT_OWNER, newOwner, tokenId);
    handleTransferSingle(event);

    assert.fieldEquals(
      "WrappedDomain",
      node,
      "owner",
      Address.fromString(newOwner).toHexString()
    );
    // makeWrappedTransfer's update branch (wrappedDomain already existed)
    // only ever reassigns .owner -- fuses/expiryDate must survive untouched,
    // not get reset to the fresh-mint placeholder values.
    assert.fieldEquals("WrappedDomain", node, "expiryDate", "999999");
    assert.fieldEquals(
      "WrappedDomain",
      node,
      "fuses",
      PARENT_CANNOT_CONTROL.toString()
    );
    assert.fieldEquals(
      "Domain",
      node,
      "wrappedOwner",
      Address.fromString(newOwner).toHexString()
    );
  });

  test("handleTransferBatch updates every token in the batch and writes one history row per index", () => {
    const tokenIdA = BigInt.fromI32(444555666);
    const tokenIdB = BigInt.fromI32(777888999);
    const nodeA =
      "0x000000000000000000000000000000000000000000000000000000001a7f6192";
    const nodeB =
      "0x000000000000000000000000000000000000000000000000000000002e5da4e7";

    let domainA = new Domain(nodeA);
    domainA.storedOffchain = false; // Blockscout/BENS extension
    domainA.resolvedWithWildcard = false; // Blockscout/BENS extension
    domainA.owner = DEFAULT_OWNER;
    domainA.isMigrated = true;
    domainA.subdomainCount = 0;
    domainA.createdAt = BigInt.fromI32(0);
    domainA.save();

    let domainB = new Domain(nodeB);
    domainB.storedOffchain = false; // Blockscout/BENS extension
    domainB.resolvedWithWildcard = false; // Blockscout/BENS extension
    domainB.owner = DEFAULT_OWNER;
    domainB.isMigrated = true;
    domainB.subdomainCount = 0;
    domainB.createdAt = BigInt.fromI32(0);
    domainB.save();

    const newOwner = "0xF0205A3A3b2A69De6Dbf7f01ED13B2108B2c4321";
    const event = createTransferBatchEvent(ZERO_ADDRESS, newOwner, [
      tokenIdA,
      tokenIdB,
    ]);
    handleTransferBatch(event);

    assert.fieldEquals(
      "WrappedDomain",
      nodeA,
      "owner",
      Address.fromString(newOwner).toHexString()
    );
    assert.fieldEquals(
      "WrappedDomain",
      nodeB,
      "owner",
      Address.fromString(newOwner).toHexString()
    );

    let eventIdA = createLegacyEventID(event).concat("-0");
    let eventIdB = createLegacyEventID(event).concat("-1");
    assert.fieldEquals("WrappedTransfer", eventIdA, "domain", nodeA);
    assert.fieldEquals("WrappedTransfer", eventIdB, "domain", nodeB);
  });

  test("handleTransferBatch handles a fresh mint and an already-wrapped transfer in the same batch", () => {
    const freshTokenId = BigInt.fromI32(555666777);
    const existingTokenId = BigInt.fromI32(888999111);
    const freshNode =
      "0x00000000000000000000000000000000000000000000000000000000211ecd59";
    const existingNode =
      "0x0000000000000000000000000000000000000000000000000000000034fd0cc7";

    let freshDomain = new Domain(freshNode);
    freshDomain.storedOffchain = false; // Blockscout/BENS extension
    freshDomain.resolvedWithWildcard = false; // Blockscout/BENS extension
    freshDomain.owner = DEFAULT_OWNER;
    freshDomain.isMigrated = true;
    freshDomain.subdomainCount = 0;
    freshDomain.createdAt = BigInt.fromI32(0);
    freshDomain.save();

    let existingDomain = new Domain(existingNode);
    existingDomain.storedOffchain = false; // Blockscout/BENS extension
    existingDomain.resolvedWithWildcard = false; // Blockscout/BENS extension
    existingDomain.owner = DEFAULT_OWNER;
    existingDomain.isMigrated = true;
    existingDomain.subdomainCount = 0;
    existingDomain.createdAt = BigInt.fromI32(0);
    existingDomain.save();

    // Only the second token already has a real WrappedDomain -- the first
    // exercises the create-placeholder branch of makeWrappedTransfer in the
    // very same batch call as an update to an existing row, proving the
    // per-index branching inside handleTransferBatch's loop doesn't leak
    // state between iterations.
    let existingWrappedDomain = new WrappedDomain(existingNode);
    existingWrappedDomain.domain = existingNode;
    existingWrappedDomain.expiryDate = BigInt.fromI32(555555);
    existingWrappedDomain.fuses = PARENT_CANNOT_CONTROL;
    existingWrappedDomain.owner = DEFAULT_OWNER;
    existingWrappedDomain.save();

    const newOwner = "0xF0205A3A3b2A69De6Dbf7f01ED13B2108B2c4321";
    const event = createTransferBatchEvent(ZERO_ADDRESS, newOwner, [
      freshTokenId,
      existingTokenId,
    ]);
    handleTransferBatch(event);

    // Fresh mint: placeholder expiry/fuses.
    assert.fieldEquals(
      "WrappedDomain",
      freshNode,
      "owner",
      Address.fromString(newOwner).toHexString()
    );
    assert.fieldEquals("WrappedDomain", freshNode, "expiryDate", "0");
    assert.fieldEquals("WrappedDomain", freshNode, "fuses", "0");

    // Already-wrapped: owner updates, real fuses/expiryDate survive.
    assert.fieldEquals(
      "WrappedDomain",
      existingNode,
      "owner",
      Address.fromString(newOwner).toHexString()
    );
    assert.fieldEquals("WrappedDomain", existingNode, "expiryDate", "555555");
    assert.fieldEquals(
      "WrappedDomain",
      existingNode,
      "fuses",
      PARENT_CANNOT_CONTROL.toString()
    );
  });
});

describe("handleExpiryExtended with uint64 max (Graveyard-parked migrated names)", () => {
  test("clamps WrappedDomain/Domain expiryDate to the max supported timestamp and keeps the raw value on the event row", () => {
    const node =
      "0x5555555555555555555555555555555555555555555555555555555555555555";
    const UINT64_MAX = BigInt.fromString("18446744073709551615");

    let domain = new Domain(node);
    domain.storedOffchain = false; // Blockscout/BENS extension
    domain.resolvedWithWildcard = false; // Blockscout/BENS extension
    domain.owner = DEFAULT_OWNER;
    domain.isMigrated = true;
    domain.subdomainCount = 0;
    domain.createdAt = BigInt.fromI32(0);
    domain.expiryDate = BigInt.fromI32(100);
    domain.save();

    let wrappedDomain = new WrappedDomain(node);
    wrappedDomain.domain = node;
    wrappedDomain.expiryDate = BigInt.fromI32(200);
    wrappedDomain.fuses = PARENT_CANNOT_CONTROL;
    wrappedDomain.owner = DEFAULT_OWNER;
    wrappedDomain.save();

    const event = createExpiryExtendedEvent(node, UINT64_MAX);
    handleExpiryExtended(event);

    assert.fieldEquals("WrappedDomain", node, "expiryDate", "253402300799");
    assert.fieldEquals("Domain", node, "expiryDate", "253402300799");
    let eventId = createLegacyEventID(event);
    assert.fieldEquals("ExpiryExtended", eventId, "expiryDate", UINT64_MAX.toString());
  });
});
