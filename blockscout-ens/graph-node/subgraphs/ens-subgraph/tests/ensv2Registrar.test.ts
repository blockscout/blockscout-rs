import { Address, BigInt, Bytes, ethereum } from "@graphprotocol/graph-ts";
import {
  afterEach,
  assert,
  clearStore,
  dataSourceMock,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import { handleLabelRegistered } from "../src/ensv2Registry";
import {
  handleNameRegistered,
  handleNameRenewed,
} from "../src/ensv2Registrar";
import { nameSlotId, toSlotId } from "../src/ensv2Utils";
import { LabelRegistered } from "../src/types/RootRegistry/PermissionedRegistry";
import {
  NameRegistered,
  NameRenewed,
} from "../src/types/ETHRegistrar/ETHRegistrar";

const ETH_REGISTRY = "0xD4eBcbBdF463C9c45784603Db0dDD499BC44A8B4";
const ETH_REGISTRAR = "0x8c2E866B439358c41AE05De9cbE8A00BFEFafFcA";
const OWNER = "0x89205A3A3b2A69De6Dbf7f01ED13B2108B2c43e7";
const SENDER = "0x11111111111111111111111111111111111111aa";

const TWO_POW_32 = BigInt.fromI64(4294967296);
function slotToken(n: i32): BigInt {
  return TWO_POW_32.times(BigInt.fromI32(n));
}

function registrationIdFor(n: i32): string {
  return nameSlotId(
    Address.fromString(ETH_REGISTRY),
    toSlotId(slotToken(n))
  ).toHexString();
}

const createLabelRegisteredEvent = (
  tokenId: BigInt,
  labelHash: Bytes,
  label: string
): LabelRegistered => {
  let mockEvent = newMockEvent();
  let event = new LabelRegistered(
    Address.fromString(ETH_REGISTRY),
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
      ethereum.Value.fromFixedBytes(labelHash)
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

const createNameRegisteredEvent = (
  tokenId: BigInt,
  label: string,
  duration: BigInt,
  paymentToken: string,
  referrer: Bytes,
  base: BigInt,
  premium: BigInt
): NameRegistered => {
  let mockEvent = newMockEvent();
  let event = new NameRegistered(
    Address.fromString(ETH_REGISTRAR),
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
      "subregistry",
      ethereum.Value.fromAddress(Address.fromString(ETH_REGISTRY))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "resolver",
      ethereum.Value.fromAddress(Address.zero())
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "duration",
      ethereum.Value.fromUnsignedBigInt(duration)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "paymentToken",
      ethereum.Value.fromAddress(Address.fromString(paymentToken))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "referrer",
      ethereum.Value.fromFixedBytes(referrer)
    )
  );
  event.parameters.push(
    new ethereum.EventParam("base", ethereum.Value.fromUnsignedBigInt(base))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "premium",
      ethereum.Value.fromUnsignedBigInt(premium)
    )
  );
  return event;
};

const createNameRenewedEvent = (
  tokenId: BigInt,
  label: string,
  duration: BigInt,
  newExpiry: BigInt,
  paymentToken: string,
  referrer: Bytes,
  amount: BigInt
): NameRenewed => {
  let mockEvent = newMockEvent();
  let event = new NameRenewed(
    Address.fromString(ETH_REGISTRAR),
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
    new ethereum.EventParam("label", ethereum.Value.fromString(label))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "duration",
      ethereum.Value.fromUnsignedBigInt(duration)
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
      "paymentToken",
      ethereum.Value.fromAddress(Address.fromString(paymentToken))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "referrer",
      ethereum.Value.fromFixedBytes(referrer)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "amount",
      ethereum.Value.fromUnsignedBigInt(amount)
    )
  );
  return event;
};

afterEach(() => {
  dataSourceMock.resetValues();
  clearStore();
});

test("handleNameRegistered creates ENSv2Registration with correct enrichment fields", () => {
  dataSourceMock.setNetwork("sepolia");

  const PAYMENT_TOKEN = "0x3DfC8b53dAFa5eBbb071a8B97678Ab534Ed838D9";
  let referrer = Bytes.fromI32(1);

  handleNameRegistered(
    createNameRegisteredEvent(
      slotToken(1),
      "alice",
      BigInt.fromI32(31536000),
      PAYMENT_TOKEN,
      referrer,
      BigInt.fromI32(1000),
      BigInt.fromI32(0)
    )
  );

  let id = registrationIdFor(1);
  assert.fieldEquals("ENSv2Registration", id, "label", "alice");
  assert.fieldEquals("ENSv2Registration", id, "duration", "31536000");
  assert.fieldEquals(
    "ENSv2Registration",
    id,
    "paymentToken",
    Address.fromString(PAYMENT_TOKEN).toHexString()
  );
  assert.fieldEquals("ENSv2Registration", id, "base", "1000");
  assert.fieldEquals(
    "ENSv2Registration",
    id,
    "owner",
    Address.fromString(OWNER).toHexString()
  );
  assert.fieldEquals("ENSv2Registration", id, "slot", id);
});

test("order-independence: registrar-then-registry and registry-then-registrar produce the same final state", () => {
  dataSourceMock.setNetwork("sepolia");

  const PAYMENT_TOKEN = "0x3DfC8b53dAFa5eBbb071a8B97678Ab534Ed838D9";
  let referrer = Bytes.fromI32(2);
  let id = registrationIdFor(2);

  // registrar event first, then registry event
  handleNameRegistered(
    createNameRegisteredEvent(
      slotToken(2),
      "bob",
      BigInt.fromI32(31536000),
      PAYMENT_TOKEN,
      referrer,
      BigInt.fromI32(500),
      BigInt.fromI32(0)
    )
  );
  handleLabelRegistered(
    createLabelRegisteredEvent(slotToken(2), Bytes.fromI32(20), "bob")
  );

  assert.fieldEquals("ENSv2Registration", id, "duration", "31536000");
  assert.fieldEquals("ENSv2Registration", id, "slot", id);
  assert.fieldEquals("ENSv2NameSlot", id, "label", "bob");
});

test("order-independence: registry-then-registrar produces the same final state", () => {
  dataSourceMock.setNetwork("sepolia");

  const PAYMENT_TOKEN = "0x3DfC8b53dAFa5eBbb071a8B97678Ab534Ed838D9";
  let referrer = Bytes.fromI32(3);
  let id = registrationIdFor(3);

  // registry event first, then registrar event
  handleLabelRegistered(
    createLabelRegisteredEvent(slotToken(3), Bytes.fromI32(30), "carol")
  );
  handleNameRegistered(
    createNameRegisteredEvent(
      slotToken(3),
      "carol",
      BigInt.fromI32(31536000),
      PAYMENT_TOKEN,
      referrer,
      BigInt.fromI32(700),
      BigInt.fromI32(0)
    )
  );

  assert.fieldEquals("ENSv2Registration", id, "duration", "31536000");
  assert.fieldEquals("ENSv2Registration", id, "slot", id);
  // Registry expiry is canonical even though timestamp + duration differs.
  assert.fieldEquals("ENSv2Registration", id, "expiryDate", "2000000000");
  assert.fieldEquals("ENSv2NameSlot", id, "label", "carol");
});

test("re-registration replaces the stable slot row's incarnation metadata", () => {
  dataSourceMock.setNetwork("sepolia");

  const PAYMENT_TOKEN = "0x3DfC8b53dAFa5eBbb071a8B97678Ab534Ed838D9";
  const PAYMENT_TOKEN_2 = "0xBA11ebdB3f9a2c5946D8629517f06364E53A2E10";
  let first = createNameRegisteredEvent(
    slotToken(6),
    "first",
    BigInt.fromI32(100),
    PAYMENT_TOKEN,
    Bytes.fromI32(6),
    BigInt.fromI32(10),
    BigInt.fromI32(1)
  );
  first.block.timestamp = BigInt.fromI32(1000);
  handleNameRegistered(first);

  let second = createNameRegisteredEvent(
    slotToken(6),
    "second",
    BigInt.fromI32(200),
    PAYMENT_TOKEN_2,
    Bytes.fromI32(7),
    BigInt.fromI32(20),
    BigInt.fromI32(2)
  );
  second.block.timestamp = BigInt.fromI32(2000);
  handleNameRegistered(second);

  let id = registrationIdFor(6);
  assert.fieldEquals("ENSv2Registration", id, "registrationDate", "2000");
  assert.fieldEquals("ENSv2Registration", id, "label", "second");
  assert.fieldEquals("ENSv2Registration", id, "duration", "200");
  assert.fieldEquals(
    "ENSv2Registration",
    id,
    "paymentToken",
    Address.fromString(PAYMENT_TOKEN_2).toHexString()
  );
  assert.fieldEquals("ENSv2Registration", id, "base", "20");
  assert.fieldEquals("ENSv2Registration", id, "premium", "2");
});

test("handleNameRenewed refreshes an existing registration and is a no-op if none exists", () => {
  dataSourceMock.setNetwork("sepolia");

  const PAYMENT_TOKEN = "0x3DfC8b53dAFa5eBbb071a8B97678Ab534Ed838D9";
  const PAYMENT_TOKEN_2 = "0xBA11ebdB3f9a2c5946D8629517f06364E53A2E10";
  let referrer = Bytes.fromI32(4);
  let id = registrationIdFor(4);

  let registered = createNameRegisteredEvent(
    slotToken(4),
    "dave",
    BigInt.fromI32(31536000),
    PAYMENT_TOKEN,
    referrer,
    BigInt.fromI32(0),
    BigInt.fromI32(0)
  );
  registered.block.timestamp = BigInt.fromI32(1234);
  handleNameRegistered(registered);
  let renewed = createNameRenewedEvent(
    slotToken(4),
    "dave",
    BigInt.fromI32(63072000),
    BigInt.fromI32(2100000000),
    PAYMENT_TOKEN_2,
    referrer,
    BigInt.fromI32(999)
  );
  renewed.block.timestamp = BigInt.fromI32(5678);
  handleNameRenewed(renewed);

  assert.fieldEquals("ENSv2Registration", id, "duration", "63072000");
  assert.fieldEquals(
    "ENSv2Registration",
    id,
    "paymentToken",
    Address.fromString(PAYMENT_TOKEN_2).toHexString()
  );
  assert.fieldEquals("ENSv2Registration", id, "expiryDate", "2100000000");
  assert.fieldEquals("ENSv2Registration", id, "registrationDate", "1234");

  // No prior registration for this slot — renewal must not fabricate one.
  let neverRegisteredId = registrationIdFor(5);
  handleNameRenewed(
    createNameRenewedEvent(
      slotToken(5),
      "erin",
      BigInt.fromI32(31536000),
      BigInt.fromI32(2100000000),
      PAYMENT_TOKEN,
      referrer,
      BigInt.fromI32(0)
    )
  );
  assert.notInStore("ENSv2Registration", neverRegisteredId);
});
