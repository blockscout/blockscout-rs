import { Address, BigInt, Bytes, ethereum } from "@graphprotocol/graph-ts";
import {
  afterEach,
  assert,
  clearStore,
  dataSourceMock,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import {
  handleExpiryUpdated,
  handleLabelRegistered,
  handleLabelReserved,
  handleLabelUnregistered,
  handleParentUpdated,
  handleTokenRegenerated,
  handleTokenResource,
  handleTransferBatch,
  handleTransferSingle,
  handleURIUpdated,
} from "../src/ensv2Registry";
import {
  ExpiryUpdated,
  LabelRegistered,
  LabelReserved,
  LabelUnregistered,
  ParentUpdated,
  TokenRegenerated,
  TokenResource,
  TransferBatch,
  TransferSingle,
  URIUpdated,
} from "../src/types/RootRegistry/PermissionedRegistry";
import {
  ENSv2NameSlot,
  ENSv2Registry,
  ENSv2Resource,
  ENSv2Token,
} from "../src/types/schema";

const ROOT_REGISTRY = "0xB458D6a3a77919449d03e7A6903C26827c1eC43f";
const ETH_REGISTRY = "0xD4eBcbBdF463C9c45784603Db0dDD499BC44A8B4";
const OWNER = "0x89205A3A3b2A69De6Dbf7f01ED13B2108B2c43e7";
const SENDER = "0x11111111111111111111111111111111111111aa";
const ROOT_NAMEHASH =
  "0x0000000000000000000000000000000000000000000000000000000000000000";

// toSlotId(tokenId) zeroes the low 32 bits of tokenId (see
// src/ensv2Utils.ts), which collapses any tokenId under 2^32 to slot 0 —
// real labelhash-derived tokenIds are far larger, so tests below use a
// distinct registry address per scenario (rather than distinct tokenIds) to
// keep ENSv2NameSlot rows from colliding across independent test cases.
const REGISTRY_FRESH_REGISTRATION = "0x44444444444444444444444444444444444444dd";
const REGISTRY_RESERVATION = "0x55555555555555555555555555555555555555dd";
const REGISTRY_UNREGISTRATION = "0x66666666666666666666666666666666666666dd";
const REGISTRY_RENEWAL = "0x77777777777777777777777777777777777777dd";
const REGISTRY_REREGISTRATION = "0x88888888888888888888888888888888888888dd";
const REGISTRY_TOKEN_RESOURCE = "0x99999999999999999999999999999999999999ee";
const REGISTRY_MINT_BEFORE_RESOURCE = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaee";
const REGISTRY_REGENERATION = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbee";
const REGISTRY_TRANSFER_BATCH = "0xccccccccccccccccccccccccccccccccccccccee";
const REGISTRY_RESOURCE_CROSS_CHECK = "0xddddddddddddddddddddddddddddddddddddddee";
const OWNER_2 = "0xF0205A3A3b2A69De6Dbf7f01ED13B2108B2c4321";

const createLabelRegisteredEvent = (
  registryAddress: string,
  tokenId: BigInt,
  label: string
): LabelRegistered => {
  let mockEvent = newMockEvent();
  let event = new LabelRegistered(
    Address.fromString(registryAddress),
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
    new ethereum.EventParam(
      "tokenId",
      ethereum.Value.fromUnsignedBigInt(tokenId)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "labelHash",
      ethereum.Value.fromFixedBytes(Bytes.fromI32(1))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("label", ethereum.Value.fromString(label))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "owner",
      ethereum.Value.fromAddress(Address.fromString(OWNER))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "expiry",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2000000000))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "sender",
      ethereum.Value.fromAddress(Address.fromString(SENDER))
    )
  );
  return event;
};

const createLabelReservedEvent = (
  registryAddress: string,
  tokenId: BigInt,
  label: string
): LabelReserved => {
  let mockEvent = newMockEvent();
  let event = new LabelReserved(
    Address.fromString(registryAddress),
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
    new ethereum.EventParam(
      "tokenId",
      ethereum.Value.fromUnsignedBigInt(tokenId)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "labelHash",
      ethereum.Value.fromFixedBytes(Bytes.fromI32(1))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("label", ethereum.Value.fromString(label))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "expiry",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2000000000))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "sender",
      ethereum.Value.fromAddress(Address.fromString(SENDER))
    )
  );
  return event;
};

const createLabelUnregisteredEvent = (
  registryAddress: string,
  tokenId: BigInt
): LabelUnregistered => {
  let mockEvent = newMockEvent();
  let event = new LabelUnregistered(
    Address.fromString(registryAddress),
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
    new ethereum.EventParam(
      "tokenId",
      ethereum.Value.fromUnsignedBigInt(tokenId)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "sender",
      ethereum.Value.fromAddress(Address.fromString(SENDER))
    )
  );
  return event;
};

const createExpiryUpdatedEvent = (
  registryAddress: string,
  tokenId: BigInt,
  newExpiry: BigInt
): ExpiryUpdated => {
  let mockEvent = newMockEvent();
  let event = new ExpiryUpdated(
    Address.fromString(registryAddress),
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
    new ethereum.EventParam(
      "tokenId",
      ethereum.Value.fromUnsignedBigInt(tokenId)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "newExpiry",
      ethereum.Value.fromUnsignedBigInt(newExpiry)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "sender",
      ethereum.Value.fromAddress(Address.fromString(SENDER))
    )
  );
  return event;
};

const createTokenResourceEvent = (
  registryAddress: string,
  tokenId: BigInt,
  resource: BigInt
): TokenResource => {
  let mockEvent = newMockEvent();
  let event = new TokenResource(
    Address.fromString(registryAddress),
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
    new ethereum.EventParam(
      "tokenId",
      ethereum.Value.fromUnsignedBigInt(tokenId)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "resource",
      ethereum.Value.fromUnsignedBigInt(resource)
    )
  );
  return event;
};

const createTokenRegeneratedEvent = (
  registryAddress: string,
  oldTokenId: BigInt,
  newTokenId: BigInt
): TokenRegenerated => {
  let mockEvent = newMockEvent();
  let event = new TokenRegenerated(
    Address.fromString(registryAddress),
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
    new ethereum.EventParam(
      "oldTokenId",
      ethereum.Value.fromUnsignedBigInt(oldTokenId)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "newTokenId",
      ethereum.Value.fromUnsignedBigInt(newTokenId)
    )
  );
  return event;
};

const createTransferSingleEvent = (
  registryAddress: string,
  tokenId: BigInt,
  from: string,
  to: string
): TransferSingle => {
  let mockEvent = newMockEvent();
  let event = new TransferSingle(
    Address.fromString(registryAddress),
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
    new ethereum.EventParam(
      "operator",
      ethereum.Value.fromAddress(Address.fromString(from))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "from",
      ethereum.Value.fromAddress(Address.fromString(from))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "to",
      ethereum.Value.fromAddress(Address.fromString(to))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "id",
      ethereum.Value.fromUnsignedBigInt(tokenId)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "value",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(1))
    )
  );
  return event;
};

const createTransferBatchEvent = (
  registryAddress: string,
  tokenIds: Array<BigInt>,
  from: string,
  to: string
): TransferBatch => {
  let mockEvent = newMockEvent();
  let event = new TransferBatch(
    Address.fromString(registryAddress),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );

  let valueOnes = new Array<BigInt>();
  for (let i = 0; i < tokenIds.length; i++) {
    valueOnes.push(BigInt.fromI32(1));
  }

  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "operator",
      ethereum.Value.fromAddress(Address.fromString(from))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "from",
      ethereum.Value.fromAddress(Address.fromString(from))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "to",
      ethereum.Value.fromAddress(Address.fromString(to))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "ids",
      ethereum.Value.fromUnsignedBigIntArray(tokenIds)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "values",
      ethereum.Value.fromUnsignedBigIntArray(valueOnes)
    )
  );
  return event;
};

afterEach(() => {
  dataSourceMock.resetValues();
  clearStore();
});

// assert.fieldEquals compares an entity's id as its lowercase-hex string
// form regardless of the underlying GraphQL type.
// Production code now builds these ids as fixed-width Bytes concatenation
// with no delimiter: addresses are 20 bytes, a BigInt component is a
// 32-byte big-endian value (src/utils.ts::uint256ToByteArray), a small loop
// index is a 4-byte big-endian value (src/utils.ts::i32ToBytes) — these
// mirror that exact encoding to reproduce the same hex strings.
function bigIntHex32(i: BigInt): string {
  return i.toHex().slice(2).padStart(64, "0");
}
function i32Hex4(i: i32): string {
  return bigIntHex32(BigInt.fromI32(i)).slice(56);
}

test("RootRegistry event bootstraps ENSv2Registry(kind=ROOT) and the root ENSv2Namespace", () => {
  dataSourceMock.setNetwork("sepolia");

  let event = createLabelRegisteredEvent(
    ROOT_REGISTRY,
    BigInt.fromI32(1),
    "eth"
  );
  handleLabelRegistered(event);

  let registryId = Address.fromString(ROOT_REGISTRY).toHexString();
  assert.fieldEquals("ENSv2Registry", registryId, "kind", "ROOT");
  assert.fieldEquals("ENSv2Registry", registryId, "namespaceCount", "1");

  let namespaceId = registryId.concat(ROOT_NAMEHASH.slice(2));
  // baseName is deliberately never assigned for root (see
  // src/ensv2Discovery.ts::getOrCreateRootNamespace) — the generated
  // nullable-String setter treats "" as falsy and unsets the field anyway,
  // so leaving it untouched is equivalent and matchstick has no stored
  // field to assert on either way.
  assert.fieldEquals(
    "ENSv2Namespace",
    namespaceId,
    "baseNamehash",
    ROOT_NAMEHASH
  );
  assert.fieldEquals("ENSv2Namespace", namespaceId, "registry", registryId);
});

test("ETHRegistry event bootstraps ENSv2Registry(kind=ETH) without a root namespace", () => {
  dataSourceMock.setNetwork("sepolia");

  let event = createLabelRegisteredEvent(
    ETH_REGISTRY,
    BigInt.fromI32(1),
    "vitalik"
  );
  handleLabelRegistered(event);

  let registryId = Address.fromString(ETH_REGISTRY).toHexString();
  assert.fieldEquals("ENSv2Registry", registryId, "kind", "ETH");
  assert.fieldEquals("ENSv2Registry", registryId, "namespaceCount", "0");

  let namespaceId = registryId.concat(ROOT_NAMEHASH.slice(2));
  assert.notInStore("ENSv2Namespace", namespaceId);
});

test("an address that isn't RootRegistry or ETHRegistry bootstraps kind=UNKNOWN", () => {
  dataSourceMock.setNetwork("sepolia");

  const OTHER_REGISTRY = "0x33333333333333333333333333333333333333cc";
  let event = createLabelRegisteredEvent(
    OTHER_REGISTRY,
    BigInt.fromI32(1),
    "sub"
  );
  handleLabelRegistered(event);

  let registryId = Address.fromString(OTHER_REGISTRY).toHexString();
  assert.fieldEquals("ENSv2Registry", registryId, "kind", "UNKNOWN");
});

test("fresh registration populates ENSv2NameSlot and creates ENSv2LabelRegistered history", () => {
  dataSourceMock.setNetwork("sepolia");

  let registryId = Address.fromString(REGISTRY_FRESH_REGISTRATION).toHexString();
  let event = createLabelRegisteredEvent(
    REGISTRY_FRESH_REGISTRATION,
    BigInt.fromI32(1),
    "alice"
  );
  handleLabelRegistered(event);

  let slotId = registryId.concat(bigIntHex32(BigInt.zero()));
  assert.fieldEquals("ENSv2NameSlot", slotId, "status", "REGISTERED");
  assert.fieldEquals("ENSv2NameSlot", slotId, "label", "alice");
  assert.fieldEquals("ENSv2NameSlot", slotId, "expiryDate", "2000000000");
  let ownerId = Address.fromString(OWNER).toHexString();
  assert.fieldEquals("ENSv2NameSlot", slotId, "owner", ownerId);
  assert.fieldEquals("ENSv2NameSlot", slotId, "registrant", ownerId);
  assert.fieldEquals("ENSv2NameSlot", slotId, "migratedFromV1", "false");

  let historyId = "0x".concat(bigIntHex32(event.block.number)).concat(bigIntHex32(event.logIndex));
  assert.fieldEquals("ENSv2LabelRegistered", historyId, "slot", slotId);
  assert.fieldEquals("ENSv2LabelRegistered", historyId, "isReRegistration", "false");
  assert.fieldEquals(
    "ENSv2LabelRegistered",
    historyId,
    "sender",
    Address.fromString(SENDER).toHexString()
  );
});

test("reservation sets status RESERVED and creates no history entity", () => {
  dataSourceMock.setNetwork("sepolia");

  let registryId = Address.fromString(REGISTRY_RESERVATION).toHexString();
  let event = createLabelReservedEvent(
    REGISTRY_RESERVATION,
    BigInt.fromI32(1),
    "reserved-name"
  );
  handleLabelReserved(event);

  let slotId = registryId.concat(bigIntHex32(BigInt.zero()));
  assert.fieldEquals("ENSv2NameSlot", slotId, "status", "RESERVED");
  assert.fieldEquals("ENSv2NameSlot", slotId, "expiryDate", "2000000000");

  let historyId = "0x".concat(bigIntHex32(event.block.number)).concat(bigIntHex32(event.logIndex));
  assert.notInStore("ENSv2LabelRegistered", historyId);
});

test("LabelUnregistered followed by burn clears active native ownership and incarnation state", () => {
  dataSourceMock.setNetwork("sepolia");

  let registryId = Address.fromString(REGISTRY_UNREGISTRATION).toHexString();
  let tokenId = BigInt.fromI32(1);
  let registerEvent = createLabelRegisteredEvent(
    REGISTRY_UNREGISTRATION,
    tokenId,
    "bob"
  );
  handleLabelRegistered(registerEvent);
  let mintEvent = createTransferSingleEvent(
    REGISTRY_UNREGISTRATION,
    tokenId,
    "0x0000000000000000000000000000000000000000",
    OWNER
  );
  handleTransferSingle(mintEvent);
  let resource = BigInt.fromI32(55);
  handleTokenResource(
    createTokenResourceEvent(REGISTRY_UNREGISTRATION, tokenId, resource)
  );

  let unregisterEvent = createLabelUnregisteredEvent(
    REGISTRY_UNREGISTRATION,
    tokenId
  );
  handleLabelUnregistered(unregisterEvent);
  handleTransferSingle(
    createTransferSingleEvent(
      REGISTRY_UNREGISTRATION,
      tokenId,
      OWNER,
      "0x0000000000000000000000000000000000000000"
    )
  );

  let slotId = registryId.concat(bigIntHex32(BigInt.zero()));
  assert.fieldEquals("ENSv2NameSlot", slotId, "status", "AVAILABLE");
  // Stable identity remains, active ownership/incarnation does not.
  assert.fieldEquals("ENSv2NameSlot", slotId, "label", "bob");
  let slot = ENSv2NameSlot.load(Bytes.fromHexString(slotId));
  let slotHasActiveState = true;
  if (slot != null) {
    slotHasActiveState =
      !!slot.owner ||
      !!slot.registrant ||
      !!slot.currentToken ||
      !!slot.currentResource;
  }
  assert.assertTrue(slot != null && !slotHasActiveState);

  let tokenIdHex = registryId.concat(bigIntHex32(tokenId));
  let token = ENSv2Token.load(Bytes.fromHexString(tokenIdHex));
  let tokenStillOwned = true;
  if (token != null) {
    tokenStillOwned = token.active || !!token.owner;
  }
  assert.assertTrue(token != null && !tokenStillOwned);
  let resourceEntityId = registryId.concat(bigIntHex32(resource));
  assert.fieldEquals("ENSv2Resource", resourceEntityId, "active", "false");

  let historyId = "0x".concat(bigIntHex32(unregisterEvent.block.number)).concat(bigIntHex32(unregisterEvent.logIndex));
  assert.fieldEquals("ENSv2LabelUnregistered", historyId, "slot", slotId);
});

test("renewal (ExpiryUpdated) updates expiryDate and creates ENSv2LabelRenewed", () => {
  dataSourceMock.setNetwork("sepolia");

  let registryId = Address.fromString(REGISTRY_RENEWAL).toHexString();
  let tokenId = BigInt.fromI32(1);
  let registerEvent = createLabelRegisteredEvent(
    REGISTRY_RENEWAL,
    tokenId,
    "carol"
  );
  handleLabelRegistered(registerEvent);

  let newExpiry = BigInt.fromI32(2100000000);
  let renewEvent = createExpiryUpdatedEvent(REGISTRY_RENEWAL, tokenId, newExpiry);
  handleExpiryUpdated(renewEvent);

  let slotId = registryId.concat(bigIntHex32(BigInt.zero()));
  assert.fieldEquals("ENSv2NameSlot", slotId, "expiryDate", "2100000000");

  let historyId = "0x".concat(bigIntHex32(renewEvent.block.number)).concat(bigIntHex32(renewEvent.logIndex));
  assert.fieldEquals("ENSv2LabelRenewed", historyId, "slot", slotId);
  assert.fieldEquals("ENSv2LabelRenewed", historyId, "newExpiryDate", "2100000000");
});

test("re-registration after unregistration reuses the same slot and sets isReRegistration=true", () => {
  dataSourceMock.setNetwork("sepolia");

  let registryId = Address.fromString(REGISTRY_REREGISTRATION).toHexString();
  let tokenId = BigInt.fromI32(1);

  handleLabelRegistered(
    createLabelRegisteredEvent(REGISTRY_REREGISTRATION, tokenId, "dave")
  );
  handleLabelUnregistered(
    createLabelUnregisteredEvent(REGISTRY_REREGISTRATION, tokenId)
  );
  let secondRegisterEvent = createLabelRegisteredEvent(
    REGISTRY_REREGISTRATION,
    tokenId,
    "dave-again"
  );
  handleLabelRegistered(secondRegisterEvent);

  // Same ID is reused by construction (load-or-create on a deterministic
  // id) — proving the second event's fields landed on that same row is
  // enough to show reuse, no separate collision-prone count check needed.
  let slotId = registryId.concat(bigIntHex32(BigInt.zero()));
  assert.fieldEquals("ENSv2NameSlot", slotId, "status", "REGISTERED");
  assert.fieldEquals("ENSv2NameSlot", slotId, "label", "dave-again");

  let historyId = "0x".concat(bigIntHex32(secondRegisterEvent.block.number)).concat(bigIntHex32(secondRegisterEvent.logIndex));
  assert.fieldEquals("ENSv2LabelRegistered", historyId, "isReRegistration", "true");
});

test("expired-name replacement burn clears the old incarnation before LabelRegistered restores ownership", () => {
  dataSourceMock.setNetwork("sepolia");

  let registryId = Address.fromString(REGISTRY_REREGISTRATION).toHexString();
  let oldTokenId = BigInt.fromI32(1);
  handleLabelRegistered(
    createLabelRegisteredEvent(REGISTRY_REREGISTRATION, oldTokenId, "expired")
  );
  handleTransferSingle(
    createTransferSingleEvent(
      REGISTRY_REREGISTRATION,
      oldTokenId,
      "0x0000000000000000000000000000000000000000",
      OWNER
    )
  );
  handleTokenResource(
    createTokenResourceEvent(
      REGISTRY_REREGISTRATION,
      oldTokenId,
      BigInt.fromI32(88)
    )
  );

  let burn = createTransferSingleEvent(
    REGISTRY_REREGISTRATION,
    oldTokenId,
    OWNER,
    "0x0000000000000000000000000000000000000000"
  );
  burn.block.timestamp = BigInt.fromI32(2000000000);
  handleTransferSingle(burn);

  let slotId = registryId.concat(bigIntHex32(BigInt.zero()));
  let afterBurn = ENSv2NameSlot.load(Bytes.fromHexString(slotId));
  let hasOwnerAfterBurn = false;
  if (afterBurn != null && afterBurn.owner) {
    hasOwnerAfterBurn = true;
  }
  assert.assertTrue(!hasOwnerAfterBurn);
  assert.fieldEquals("ENSv2NameSlot", slotId, "status", "AVAILABLE");

  // The new token version shares the same stable slot id.
  handleLabelRegistered(
    createLabelRegisteredEvent(
      REGISTRY_REREGISTRATION,
      BigInt.fromI32(2),
      "replacement"
    )
  );
  assert.fieldEquals("ENSv2NameSlot", slotId, "status", "REGISTERED");
  assert.fieldEquals(
    "ENSv2NameSlot",
    slotId,
    "owner",
    Address.fromString(OWNER).toHexString()
  );
});

test("TokenResource after registration links slot/resource/token", () => {
  dataSourceMock.setNetwork("sepolia");

  let registryId = Address.fromString(REGISTRY_TOKEN_RESOURCE).toHexString();
  let tokenId = BigInt.fromI32(1);
  handleLabelRegistered(
    createLabelRegisteredEvent(REGISTRY_TOKEN_RESOURCE, tokenId, "erin")
  );

  let resource = BigInt.fromI32(555);
  handleTokenResource(
    createTokenResourceEvent(REGISTRY_TOKEN_RESOURCE, tokenId, resource)
  );

  let slotId = registryId.concat(bigIntHex32(BigInt.zero()));
  let resourceEntityId = registryId.concat(bigIntHex32(resource));
  let tokenEntityId = registryId.concat(bigIntHex32(tokenId));

  assert.fieldEquals("ENSv2NameSlot", slotId, "currentResource", resourceEntityId);
  assert.fieldEquals("ENSv2NameSlot", slotId, "currentToken", tokenEntityId);
  assert.fieldEquals("ENSv2Resource", resourceEntityId, "slot", slotId);
  assert.fieldEquals("ENSv2Resource", resourceEntityId, "currentToken", tokenEntityId);
  assert.fieldEquals("ENSv2Token", tokenEntityId, "resourceEntity", resourceEntityId);
  assert.fieldEquals("ENSv2Token", tokenEntityId, "slot", slotId);
});

test("mint before TokenResource reconciles: token row exists without history, then resolves", () => {
  dataSourceMock.setNetwork("sepolia");

  let registryId = Address.fromString(REGISTRY_MINT_BEFORE_RESOURCE).toHexString();
  let tokenId = BigInt.fromI32(1);
  let tokenEntityId = registryId.concat(bigIntHex32(tokenId));

  // No LabelRegistered/TokenResource yet — a fresh mint's TransferSingle
  // arrives first.
  let mintEvent = createTransferSingleEvent(
    REGISTRY_MINT_BEFORE_RESOURCE,
    tokenId,
    "0x0000000000000000000000000000000000000000",
    OWNER
  );
  handleTransferSingle(mintEvent);

  assert.fieldEquals(
    "ENSv2Token",
    tokenEntityId,
    "owner",
    Address.fromString(OWNER).toHexString()
  );
  let mintHistoryId = "0x".concat(bigIntHex32(mintEvent.block.number)).concat(bigIntHex32(mintEvent.logIndex)).concat(i32Hex4(0));
  assert.notInStore("ENSv2TokenTransferred", mintHistoryId);

  // Registration + TokenResource now resolve the slot.
  handleLabelRegistered(
    createLabelRegisteredEvent(REGISTRY_MINT_BEFORE_RESOURCE, tokenId, "frank")
  );
  handleTokenResource(
    createTokenResourceEvent(
      REGISTRY_MINT_BEFORE_RESOURCE,
      tokenId,
      BigInt.fromI32(777)
    )
  );

  let slotId = registryId.concat(bigIntHex32(BigInt.zero()));
  assert.fieldEquals("ENSv2Token", tokenEntityId, "slot", slotId);

  // A subsequent transfer now gets a proper history row.
  let secondTransfer = createTransferSingleEvent(
    REGISTRY_MINT_BEFORE_RESOURCE,
    tokenId,
    OWNER,
    OWNER_2
  );
  handleTransferSingle(secondTransfer);

  let secondHistoryId = "0x".concat(bigIntHex32(secondTransfer.block.number)).concat(bigIntHex32(secondTransfer.logIndex)).concat(i32Hex4(0));
  assert.fieldEquals("ENSv2TokenTransferred", secondHistoryId, "slot", slotId);
  assert.fieldEquals(
    "ENSv2NameSlot",
    slotId,
    "owner",
    Address.fromString(OWNER_2).toHexString()
  );
});

test("regeneration clones slot/resource onto the new token and deactivates the old one", () => {
  dataSourceMock.setNetwork("sepolia");

  let registryId = Address.fromString(REGISTRY_REGENERATION).toHexString();
  let oldTokenId = BigInt.fromI32(1);
  let newTokenId = BigInt.fromI32(2);

  handleLabelRegistered(
    createLabelRegisteredEvent(REGISTRY_REGENERATION, oldTokenId, "grace")
  );
  handleTransferSingle(
    createTransferSingleEvent(
      REGISTRY_REGENERATION,
      oldTokenId,
      "0x0000000000000000000000000000000000000000",
      OWNER
    )
  );
  handleTokenResource(
    createTokenResourceEvent(
      REGISTRY_REGENERATION,
      oldTokenId,
      BigInt.fromI32(888)
    )
  );

  // Exact contract order: the old ERC-1155 is burned, TokenRegenerated
  // identifies its replacement, then the replacement is minted.
  handleTransferSingle(
    createTransferSingleEvent(
      REGISTRY_REGENERATION,
      oldTokenId,
      OWNER,
      "0x0000000000000000000000000000000000000000"
    )
  );

  let regenEvent = createTokenRegeneratedEvent(
    REGISTRY_REGENERATION,
    oldTokenId,
    newTokenId
  );
  handleTokenRegenerated(regenEvent);
  handleTransferSingle(
    createTransferSingleEvent(
      REGISTRY_REGENERATION,
      newTokenId,
      "0x0000000000000000000000000000000000000000",
      OWNER
    )
  );

  let slotId = registryId.concat(bigIntHex32(BigInt.zero()));
  let oldTokenEntityId = registryId.concat(bigIntHex32(oldTokenId));
  let newTokenEntityId = registryId.concat(bigIntHex32(newTokenId));
  let resourceEntityId = registryId.concat(bigIntHex32(BigInt.fromI32(888)));

  // Cloned onto the new token with zero TokenResource calls for it.
  assert.fieldEquals("ENSv2Token", newTokenEntityId, "slot", slotId);
  assert.fieldEquals("ENSv2Token", newTokenEntityId, "resourceEntity", resourceEntityId);
  assert.fieldEquals("ENSv2Token", newTokenEntityId, "active", "true");
  assert.fieldEquals(
    "ENSv2Token",
    newTokenEntityId,
    "owner",
    Address.fromString(OWNER).toHexString()
  );
  assert.fieldEquals("ENSv2Token", oldTokenEntityId, "active", "false");
  let oldToken = ENSv2Token.load(Bytes.fromHexString(oldTokenEntityId));
  let oldTokenHasOwner = false;
  if (oldToken != null && oldToken.owner) {
    oldTokenHasOwner = true;
  }
  assert.assertTrue(!oldTokenHasOwner);
  assert.fieldEquals("ENSv2NameSlot", slotId, "currentToken", newTokenEntityId);
  assert.fieldEquals("ENSv2Resource", resourceEntityId, "currentToken", newTokenEntityId);

  let historyId = "0x".concat(bigIntHex32(regenEvent.block.number)).concat(bigIntHex32(regenEvent.logIndex));
  assert.fieldEquals("ENSv2TokenRegenerated", historyId, "oldTokenId", oldTokenId.toString());
  assert.fieldEquals("ENSv2TokenRegenerated", historyId, "newTokenId", newTokenId.toString());
});

test("TransferBatch updates all token rows and writes one history row per id", () => {
  dataSourceMock.setNetwork("sepolia");

  let registryId = Address.fromString(REGISTRY_TRANSFER_BATCH).toHexString();
  // Distinct low-32-bit values so each id maps to a distinct slot within
  // this one registry (unlike the other tests, this scenario needs several
  // slots at once).
  let tokenIdA = BigInt.fromI32(101);
  let tokenIdB = BigInt.fromI32(202);

  handleLabelRegistered(
    createLabelRegisteredEvent(REGISTRY_TRANSFER_BATCH, tokenIdA, "henry")
  );
  handleTokenResource(
    createTokenResourceEvent(REGISTRY_TRANSFER_BATCH, tokenIdA, BigInt.fromI32(1001))
  );
  handleLabelRegistered(
    createLabelRegisteredEvent(REGISTRY_TRANSFER_BATCH, tokenIdB, "iris")
  );
  handleTokenResource(
    createTokenResourceEvent(REGISTRY_TRANSFER_BATCH, tokenIdB, BigInt.fromI32(1002))
  );

  let batchEvent = createTransferBatchEvent(
    REGISTRY_TRANSFER_BATCH,
    [tokenIdA, tokenIdB],
    OWNER,
    OWNER_2
  );
  handleTransferBatch(batchEvent);

  let tokenEntityIdA = registryId.concat(bigIntHex32(tokenIdA));
  let tokenEntityIdB = registryId.concat(bigIntHex32(tokenIdB));
  assert.fieldEquals(
    "ENSv2Token",
    tokenEntityIdA,
    "owner",
    Address.fromString(OWNER_2).toHexString()
  );
  assert.fieldEquals(
    "ENSv2Token",
    tokenEntityIdB,
    "owner",
    Address.fromString(OWNER_2).toHexString()
  );

  let baseHistoryId = "0x".concat(bigIntHex32(batchEvent.block.number)).concat(bigIntHex32(batchEvent.logIndex));
  assert.fieldEquals("ENSv2TokenTransferred", baseHistoryId.concat(i32Hex4(0)), "tokenId", tokenIdA.toString());
  assert.fieldEquals("ENSv2TokenTransferred", baseHistoryId.concat(i32Hex4(1)), "tokenId", tokenIdB.toString());
});

test("unregister then re-register creates a new resource and leaves the old one inactive", () => {
  dataSourceMock.setNetwork("sepolia");

  let registryId = Address.fromString(REGISTRY_RESOURCE_CROSS_CHECK).toHexString();
  let tokenId = BigInt.fromI32(1);

  handleLabelRegistered(
    createLabelRegisteredEvent(REGISTRY_RESOURCE_CROSS_CHECK, tokenId, "jack")
  );
  let resourceA = BigInt.fromI32(1);
  handleTokenResource(
    createTokenResourceEvent(REGISTRY_RESOURCE_CROSS_CHECK, tokenId, resourceA)
  );

  handleLabelUnregistered(
    createLabelUnregisteredEvent(REGISTRY_RESOURCE_CROSS_CHECK, tokenId)
  );
  handleLabelRegistered(
    createLabelRegisteredEvent(REGISTRY_RESOURCE_CROSS_CHECK, tokenId, "jack-again")
  );
  let resourceB = BigInt.fromI32(2);
  handleTokenResource(
    createTokenResourceEvent(REGISTRY_RESOURCE_CROSS_CHECK, tokenId, resourceB)
  );

  let slotId = registryId.concat(bigIntHex32(BigInt.zero()));
  let resourceEntityIdA = registryId.concat(bigIntHex32(resourceA));
  let resourceEntityIdB = registryId.concat(bigIntHex32(resourceB));

  assert.fieldEquals("ENSv2NameSlot", slotId, "currentResource", resourceEntityIdB);
  // Old resource still exists (never deleted) but is superseded.
  assert.fieldEquals("ENSv2Resource", resourceEntityIdA, "active", "false");
  let oldResource = ENSv2Resource.load(Bytes.fromHexString(resourceEntityIdA));
  let hasEndedAt = false;
  if (oldResource != null) {
    if (oldResource.endedAt) {
      hasEndedAt = true;
    }
  }
  assert.assertTrue(hasEndedAt);
});

const createParentUpdatedEvent = (
  registryAddress: string,
  parent: string,
  label: string,
  sender: string
): ParentUpdated => {
  let mockEvent = newMockEvent();
  let event = new ParentUpdated(
    Address.fromString(registryAddress),
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
    new ethereum.EventParam("parent", ethereum.Value.fromAddress(Address.fromString(parent)))
  );
  event.parameters.push(
    new ethereum.EventParam("label", ethereum.Value.fromString(label))
  );
  event.parameters.push(
    new ethereum.EventParam("sender", ethereum.Value.fromAddress(Address.fromString(sender)))
  );
  return event;
};

test("handleParentUpdated sets canonicalParentRegistry/Label, then clears both when parent goes to address(0)", () => {
  dataSourceMock.setNetwork("sepolia");

  const registryAddress = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
  const parentAddress = "0xffffffffffffffffffffffffffffffffffffffff";
  const zeroAddress = "0x0000000000000000000000000000000000000000";

  const setEvent = createParentUpdatedEvent(
    registryAddress,
    parentAddress,
    "parentlabel",
    SENDER
  );
  handleParentUpdated(setEvent);

  assert.fieldEquals(
    "ENSv2Registry",
    registryAddress,
    "canonicalParentRegistry",
    Address.fromString(parentAddress).toHexString()
  );
  assert.fieldEquals(
    "ENSv2Registry",
    registryAddress,
    "canonicalParentLabel",
    "parentlabel"
  );

  // Direct non-zero -> non-zero transition (parent changes from one real
  // registry straight to another, never touching zero in between) -- both
  // fields must move to the new parent's values, not just tolerate a
  // clear-then-set sequence.
  const secondParentAddress = "0xdddddddddddddddddddddddddddddddddddddddd";
  const secondSetEvent = createParentUpdatedEvent(
    registryAddress,
    secondParentAddress,
    "secondlabel",
    SENDER
  );
  handleParentUpdated(secondSetEvent);

  assert.fieldEquals(
    "ENSv2Registry",
    registryAddress,
    "canonicalParentRegistry",
    Address.fromString(secondParentAddress).toHexString()
  );
  assert.fieldEquals(
    "ENSv2Registry",
    registryAddress,
    "canonicalParentLabel",
    "secondlabel"
  );

  // Invalid label while SETTING a real (non-zero) parent -- the parent
  // itself must still update (label validity has nothing to do with
  // whether the parent registry link is real), but the label must be
  // cleared to null rather than either the malformed string or the STALE
  // previous label sitting next to a new parent.
  const thirdParentAddress = "0xccccccccccccccccccccccccccccccccccccccc0";
  const invalidLabelEvent = createParentUpdatedEvent(
    registryAddress,
    thirdParentAddress,
    "invalid.label",
    SENDER
  );
  handleParentUpdated(invalidLabelEvent);

  assert.fieldEquals(
    "ENSv2Registry",
    registryAddress,
    "canonicalParentRegistry",
    Address.fromString(thirdParentAddress).toHexString()
  );
  let hasStaleOrInvalidLabel = false;
  let registryAfterInvalidLabel = ENSv2Registry.load(
    Address.fromString(registryAddress)
  );
  if (registryAfterInvalidLabel != null) {
    if (registryAfterInvalidLabel.canonicalParentLabel) {
      hasStaleOrInvalidLabel = true;
    }
  }
  assert.assertTrue(!hasStaleOrInvalidLabel);

  const clearEvent = createParentUpdatedEvent(
    registryAddress,
    zeroAddress,
    "",
    SENDER
  );
  handleParentUpdated(clearEvent);

  let hasParentRegistry = false;
  let hasParentLabel = false;
  let registry = ENSv2Registry.load(Address.fromString(registryAddress));
  if (registry != null) {
    if (registry.canonicalParentRegistry) {
      hasParentRegistry = true;
    }
    if (registry.canonicalParentLabel) {
      hasParentLabel = true;
    }
  }
  assert.assertTrue(!hasParentRegistry);
  assert.assertTrue(!hasParentLabel);
});

const createURIUpdatedEvent = (
  registryAddress: string,
  uri: string,
  renderer: string,
  sender: string
): URIUpdated => {
  let mockEvent = newMockEvent();
  let event = new URIUpdated(
    Address.fromString(registryAddress),
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
    new ethereum.EventParam("uri", ethereum.Value.fromString(uri))
  );
  event.parameters.push(
    new ethereum.EventParam("renderer", ethereum.Value.fromAddress(Address.fromString(renderer)))
  );
  event.parameters.push(
    new ethereum.EventParam("sender", ethereum.Value.fromAddress(Address.fromString(sender)))
  );
  return event;
};

test("handleURIUpdated sets uri/uriRenderer on the registry, and a later call overwrites both", () => {
  dataSourceMock.setNetwork("sepolia");

  const registryAddress = "0xdddddddddddddddddddddddddddddddddddddddd";
  const rendererA = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
  const rendererB = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

  const firstEvent = createURIUpdatedEvent(
    registryAddress,
    "ipfs://first",
    rendererA,
    SENDER
  );
  handleURIUpdated(firstEvent);

  assert.fieldEquals("ENSv2Registry", registryAddress, "uri", "ipfs://first");
  assert.fieldEquals(
    "ENSv2Registry",
    registryAddress,
    "uriRenderer",
    Address.fromString(rendererA).toHexString()
  );

  const secondEvent = createURIUpdatedEvent(
    registryAddress,
    "ipfs://second",
    rendererB,
    SENDER
  );
  handleURIUpdated(secondEvent);

  assert.fieldEquals("ENSv2Registry", registryAddress, "uri", "ipfs://second");
  assert.fieldEquals(
    "ENSv2Registry",
    registryAddress,
    "uriRenderer",
    Address.fromString(rendererB).toHexString()
  );
});

test("WrapperRegistry initialization accepts zero-sender parent and URI events", () => {
  dataSourceMock.setNetwork("sepolia");

  const registryAddress = "0xdddddddddddddddddddddddddddddddddddddd01";
  const parentAddress = "0xdddddddddddddddddddddddddddddddddddddd02";
  const renderer = "0x0f5B101b6Fc626b9b210bB5E70f60F3dd9cA0d96";
  const zeroAddress = "0x0000000000000000000000000000000000000000";

  handleParentUpdated(
    createParentUpdatedEvent(
      registryAddress,
      parentAddress,
      "wrapped-label",
      zeroAddress
    )
  );
  handleURIUpdated(
    createURIUpdatedEvent(registryAddress, "", renderer, zeroAddress)
  );

  assert.fieldEquals(
    "ENSv2Registry",
    registryAddress,
    "canonicalParentRegistry",
    Address.fromString(parentAddress).toHexString()
  );
  assert.fieldEquals(
    "ENSv2Registry",
    registryAddress,
    "canonicalParentLabel",
    "wrapped-label"
  );
  let registry = ENSv2Registry.load(Address.fromString(registryAddress));
  assert.assertTrue(registry != null);
  // The generated nullable-string setter represents an empty URI as null;
  // the non-null renderer is the active metadata source in this state.
  assert.assertTrue(registry!.uri == null);
  assert.fieldEquals(
    "ENSv2Registry",
    registryAddress,
    "uriRenderer",
    Address.fromString(renderer).toHexString()
  );
  assert.entityCount("Account", 0);
  assert.entityCount("ENSv2RoleAssignment", 0);
  assert.entityCount("ENSv2RoleChange", 0);
});
