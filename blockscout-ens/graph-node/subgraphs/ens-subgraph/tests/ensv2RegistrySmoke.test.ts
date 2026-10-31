// Minimal smoke-test layer for every handler wired from
// src/ensv2Registry.ts (RootRegistry/ETHRegistry/ENSv2Registry template
// mapping). Independent of and redundant with ensv2Registry.test.ts's
// richer behavioral tests -- one mock-event-per-handler baseline only.
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
  handleENSv2ApprovalForAll,
  handleEACRolesChanged,
  handleExpiryUpdated,
  handleLabelRegistered,
  handleLabelReserved,
  handleLabelUnregistered,
  handleParentUpdated,
  handleResolverUpdated,
  handleSubregistryUpdated,
  handleTokenRegenerated,
  handleTokenResource,
  handleTransferBatch,
  handleTransferSingle,
  handleURIUpdated,
} from "../src/ensv2Registry";
import { nameSlotId, tokenEntityId, toSlotId } from "../src/ensv2Utils";
import {
  ApprovalForAll,
  EACRolesChanged,
  ExpiryUpdated,
  LabelRegistered,
  LabelReserved,
  LabelUnregistered,
  ParentUpdated,
  ResolverUpdated,
  SubregistryUpdated,
  TokenRegenerated,
  TokenResource,
  TransferBatch,
  TransferSingle,
  URIUpdated,
} from "../src/types/RootRegistry/PermissionedRegistry";

const REGISTRY = "0x1010101010101010101010101010101010101010";
const OWNER = "0x89205A3A3b2A69De6Dbf7f01ED13B2108B2c43e7";
const SENDER = "0x11111111111111111111111111111111111111aa";

const TWO_POW_32 = BigInt.fromI64(4294967296);
function slotToken(n: i32): BigInt {
  return TWO_POW_32.times(BigInt.fromI32(n));
}
function slotIdFor(tokenId: BigInt): Bytes {
  return nameSlotId(Address.fromString(REGISTRY), toSlotId(tokenId));
}

function newEventLike<T>(
  ctor: (
    address: Address,
    logIndex: BigInt,
    transactionLogIndex: BigInt,
    logType: string | null,
    block: ethereum.Block,
    transaction: ethereum.Transaction,
    parameters: Array<ethereum.EventParam>,
    receipt: ethereum.TransactionReceipt | null
  ) => T
): T {
  let mockEvent = newMockEvent();
  return ctor(
    Address.fromString(REGISTRY),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
}

afterEach(() => {
  clearStore();
});

function fireLabelRegistered(tokenId: BigInt, label: string, labelHash: Bytes): void {
  let event = newEventLike<LabelRegistered>(
    (a, li, tli, lt, b, t, p, r) => new LabelRegistered(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("tokenId", ethereum.Value.fromUnsignedBigInt(tokenId))
  );
  event.parameters.push(
    new ethereum.EventParam("labelHash", ethereum.Value.fromFixedBytes(labelHash))
  );
  event.parameters.push(new ethereum.EventParam("label", ethereum.Value.fromString(label)));
  event.parameters.push(
    new ethereum.EventParam("owner", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "expiry",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2000000000))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("sender", ethereum.Value.fromAddress(Address.fromString(SENDER)))
  );
  handleLabelRegistered(event);
}

test("smoke: handleLabelRegistered creates an ENSv2NameSlot", () => {
  dataSourceMock.setNetwork("sepolia");
  fireLabelRegistered(slotToken(1), "smoke1", Bytes.fromI32(1));
  assert.fieldEquals("ENSv2NameSlot", slotIdFor(slotToken(1)).toHexString(), "status", "REGISTERED");
});

test("smoke: handleLabelReserved creates an ENSv2NameSlot with status RESERVED", () => {
  dataSourceMock.setNetwork("sepolia");
  let event = newEventLike<LabelReserved>(
    (a, li, tli, lt, b, t, p, r) => new LabelReserved(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("tokenId", ethereum.Value.fromUnsignedBigInt(slotToken(2)))
  );
  event.parameters.push(
    new ethereum.EventParam("labelHash", ethereum.Value.fromFixedBytes(Bytes.fromI32(2)))
  );
  event.parameters.push(new ethereum.EventParam("label", ethereum.Value.fromString("smoke2")));
  event.parameters.push(
    new ethereum.EventParam(
      "expiry",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2000000000))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("sender", ethereum.Value.fromAddress(Address.fromString(SENDER)))
  );
  handleLabelReserved(event);
  assert.fieldEquals("ENSv2NameSlot", slotIdFor(slotToken(2)).toHexString(), "status", "RESERVED");
});

test("smoke: handleLabelUnregistered flips an existing slot's status to AVAILABLE", () => {
  dataSourceMock.setNetwork("sepolia");
  fireLabelRegistered(slotToken(3), "smoke3", Bytes.fromI32(3));

  let event = newEventLike<LabelUnregistered>(
    (a, li, tli, lt, b, t, p, r) => new LabelUnregistered(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("tokenId", ethereum.Value.fromUnsignedBigInt(slotToken(3)))
  );
  event.parameters.push(
    new ethereum.EventParam("sender", ethereum.Value.fromAddress(Address.fromString(SENDER)))
  );
  handleLabelUnregistered(event);
  assert.fieldEquals("ENSv2NameSlot", slotIdFor(slotToken(3)).toHexString(), "status", "AVAILABLE");
});

test("smoke: handleExpiryUpdated updates an existing slot's expiryDate", () => {
  dataSourceMock.setNetwork("sepolia");
  fireLabelRegistered(slotToken(4), "smoke4", Bytes.fromI32(4));

  let event = newEventLike<ExpiryUpdated>(
    (a, li, tli, lt, b, t, p, r) => new ExpiryUpdated(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("tokenId", ethereum.Value.fromUnsignedBigInt(slotToken(4)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "newExpiry",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2100000000))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("sender", ethereum.Value.fromAddress(Address.fromString(SENDER)))
  );
  handleExpiryUpdated(event);
  assert.fieldEquals(
    "ENSv2NameSlot",
    slotIdFor(slotToken(4)).toHexString(),
    "expiryDate",
    "2100000000"
  );
});

test("smoke: handleSubregistryUpdated writes a history row for an existing slot", () => {
  dataSourceMock.setNetwork("sepolia");
  fireLabelRegistered(slotToken(5), "smoke5", Bytes.fromI32(5));

  let event = newEventLike<SubregistryUpdated>(
    (a, li, tli, lt, b, t, p, r) => new SubregistryUpdated(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("tokenId", ethereum.Value.fromUnsignedBigInt(slotToken(5)))
  );
  event.parameters.push(
    new ethereum.EventParam("subregistry", ethereum.Value.fromAddress(Address.zero()))
  );
  event.parameters.push(
    new ethereum.EventParam("sender", ethereum.Value.fromAddress(Address.fromString(SENDER)))
  );
  handleSubregistryUpdated(event);
  assert.fieldEquals(
    "ENSv2NameSlot",
    slotIdFor(slotToken(5)).toHexString(),
    "subregistryAddress",
    "null"
  );
});

test("smoke: handleResolverUpdated writes a history row for an existing slot", () => {
  dataSourceMock.setNetwork("sepolia");
  fireLabelRegistered(slotToken(6), "smoke6", Bytes.fromI32(6));

  let event = newEventLike<ResolverUpdated>(
    (a, li, tli, lt, b, t, p, r) => new ResolverUpdated(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("tokenId", ethereum.Value.fromUnsignedBigInt(slotToken(6)))
  );
  event.parameters.push(
    new ethereum.EventParam("resolver", ethereum.Value.fromAddress(Address.zero()))
  );
  event.parameters.push(
    new ethereum.EventParam("sender", ethereum.Value.fromAddress(Address.fromString(SENDER)))
  );
  handleResolverUpdated(event);
  assert.fieldEquals(
    "ENSv2NameSlot",
    slotIdFor(slotToken(6)).toHexString(),
    "resolverAddress",
    "null"
  );
});

test("smoke: handleTokenResource creates an ENSv2Token linked to an existing slot", () => {
  dataSourceMock.setNetwork("sepolia");
  fireLabelRegistered(slotToken(7), "smoke7", Bytes.fromI32(7));

  let event = newEventLike<TokenResource>(
    (a, li, tli, lt, b, t, p, r) => new TokenResource(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("tokenId", ethereum.Value.fromUnsignedBigInt(slotToken(7)))
  );
  event.parameters.push(
    new ethereum.EventParam("resource", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(42)))
  );
  handleTokenResource(event);
  assert.fieldEquals(
    "ENSv2Token",
    tokenEntityId(Address.fromString(REGISTRY), slotToken(7)).toHexString(),
    "active",
    "true"
  );
});

test("smoke: handleTokenRegenerated deactivates the old token and creates a new one", () => {
  dataSourceMock.setNetwork("sepolia");
  fireLabelRegistered(slotToken(8), "smoke8", Bytes.fromI32(8));

  let resourceEvent = newEventLike<TokenResource>(
    (a, li, tli, lt, b, t, p, r) => new TokenResource(a, li, tli, lt, b, t, p, r)
  );
  resourceEvent.parameters = new Array();
  resourceEvent.parameters.push(
    new ethereum.EventParam("tokenId", ethereum.Value.fromUnsignedBigInt(slotToken(8)))
  );
  resourceEvent.parameters.push(
    new ethereum.EventParam("resource", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(1)))
  );
  handleTokenResource(resourceEvent);

  let regenEvent = newEventLike<TokenRegenerated>(
    (a, li, tli, lt, b, t, p, r) => new TokenRegenerated(a, li, tli, lt, b, t, p, r)
  );
  regenEvent.parameters = new Array();
  regenEvent.parameters.push(
    new ethereum.EventParam("oldTokenId", ethereum.Value.fromUnsignedBigInt(slotToken(8)))
  );
  regenEvent.parameters.push(
    new ethereum.EventParam(
      "newTokenId",
      ethereum.Value.fromUnsignedBigInt(slotToken(8).plus(BigInt.fromI32(1)))
    )
  );
  handleTokenRegenerated(regenEvent);

  assert.fieldEquals(
    "ENSv2Token",
    tokenEntityId(Address.fromString(REGISTRY), slotToken(8)).toHexString(),
    "active",
    "false"
  );
});

test("smoke: handleTransferSingle creates an ENSv2Token with the recipient as owner", () => {
  dataSourceMock.setNetwork("sepolia");
  let event = newEventLike<TransferSingle>(
    (a, li, tli, lt, b, t, p, r) => new TransferSingle(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("operator", ethereum.Value.fromAddress(Address.fromString(SENDER)))
  );
  event.parameters.push(
    new ethereum.EventParam("from", ethereum.Value.fromAddress(Address.zero()))
  );
  event.parameters.push(
    new ethereum.EventParam("to", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  event.parameters.push(
    new ethereum.EventParam("id", ethereum.Value.fromUnsignedBigInt(slotToken(9)))
  );
  event.parameters.push(
    new ethereum.EventParam("value", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(1)))
  );
  handleTransferSingle(event);
  assert.fieldEquals(
    "ENSv2Token",
    tokenEntityId(Address.fromString(REGISTRY), slotToken(9)).toHexString(),
    "owner",
    Address.fromString(OWNER).toHexString()
  );
});

test("smoke: handleTransferBatch creates an ENSv2Token for every id in the batch", () => {
  dataSourceMock.setNetwork("sepolia");
  let event = newEventLike<TransferBatch>(
    (a, li, tli, lt, b, t, p, r) => new TransferBatch(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("operator", ethereum.Value.fromAddress(Address.fromString(SENDER)))
  );
  event.parameters.push(
    new ethereum.EventParam("from", ethereum.Value.fromAddress(Address.zero()))
  );
  event.parameters.push(
    new ethereum.EventParam("to", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  let ids = new Array<BigInt>();
  ids.push(slotToken(10));
  ids.push(slotToken(11));
  event.parameters.push(
    new ethereum.EventParam("ids", ethereum.Value.fromUnsignedBigIntArray(ids))
  );
  let values = new Array<BigInt>();
  values.push(BigInt.fromI32(1));
  values.push(BigInt.fromI32(1));
  event.parameters.push(
    new ethereum.EventParam("values", ethereum.Value.fromUnsignedBigIntArray(values))
  );
  handleTransferBatch(event);
  assert.fieldEquals(
    "ENSv2Token",
    tokenEntityId(Address.fromString(REGISTRY), slotToken(10)).toHexString(),
    "owner",
    Address.fromString(OWNER).toHexString()
  );
  assert.fieldEquals(
    "ENSv2Token",
    tokenEntityId(Address.fromString(REGISTRY), slotToken(11)).toHexString(),
    "owner",
    Address.fromString(OWNER).toHexString()
  );
});

test("smoke: handleParentUpdated sets canonicalParentRegistry on the registry row", () => {
  dataSourceMock.setNetwork("sepolia");
  let parentAddress = "0x2020202020202020202020202020202020202020";
  let event = newEventLike<ParentUpdated>(
    (a, li, tli, lt, b, t, p, r) => new ParentUpdated(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "parent",
      ethereum.Value.fromAddress(Address.fromString(parentAddress))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("label", ethereum.Value.fromString("smokeparent"))
  );
  event.parameters.push(
    new ethereum.EventParam("sender", ethereum.Value.fromAddress(Address.fromString(SENDER)))
  );
  handleParentUpdated(event);
  assert.fieldEquals(
    "ENSv2Registry",
    Address.fromString(REGISTRY).toHexString(),
    "canonicalParentRegistry",
    Address.fromString(parentAddress).toHexString()
  );
});

test("smoke: handleENSv2ApprovalForAll creates an OperatorApproval row", () => {
  dataSourceMock.setNetwork("sepolia");
  let operator = "0x3030303030303030303030303030303030303030";
  let event = newEventLike<ApprovalForAll>(
    (a, li, tli, lt, b, t, p, r) => new ApprovalForAll(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("account", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  event.parameters.push(
    new ethereum.EventParam("operator", ethereum.Value.fromAddress(Address.fromString(operator)))
  );
  event.parameters.push(new ethereum.EventParam("approved", ethereum.Value.fromBoolean(true)));
  handleENSv2ApprovalForAll(event);
  assert.fieldEquals(
    "OperatorApproval",
    Address.fromString(REGISTRY)
      .toHexString()
      .concat(Address.fromString(OWNER).toHexString().slice(2))
      .concat(Address.fromString(operator).toHexString().slice(2)),
    "approved",
    "true"
  );
});

test("smoke: handleEACRolesChanged creates an ENSv2RoleAssignment row", () => {
  dataSourceMock.setNetwork("sepolia");
  let event = newEventLike<EACRolesChanged>(
    (a, li, tli, lt, b, t, p, r) => new EACRolesChanged(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("resource", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(1)))
  );
  event.parameters.push(
    new ethereum.EventParam("account", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  event.parameters.push(
    new ethereum.EventParam("oldRoleBitmap", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(0)))
  );
  event.parameters.push(
    new ethereum.EventParam("newRoleBitmap", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(1)))
  );
  handleEACRolesChanged(event);
  assert.entityCount("ENSv2RoleAssignment", 1);
});

test("smoke: handleURIUpdated sets uri/uriRenderer on the registry row", () => {
  dataSourceMock.setNetwork("sepolia");
  let renderer = "0x4040404040404040404040404040404040404040";
  let event = newEventLike<URIUpdated>(
    (a, li, tli, lt, b, t, p, r) => new URIUpdated(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("uri", ethereum.Value.fromString("ipfs://smoke"))
  );
  event.parameters.push(
    new ethereum.EventParam("renderer", ethereum.Value.fromAddress(Address.fromString(renderer)))
  );
  event.parameters.push(
    new ethereum.EventParam("sender", ethereum.Value.fromAddress(Address.fromString(SENDER)))
  );
  handleURIUpdated(event);
  assert.fieldEquals(
    "ENSv2Registry",
    Address.fromString(REGISTRY).toHexString(),
    "uri",
    "ipfs://smoke"
  );
});
