// Minimal smoke-test layer for every handler wired from
// src/ensv2Registrar.ts (ETHRegistrar data source). Independent of and
// redundant with ensv2Registrar.test.ts's richer behavioral tests.
import { Address, BigInt, Bytes, ethereum } from "@graphprotocol/graph-ts";
import {
  assert,
  dataSourceMock,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import {
  handleETHRegistrarOwnershipTransferred,
  handleNameRegistered,
  handleNameRenewed,
} from "../src/ensv2Registrar";
import { nameSlotId, toSlotId } from "../src/ensv2Utils";
import {
  NameRegistered,
  NameRenewed,
  OwnershipTransferred,
} from "../src/types/ETHRegistrar/ETHRegistrar";

const ETH_REGISTRY = "0xD4eBcbBdF463C9c45784603Db0dDD499BC44A8B4";
const ETH_REGISTRAR = "0x8c2E866B439358c41AE05De9cbE8A00BFEFafFcA";
const OWNER = "0x89205A3A3b2A69De6Dbf7f01ED13B2108B2c43e7";

const TWO_POW_32 = BigInt.fromI64(4294967296);
const TOKEN_ID = TWO_POW_32.times(BigInt.fromI32(555));
const REGISTRATION_ID = nameSlotId(
  Address.fromString(ETH_REGISTRY),
  toSlotId(TOKEN_ID)
).toHexString();

const createNameRegisteredEvent = (): NameRegistered => {
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
    new ethereum.EventParam("tokenId", ethereum.Value.fromUnsignedBigInt(TOKEN_ID))
  );
  event.parameters.push(
    new ethereum.EventParam("label", ethereum.Value.fromString("smoketest"))
  );
  event.parameters.push(
    new ethereum.EventParam("owner", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "subregistry",
      ethereum.Value.fromAddress(Address.fromString(ETH_REGISTRY))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("resolver", ethereum.Value.fromAddress(Address.zero()))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "duration",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(31536000))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "paymentToken",
      ethereum.Value.fromAddress(Address.zero())
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "referrer",
      ethereum.Value.fromFixedBytes(Bytes.fromI32(0))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("base", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(0)))
  );
  event.parameters.push(
    new ethereum.EventParam("premium", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(0)))
  );
  return event;
};

const createNameRenewedEvent = (): NameRenewed => {
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
    new ethereum.EventParam("tokenId", ethereum.Value.fromUnsignedBigInt(TOKEN_ID))
  );
  event.parameters.push(
    new ethereum.EventParam("label", ethereum.Value.fromString("smoketest"))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "duration",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(31536000))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "newExpiry",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2000000000))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "paymentToken",
      ethereum.Value.fromAddress(Address.zero())
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "referrer",
      ethereum.Value.fromFixedBytes(Bytes.fromI32(0))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("amount", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(0)))
  );
  return event;
};

test("smoke: handleNameRegistered creates an ENSv2Registration row", () => {
  dataSourceMock.setNetwork("sepolia");
  handleNameRegistered(createNameRegisteredEvent());
  assert.fieldEquals("ENSv2Registration", REGISTRATION_ID, "label", "smoketest");
});

test("smoke: handleNameRenewed updates an existing ENSv2Registration's expiryDate", () => {
  dataSourceMock.setNetwork("sepolia");
  handleNameRegistered(createNameRegisteredEvent());
  handleNameRenewed(createNameRenewedEvent());
  assert.fieldEquals("ENSv2Registration", REGISTRATION_ID, "expiryDate", "2000000000");
});

test("smoke: handleETHRegistrarOwnershipTransferred runs without crashing", () => {
  dataSourceMock.setNetwork("sepolia");
  let mockEvent = newMockEvent();
  let event = new OwnershipTransferred(
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
      "previousOwner",
      ethereum.Value.fromAddress(Address.fromString(OWNER))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "newOwner",
      ethereum.Value.fromAddress(Address.fromString("0x99e99e99e99e99e99e99e99e99e99e99e99e9999"))
    )
  );
  handleETHRegistrarOwnershipTransferred(event);
  assert.fieldEquals(
    "ContractOwnership",
    Address.fromString(ETH_REGISTRAR).toHexString(),
    "owner",
    Address.fromString("0x99e99e99e99e99e99e99e99e99e99e99e99e9999").toHexString()
  );
});
