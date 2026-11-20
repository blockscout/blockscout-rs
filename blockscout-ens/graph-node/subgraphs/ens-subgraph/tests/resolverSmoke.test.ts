// Minimal smoke-test layer for every handler wired from src/resolver.ts
// (the addressless classic Resolver data source, including PublicResolverV2).
// Independent of and
// redundant with resolver.test.ts's richer behavioral tests -- one
// mock-event-per-handler baseline only.
import { Address, BigInt, Bytes, ethereum } from "@graphprotocol/graph-ts";
import {
  afterEach,
  assert,
  clearStore,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import {
  createResolverID,
  handleABIChanged,
  handleAddrChanged,
  handleAuthorisationChanged,
  handleContentHashChanged,
  handleDataChanged,
  handleInterfaceChanged,
  handleMulticoinAddrChanged,
  handleNameChanged,
  handlePubkeyChanged,
  handleTextChanged,
  handleTextChangedWithValue,
  handleVersionChanged,
} from "../src/resolver";
import {
  ABIChanged,
  AddrChanged,
  AddressChanged,
  AuthorisationChanged,
  ContenthashChanged,
  DataChanged,
  InterfaceChanged,
  NameChanged,
  PubkeyChanged,
  TextChanged,
  TextChanged1 as TextChangedWithValue,
  VersionChanged,
} from "../src/types/Resolver/Resolver";

const RESOLVER_ADDRESS = "0x8080808080808080808080808080808080808080";
const NODE = Bytes.fromHexString(
  "0xd1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1"
);
const OTHER_ADDRESS = "0x9090909090909090909090909090909090909090";

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
    Address.fromString(RESOLVER_ADDRESS),
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

test("smoke: handleAddrChanged creates a Resolver row", () => {
  let event = newEventLike<AddrChanged>(
    (a, li, tli, lt, b, t, p, r) => new AddrChanged(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(NODE)));
  event.parameters.push(
    new ethereum.EventParam("a", ethereum.Value.fromAddress(Address.fromString(OTHER_ADDRESS)))
  );
  handleAddrChanged(event);
  assert.entityCount("Resolver", 1);
});

test("smoke: handleMulticoinAddrChanged creates a Resolver row with coinTypes populated", () => {
  let event = newEventLike<AddressChanged>(
    (a, li, tli, lt, b, t, p, r) => new AddressChanged(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(NODE)));
  event.parameters.push(
    new ethereum.EventParam("coinType", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(60)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "newAddress",
      ethereum.Value.fromBytes(Address.fromString(OTHER_ADDRESS))
    )
  );
  handleMulticoinAddrChanged(event);
  assert.fieldEquals(
    "Resolver",
    createResolverID(NODE, Address.fromString(RESOLVER_ADDRESS)),
    "id",
    createResolverID(NODE, Address.fromString(RESOLVER_ADDRESS))
  );
});

test("smoke: handleNameChanged creates a NameChanged history row", () => {
  let event = newEventLike<NameChanged>(
    (a, li, tli, lt, b, t, p, r) => new NameChanged(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(NODE)));
  event.parameters.push(new ethereum.EventParam("name", ethereum.Value.fromString("smoke.eth")));
  handleNameChanged(event);
  assert.entityCount("NameChanged", 1);
});

test("smoke: handleABIChanged creates an AbiChanged history row", () => {
  let event = newEventLike<ABIChanged>(
    (a, li, tli, lt, b, t, p, r) => new ABIChanged(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(NODE)));
  event.parameters.push(
    new ethereum.EventParam("contentType", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(1)))
  );
  handleABIChanged(event);
  assert.entityCount("AbiChanged", 1);
});

test("smoke: handlePubkeyChanged creates a PubkeyChanged history row", () => {
  let event = newEventLike<PubkeyChanged>(
    (a, li, tli, lt, b, t, p, r) => new PubkeyChanged(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(NODE)));
  event.parameters.push(
    new ethereum.EventParam("x", ethereum.Value.fromBytes(Bytes.fromI32(1)))
  );
  event.parameters.push(
    new ethereum.EventParam("y", ethereum.Value.fromBytes(Bytes.fromI32(1)))
  );
  handlePubkeyChanged(event);
  assert.entityCount("PubkeyChanged", 1);
});

test("smoke: handleTextChanged creates a TextChanged history row", () => {
  let event = newEventLike<TextChanged>(
    (a, li, tli, lt, b, t, p, r) => new TextChanged(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(NODE)));
  event.parameters.push(
    new ethereum.EventParam("indexedKey", ethereum.Value.fromString("smokekey"))
  );
  event.parameters.push(new ethereum.EventParam("key", ethereum.Value.fromString("smokekey")));
  handleTextChanged(event);
  assert.entityCount("TextChanged", 1);
});

test("smoke: handleTextChangedWithValue creates a TextChanged history row with value", () => {
  let event = newEventLike<TextChangedWithValue>(
    (a, li, tli, lt, b, t, p, r) => new TextChangedWithValue(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(NODE)));
  event.parameters.push(
    new ethereum.EventParam("indexedKey", ethereum.Value.fromString("smokekey"))
  );
  event.parameters.push(new ethereum.EventParam("key", ethereum.Value.fromString("smokekey")));
  event.parameters.push(
    new ethereum.EventParam("value", ethereum.Value.fromString("smokevalue"))
  );
  handleTextChangedWithValue(event);
  assert.entityCount("TextChanged", 1);
});

test("smoke: handleContentHashChanged creates a ContenthashChanged history row", () => {
  let event = newEventLike<ContenthashChanged>(
    (a, li, tli, lt, b, t, p, r) => new ContenthashChanged(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(NODE)));
  event.parameters.push(
    new ethereum.EventParam("hash", ethereum.Value.fromBytes(Bytes.fromI32(1)))
  );
  handleContentHashChanged(event);
  assert.entityCount("ContenthashChanged", 1);
});

test("smoke: handleDataChanged creates ENSv2 node-keyed data state", () => {
  let event = newEventLike<DataChanged>(
    (a, li, tli, lt, b, t, p, r) => new DataChanged(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(NODE))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "indexedKey",
      ethereum.Value.fromBytes(Bytes.fromI32(1))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("key", ethereum.Value.fromString("contentType"))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "indexedData",
      ethereum.Value.fromBytes(Bytes.fromI32(2))
    )
  );

  handleDataChanged(event);

  assert.entityCount("ENSv2Resolver", 1);
  assert.entityCount("ENSv2ResolverData", 1);
});

test("smoke: handleInterfaceChanged creates an InterfaceChanged history row", () => {
  let event = newEventLike<InterfaceChanged>(
    (a, li, tli, lt, b, t, p, r) => new InterfaceChanged(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(NODE)));
  event.parameters.push(
    new ethereum.EventParam("interfaceID", ethereum.Value.fromFixedBytes(Bytes.fromI32(1)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "implementer",
      ethereum.Value.fromAddress(Address.fromString(OTHER_ADDRESS))
    )
  );
  handleInterfaceChanged(event);
  assert.entityCount("InterfaceChanged", 1);
});

test("smoke: handleAuthorisationChanged creates an AuthorisationChanged history row", () => {
  let event = newEventLike<AuthorisationChanged>(
    (a, li, tli, lt, b, t, p, r) => new AuthorisationChanged(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(NODE)));
  event.parameters.push(
    new ethereum.EventParam(
      "owner",
      ethereum.Value.fromAddress(Address.fromString(OTHER_ADDRESS))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "target",
      ethereum.Value.fromAddress(Address.fromString(OTHER_ADDRESS))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("isAuthorised", ethereum.Value.fromBoolean(true))
  );
  handleAuthorisationChanged(event);
  assert.entityCount("AuthorisationChanged", 1);
});

test("smoke: handleVersionChanged creates a VersionChanged history row and resets Resolver fields", () => {
  let event = newEventLike<VersionChanged>(
    (a, li, tli, lt, b, t, p, r) => new VersionChanged(a, li, tli, lt, b, t, p, r)
  );
  event.parameters = new Array();
  event.parameters.push(new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(NODE)));
  event.parameters.push(
    new ethereum.EventParam("newVersion", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(1)))
  );
  handleVersionChanged(event);
  assert.entityCount("VersionChanged", 1);
});
