// Minimal smoke-test layer for every handler wired from src/ethRegistrar.ts
// (BaseRegistrar + Legacy/Wrapped/Unwrapped EthRegistrarController data
// sources). Independent of and redundant with ensRegistrar.test.ts's richer
// behavioral tests (that file, despite its v1-registry-sounding name, is
// actually this file's existing counterpart) -- one mock-event-per-handler
// baseline only.
import { Address, BigInt, Bytes, ethereum } from "@graphprotocol/graph-ts";
import {
  assert,
  beforeEach,
  clearStore,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import {
  handleBaseRegistrarApprovalForAll,
  handleBaseRegistrarOwnershipTransferred,
  handleControllerAdded,
  handleControllerRemoved,
  handleLegacyControllerOwnershipTransferred,
  handleNameRegistered,
  handleNameRegisteredByLegacyController,
  handleNameRegisteredByUnwrappedController,
  handleNameRegisteredByWrappedController,
  handleNameRenewed,
  handleNameRenewedByLegacyController,
  handleNameRenewedByUnwrappedController,
  handleNameTransferred,
  handleUnwrappedControllerOwnershipTransferred,
  handleWrappedControllerOwnershipTransferred,
} from "../src/ethRegistrar";
import {
  ApprovalForAll,
  ControllerAdded,
  ControllerRemoved,
  NameRegistered,
  NameRenewed,
  OwnershipTransferred,
  Transfer,
} from "../src/types/BaseRegistrar/BaseRegistrar";
import {
  NameRegistered as LegacyNameRegistered,
  NameRenewed as LegacyNameRenewed,
  OwnershipTransferred as LegacyOwnershipTransferred,
} from "../src/types/LegacyEthRegistrarController/LegacyEthRegistrarController";
import {
  NameRegistered as UnwrappedNameRegistered,
  NameRenewed as UnwrappedNameRenewed,
  OwnershipTransferred as UnwrappedOwnershipTransferred,
} from "../src/types/UnwrappedEthRegistrarController/UnwrappedEthRegistrarController";
import {
  NameRegistered as WrappedNameRegistered,
  OwnershipTransferred as WrappedOwnershipTransferred,
} from "../src/types/WrappedEthRegistrarController/WrappedEthRegistrarController";
import { ETH_NODE, uint256ToByteArray } from "../src/utils";
import { createNewOwnerEvent, DEFAULT_OWNER, setEthOwner } from "./testUtils";
import { handleNewOwner } from "../src/ensRegistry";

const CONTRACT = "0xE0E0E0E0E0E0E0E0E0E0E0E0E0E0E0E0E0E0E0E0";
const NEW_OWNER = "0xF0F0F0F0F0F0F0F0F0F0F0F0F0F0F0F0F0F0F0F0";

// beforeEach, not beforeAll -- every test below ends with its own
// clearStore(), which would otherwise wipe the ETH_NODE domain out from
// under every test after the first.
beforeEach(() => {
  setEthOwner();
});

function tokenIdFor(n: i32): BigInt {
  return BigInt.fromI32(n);
}
function labelHashFor(n: i32): Bytes {
  return Bytes.fromByteArray(uint256ToByteArray(tokenIdFor(n)));
}
function labelHexFor(n: i32): string {
  return labelHashFor(n).toHexString();
}

// A fresh subnode of ETH_NODE must exist (created the same way
// ensRegistry.ts's NewOwner handler creates one) before BaseRegistrar's own
// NameRegistered will do anything other than log-and-skip.
function createEthSubnode(n: i32): void {
  handleNewOwner(
    createNewOwnerEvent(ETH_NODE.toHexString(), labelHashFor(n).toHexString(), DEFAULT_OWNER)
  );
}

test("smoke: handleNameRegistered creates a Registration row", () => {
  createEthSubnode(1);
  let mockEvent = newMockEvent();
  let event = new NameRegistered(
    Address.fromString(CONTRACT),
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
    new ethereum.EventParam("id", ethereum.Value.fromUnsignedBigInt(tokenIdFor(1)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "owner",
      ethereum.Value.fromAddress(Address.fromString(DEFAULT_OWNER))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "expires",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2000000000))
    )
  );
  handleNameRegistered(event);
  assert.entityCount("Registration", 1);
  clearStore();
});

test("smoke: handleNameRenewed updates an existing Registration's expiryDate", () => {
  createEthSubnode(2);
  let registerEvent = new NameRegistered(
    Address.fromString(CONTRACT),
    newMockEvent().logIndex,
    newMockEvent().transactionLogIndex,
    newMockEvent().logType,
    newMockEvent().block,
    newMockEvent().transaction,
    new Array<ethereum.EventParam>(),
    newMockEvent().receipt
  );
  registerEvent.parameters.push(
    new ethereum.EventParam("id", ethereum.Value.fromUnsignedBigInt(tokenIdFor(2)))
  );
  registerEvent.parameters.push(
    new ethereum.EventParam(
      "owner",
      ethereum.Value.fromAddress(Address.fromString(DEFAULT_OWNER))
    )
  );
  registerEvent.parameters.push(
    new ethereum.EventParam(
      "expires",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2000000000))
    )
  );
  handleNameRegistered(registerEvent);

  let mockEvent = newMockEvent();
  let renewEvent = new NameRenewed(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  renewEvent.parameters = new Array();
  renewEvent.parameters.push(
    new ethereum.EventParam("id", ethereum.Value.fromUnsignedBigInt(tokenIdFor(2)))
  );
  renewEvent.parameters.push(
    new ethereum.EventParam(
      "expires",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2100000000))
    )
  );
  handleNameRenewed(renewEvent);
  assert.fieldEquals("Registration", labelHexFor(2), "expiryDate", "2100000000");
  clearStore();
});

test("smoke: handleNameTransferred updates an existing Registration's registrant", () => {
  createEthSubnode(3);
  let registerEvent = new NameRegistered(
    Address.fromString(CONTRACT),
    newMockEvent().logIndex,
    newMockEvent().transactionLogIndex,
    newMockEvent().logType,
    newMockEvent().block,
    newMockEvent().transaction,
    new Array<ethereum.EventParam>(),
    newMockEvent().receipt
  );
  registerEvent.parameters.push(
    new ethereum.EventParam("id", ethereum.Value.fromUnsignedBigInt(tokenIdFor(3)))
  );
  registerEvent.parameters.push(
    new ethereum.EventParam(
      "owner",
      ethereum.Value.fromAddress(Address.fromString(DEFAULT_OWNER))
    )
  );
  registerEvent.parameters.push(
    new ethereum.EventParam(
      "expires",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2000000000))
    )
  );
  handleNameRegistered(registerEvent);

  let mockEvent = newMockEvent();
  let transferEvent = new Transfer(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  transferEvent.parameters = new Array();
  transferEvent.parameters.push(
    new ethereum.EventParam(
      "from",
      ethereum.Value.fromAddress(Address.fromString(DEFAULT_OWNER))
    )
  );
  transferEvent.parameters.push(
    new ethereum.EventParam("to", ethereum.Value.fromAddress(Address.fromString(NEW_OWNER)))
  );
  transferEvent.parameters.push(
    new ethereum.EventParam("tokenId", ethereum.Value.fromUnsignedBigInt(tokenIdFor(3)))
  );
  handleNameTransferred(transferEvent);
  assert.fieldEquals(
    "Registration",
    labelHexFor(3),
    "registrant",
    Address.fromString(NEW_OWNER).toHexString()
  );
  clearStore();
});

test("smoke: handleBaseRegistrarApprovalForAll creates an OperatorApproval row", () => {
  let mockEvent = newMockEvent();
  let event = new ApprovalForAll(
    Address.fromString(CONTRACT),
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
      "owner",
      ethereum.Value.fromAddress(Address.fromString(DEFAULT_OWNER))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("operator", ethereum.Value.fromAddress(Address.fromString(NEW_OWNER)))
  );
  event.parameters.push(new ethereum.EventParam("approved", ethereum.Value.fromBoolean(true)));
  handleBaseRegistrarApprovalForAll(event);
  assert.entityCount("OperatorApproval", 1);
  clearStore();
});

test("smoke: handleBaseRegistrarOwnershipTransferred sets ContractOwnership.owner", () => {
  let mockEvent = newMockEvent();
  let event = new OwnershipTransferred(
    Address.fromString(CONTRACT),
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
      ethereum.Value.fromAddress(Address.fromString(DEFAULT_OWNER))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("newOwner", ethereum.Value.fromAddress(Address.fromString(NEW_OWNER)))
  );
  handleBaseRegistrarOwnershipTransferred(event);
  assert.fieldEquals(
    "ContractOwnership",
    Address.fromString(CONTRACT).toHexString(),
    "owner",
    Address.fromString(NEW_OWNER).toHexString()
  );
  clearStore();
});

test("smoke: handleControllerAdded/handleControllerRemoved flip RegistrarController.active", () => {
  let controller = "0x1212121212121212121212121212121212121212";
  let mockEvent = newMockEvent();
  let addEvent = new ControllerAdded(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  addEvent.parameters = new Array();
  addEvent.parameters.push(
    new ethereum.EventParam(
      "controller",
      ethereum.Value.fromAddress(Address.fromString(controller))
    )
  );
  handleControllerAdded(addEvent);

  let removeEvent = new ControllerRemoved(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  removeEvent.parameters = new Array();
  removeEvent.parameters.push(
    new ethereum.EventParam(
      "controller",
      ethereum.Value.fromAddress(Address.fromString(controller))
    )
  );
  handleControllerRemoved(removeEvent);

  let id = Address.fromString(CONTRACT).toHexString().concat(
    Address.fromString(controller).toHexString().slice(2)
  );
  assert.fieldEquals("RegistrarController", id, "active", "false");
  clearStore();
});

test("smoke: handleNameRegisteredByLegacyController sets Registration.cost", () => {
  createEthSubnode(4);
  let mockEvent = newMockEvent();
  let event = new LegacyNameRegistered(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(new ethereum.EventParam("name", ethereum.Value.fromString("smoke4")));
  event.parameters.push(
    new ethereum.EventParam("label", ethereum.Value.fromFixedBytes(labelHashFor(4)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "owner",
      ethereum.Value.fromAddress(Address.fromString(DEFAULT_OWNER))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("cost", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(100)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "expires",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2000000000))
    )
  );
  // Registration must already exist for setNamePreimage's cost-write to
  // stick -- BaseRegistrar's own NameRegistered creates it.
  let registerEvent = new NameRegistered(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    new Array<ethereum.EventParam>(),
    mockEvent.receipt
  );
  registerEvent.parameters.push(
    new ethereum.EventParam("id", ethereum.Value.fromUnsignedBigInt(tokenIdFor(4)))
  );
  registerEvent.parameters.push(
    new ethereum.EventParam(
      "owner",
      ethereum.Value.fromAddress(Address.fromString(DEFAULT_OWNER))
    )
  );
  registerEvent.parameters.push(
    new ethereum.EventParam(
      "expires",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2000000000))
    )
  );
  handleNameRegistered(registerEvent);

  handleNameRegisteredByLegacyController(event);
  assert.fieldEquals("Registration", labelHexFor(4), "cost", "100");
  clearStore();
});

test("smoke: handleNameRenewedByLegacyController sets Registration.cost", () => {
  createEthSubnode(5);
  let mockEvent = newMockEvent();
  let registerEvent = new NameRegistered(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    new Array<ethereum.EventParam>(),
    mockEvent.receipt
  );
  registerEvent.parameters.push(
    new ethereum.EventParam("id", ethereum.Value.fromUnsignedBigInt(tokenIdFor(5)))
  );
  registerEvent.parameters.push(
    new ethereum.EventParam(
      "owner",
      ethereum.Value.fromAddress(Address.fromString(DEFAULT_OWNER))
    )
  );
  registerEvent.parameters.push(
    new ethereum.EventParam(
      "expires",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2000000000))
    )
  );
  handleNameRegistered(registerEvent);

  let event = new LegacyNameRenewed(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(new ethereum.EventParam("name", ethereum.Value.fromString("smoke5")));
  event.parameters.push(
    new ethereum.EventParam("label", ethereum.Value.fromFixedBytes(labelHashFor(5)))
  );
  event.parameters.push(
    new ethereum.EventParam("cost", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(200)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "expires",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2100000000))
    )
  );
  handleNameRenewedByLegacyController(event);
  assert.fieldEquals("Registration", labelHexFor(5), "cost", "200");
  clearStore();
});

test("smoke: handleLegacyControllerOwnershipTransferred sets ContractOwnership.owner", () => {
  let mockEvent = newMockEvent();
  let event = new LegacyOwnershipTransferred(
    Address.fromString(CONTRACT),
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
      ethereum.Value.fromAddress(Address.fromString(DEFAULT_OWNER))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("newOwner", ethereum.Value.fromAddress(Address.fromString(NEW_OWNER)))
  );
  handleLegacyControllerOwnershipTransferred(event);
  assert.fieldEquals(
    "ContractOwnership",
    Address.fromString(CONTRACT).toHexString(),
    "owner",
    Address.fromString(NEW_OWNER).toHexString()
  );
  clearStore();
});

test("smoke: handleNameRegisteredByWrappedController sets Registration.cost (baseCost+premium)", () => {
  createEthSubnode(6);
  let mockEvent = newMockEvent();
  let registerEvent = new NameRegistered(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    new Array<ethereum.EventParam>(),
    mockEvent.receipt
  );
  registerEvent.parameters.push(
    new ethereum.EventParam("id", ethereum.Value.fromUnsignedBigInt(tokenIdFor(6)))
  );
  registerEvent.parameters.push(
    new ethereum.EventParam(
      "owner",
      ethereum.Value.fromAddress(Address.fromString(DEFAULT_OWNER))
    )
  );
  registerEvent.parameters.push(
    new ethereum.EventParam(
      "expires",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2000000000))
    )
  );
  handleNameRegistered(registerEvent);

  let event = new WrappedNameRegistered(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(new ethereum.EventParam("name", ethereum.Value.fromString("smoke6")));
  event.parameters.push(
    new ethereum.EventParam("label", ethereum.Value.fromFixedBytes(labelHashFor(6)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "owner",
      ethereum.Value.fromAddress(Address.fromString(DEFAULT_OWNER))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("baseCost", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(300)))
  );
  event.parameters.push(
    new ethereum.EventParam("premium", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(0)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "expires",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2000000000))
    )
  );
  handleNameRegisteredByWrappedController(event);
  assert.fieldEquals("Registration", labelHexFor(6), "cost", "300");
  clearStore();
});

test("smoke: handleWrappedControllerOwnershipTransferred sets ContractOwnership.owner", () => {
  let mockEvent = newMockEvent();
  let event = new WrappedOwnershipTransferred(
    Address.fromString(CONTRACT),
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
      ethereum.Value.fromAddress(Address.fromString(DEFAULT_OWNER))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("newOwner", ethereum.Value.fromAddress(Address.fromString(NEW_OWNER)))
  );
  handleWrappedControllerOwnershipTransferred(event);
  assert.fieldEquals(
    "ContractOwnership",
    Address.fromString(CONTRACT).toHexString(),
    "owner",
    Address.fromString(NEW_OWNER).toHexString()
  );
  clearStore();
});

test("smoke: handleNameRegisteredByUnwrappedController sets Registration.cost (baseCost+premium)", () => {
  createEthSubnode(7);
  let mockEvent = newMockEvent();
  let registerEvent = new NameRegistered(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    new Array<ethereum.EventParam>(),
    mockEvent.receipt
  );
  registerEvent.parameters.push(
    new ethereum.EventParam("id", ethereum.Value.fromUnsignedBigInt(tokenIdFor(7)))
  );
  registerEvent.parameters.push(
    new ethereum.EventParam(
      "owner",
      ethereum.Value.fromAddress(Address.fromString(DEFAULT_OWNER))
    )
  );
  registerEvent.parameters.push(
    new ethereum.EventParam(
      "expires",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2000000000))
    )
  );
  handleNameRegistered(registerEvent);

  let event = new UnwrappedNameRegistered(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(new ethereum.EventParam("label", ethereum.Value.fromString("smoke7")));
  event.parameters.push(
    new ethereum.EventParam("labelhash", ethereum.Value.fromFixedBytes(labelHashFor(7)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "owner",
      ethereum.Value.fromAddress(Address.fromString(DEFAULT_OWNER))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("baseCost", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(400)))
  );
  event.parameters.push(
    new ethereum.EventParam("premium", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(0)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "expires",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2000000000))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("referrer", ethereum.Value.fromFixedBytes(Bytes.fromI32(0)))
  );
  handleNameRegisteredByUnwrappedController(event);
  assert.fieldEquals("Registration", labelHexFor(7), "cost", "400");
  clearStore();
});

test("smoke: handleNameRenewedByUnwrappedController sets Registration.cost", () => {
  createEthSubnode(8);
  let mockEvent = newMockEvent();
  let registerEvent = new NameRegistered(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    new Array<ethereum.EventParam>(),
    mockEvent.receipt
  );
  registerEvent.parameters.push(
    new ethereum.EventParam("id", ethereum.Value.fromUnsignedBigInt(tokenIdFor(8)))
  );
  registerEvent.parameters.push(
    new ethereum.EventParam(
      "owner",
      ethereum.Value.fromAddress(Address.fromString(DEFAULT_OWNER))
    )
  );
  registerEvent.parameters.push(
    new ethereum.EventParam(
      "expires",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2000000000))
    )
  );
  handleNameRegistered(registerEvent);

  let event = new UnwrappedNameRenewed(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(new ethereum.EventParam("label", ethereum.Value.fromString("smoke8")));
  event.parameters.push(
    new ethereum.EventParam("labelhash", ethereum.Value.fromFixedBytes(labelHashFor(8)))
  );
  event.parameters.push(
    new ethereum.EventParam("cost", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(500)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "expires",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2100000000))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("referrer", ethereum.Value.fromFixedBytes(Bytes.fromI32(0)))
  );
  handleNameRenewedByUnwrappedController(event);
  assert.fieldEquals("Registration", labelHexFor(8), "cost", "500");
  clearStore();
});

test("smoke: handleUnwrappedControllerOwnershipTransferred sets ContractOwnership.owner", () => {
  let mockEvent = newMockEvent();
  let event = new UnwrappedOwnershipTransferred(
    Address.fromString(CONTRACT),
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
      ethereum.Value.fromAddress(Address.fromString(DEFAULT_OWNER))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("newOwner", ethereum.Value.fromAddress(Address.fromString(NEW_OWNER)))
  );
  handleUnwrappedControllerOwnershipTransferred(event);
  assert.fieldEquals(
    "ContractOwnership",
    Address.fromString(CONTRACT).toHexString(),
    "owner",
    Address.fromString(NEW_OWNER).toHexString()
  );
  clearStore();
});
