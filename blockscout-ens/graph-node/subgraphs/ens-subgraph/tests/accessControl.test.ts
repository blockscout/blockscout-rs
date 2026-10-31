import { Address, ethereum } from "@graphprotocol/graph-ts";
import {
  afterEach,
  assert,
  clearStore,
  dataSourceMock,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import {
  processApprovalForAll,
  processControllerStatus,
  processOwnershipTransferred,
} from "../src/accessControl";
import {
  handleApprovalForAll,
  handleApprovalForAllOldRegistry,
} from "../src/ensRegistry";
import {
  handleBaseRegistrarApprovalForAll,
  handleBaseRegistrarOwnershipTransferred,
  handleControllerAdded,
  handleControllerRemoved,
  handleLegacyControllerOwnershipTransferred,
  handleUnwrappedControllerOwnershipTransferred,
  handleWrappedControllerOwnershipTransferred,
} from "../src/ethRegistrar";
import {
  handleControllerChanged,
  handleNameWrapperApprovalForAll,
  handleNameWrapperOwnershipTransferred,
} from "../src/nameWrapper";
import { handleENSv2ApprovalForAll } from "../src/ensv2Registry";
import { handleETHRegistrarOwnershipTransferred } from "../src/ensv2Registrar";
import { ApprovalForAll as ENSRegistryApprovalForAll } from "../src/types/ENSRegistry/EnsRegistry";
import {
  ApprovalForAll as BaseRegistrarApprovalForAll,
  ControllerAdded,
  ControllerRemoved,
  OwnershipTransferred as BaseRegistrarOwnershipTransferred,
} from "../src/types/BaseRegistrar/BaseRegistrar";
import { OwnershipTransferred as LegacyControllerOwnershipTransferred } from "../src/types/LegacyEthRegistrarController/LegacyEthRegistrarController";
import { OwnershipTransferred as WrappedControllerOwnershipTransferred } from "../src/types/WrappedEthRegistrarController/WrappedEthRegistrarController";
import { OwnershipTransferred as UnwrappedControllerOwnershipTransferred } from "../src/types/UnwrappedEthRegistrarController/UnwrappedEthRegistrarController";
import {
  ApprovalForAll as NameWrapperApprovalForAll,
  ControllerChanged,
  OwnershipTransferred as NameWrapperOwnershipTransferred,
} from "../src/types/NameWrapper/NameWrapper";
import { ApprovalForAll as ENSv2ApprovalForAll } from "../src/types/RootRegistry/PermissionedRegistry";
import { OwnershipTransferred as ETHRegistrarOwnershipTransferred } from "../src/types/ETHRegistrar/ETHRegistrar";

const CONTRACT = "0x1111111111111111111111111111111111111111";
const OWNER = "0x2222222222222222222222222222222222222222";
const OPERATOR = "0x3333333333333333333333333333333333333333";
const NEW_OWNER = "0x4444444444444444444444444444444444444444";
const CONTROLLER = "0x5555555555555555555555555555555555555555";

// assert.fieldEquals compares an entity's id as its lowercase-hex string
// form regardless of the underlying GraphQL type. Production code
// builds these ids as fixed-width Bytes concatenation with no delimiter
// (contract/owner/operator/controller are all 20-byte addresses) — the hex
// form of that concatenation is just each address's own hex digits run
// together with a single leading "0x", so stripping "0x" from every
// component but the first and concatenating reproduces it exactly.
function operatorApprovalId(contract: string, owner: string, operator: string): string {
  let c = Address.fromString(contract).toHexString();
  let o = Address.fromString(owner).toHexString().slice(2);
  let op = Address.fromString(operator).toHexString().slice(2);
  return c.concat(o).concat(op);
}

function controllerId(contract: string, controller: string): string {
  let c = Address.fromString(contract).toHexString();
  let ctrl = Address.fromString(controller).toHexString().slice(2);
  return c.concat(ctrl);
}

function approvalParams(
  firstParamName: string,
  first: string,
  operator: string,
  approved: boolean
): Array<ethereum.EventParam> {
  let params = new Array<ethereum.EventParam>();
  params.push(
    new ethereum.EventParam(firstParamName, ethereum.Value.fromAddress(Address.fromString(first)))
  );
  params.push(
    new ethereum.EventParam("operator", ethereum.Value.fromAddress(Address.fromString(operator)))
  );
  params.push(new ethereum.EventParam("approved", ethereum.Value.fromBoolean(approved)));
  return params;
}

function ownershipParams(previousOwner: string, newOwner: string): Array<ethereum.EventParam> {
  let params = new Array<ethereum.EventParam>();
  params.push(
    new ethereum.EventParam(
      "previousOwner",
      ethereum.Value.fromAddress(Address.fromString(previousOwner))
    )
  );
  params.push(
    new ethereum.EventParam("newOwner", ethereum.Value.fromAddress(Address.fromString(newOwner)))
  );
  return params;
}

afterEach(() => {
  clearStore();
  dataSourceMock.resetValues();
});

// --- shared accessControl.ts functions, called directly ---

test("processApprovalForAll: grant then revoke leaves a single OperatorApproval row at the latest state", () => {
  let contract = Address.fromString(CONTRACT);
  let owner = Address.fromString(OWNER);
  let operator = Address.fromString(OPERATOR);
  let id = operatorApprovalId(CONTRACT, OWNER, OPERATOR);

  processApprovalForAll(contract, owner, operator, true, newMockEvent().block);
  assert.fieldEquals("OperatorApproval", id, "approved", "true");
  assert.entityCount("OperatorApproval", 1);

  processApprovalForAll(contract, owner, operator, false, newMockEvent().block);
  assert.fieldEquals("OperatorApproval", id, "approved", "false");
  assert.entityCount("OperatorApproval", 1);
});

test("processOwnershipTransferred: one row per contract, always the latest owner", () => {
  let contract = Address.fromString(CONTRACT);
  processOwnershipTransferred(contract, Address.fromString(OWNER), newMockEvent().block);
  assert.fieldEquals(
    "ContractOwnership",
    contract.toHexString(),
    "owner",
    Address.fromString(OWNER).toHexString()
  );

  processOwnershipTransferred(contract, Address.fromString(NEW_OWNER), newMockEvent().block);
  assert.fieldEquals(
    "ContractOwnership",
    contract.toHexString(),
    "owner",
    Address.fromString(NEW_OWNER).toHexString()
  );
  assert.entityCount("ContractOwnership", 1);
});

test("processControllerStatus: add then remove flips active without deleting the row", () => {
  let contract = Address.fromString(CONTRACT);
  let controller = Address.fromString(CONTROLLER);
  let id = controllerId(CONTRACT, CONTROLLER);

  processControllerStatus(contract, controller, true, newMockEvent().block);
  assert.fieldEquals("RegistrarController", id, "active", "true");

  processControllerStatus(contract, controller, false, newMockEvent().block);
  assert.fieldEquals("RegistrarController", id, "active", "false");
  assert.entityCount("RegistrarController", 1);
});

// --- thin-wrapper sanity: each contract's own ABI param names unpack correctly ---

test("ensRegistry: handleApprovalForAll unpacks ENSRegistry's `owner` param", () => {
  let mockEvent = newMockEvent();
  let event = new ENSRegistryApprovalForAll(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    approvalParams("owner", OWNER, OPERATOR, true),
    mockEvent.receipt
  );
  handleApprovalForAll(event);
  assert.fieldEquals("OperatorApproval", operatorApprovalId(CONTRACT, OWNER, OPERATOR), "approved", "true");
});

test("ensRegistry: handleApprovalForAllOldRegistry indexes unconditionally (no migration gate)", () => {
  let mockEvent = newMockEvent();
  let event = new ENSRegistryApprovalForAll(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    approvalParams("owner", OWNER, OPERATOR, true),
    mockEvent.receipt
  );
  handleApprovalForAllOldRegistry(event);
  assert.fieldEquals("OperatorApproval", operatorApprovalId(CONTRACT, OWNER, OPERATOR), "approved", "true");
});

test("ethRegistrar: handleBaseRegistrarApprovalForAll unpacks BaseRegistrar's `owner` param", () => {
  let mockEvent = newMockEvent();
  let event = new BaseRegistrarApprovalForAll(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    approvalParams("owner", OWNER, OPERATOR, true),
    mockEvent.receipt
  );
  handleBaseRegistrarApprovalForAll(event);
  assert.fieldEquals("OperatorApproval", operatorApprovalId(CONTRACT, OWNER, OPERATOR), "approved", "true");
});

test("ethRegistrar: handleBaseRegistrarOwnershipTransferred records the new owner", () => {
  let mockEvent = newMockEvent();
  let event = new BaseRegistrarOwnershipTransferred(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    ownershipParams(OWNER, NEW_OWNER),
    mockEvent.receipt
  );
  handleBaseRegistrarOwnershipTransferred(event);
  assert.fieldEquals(
    "ContractOwnership",
    Address.fromString(CONTRACT).toHexString(),
    "owner",
    Address.fromString(NEW_OWNER).toHexString()
  );
});

test("ethRegistrar: handleControllerAdded then handleControllerRemoved flips RegistrarController.active", () => {
  let mockEvent = newMockEvent();
  let controllerParams = new Array<ethereum.EventParam>();
  controllerParams.push(
    new ethereum.EventParam("controller", ethereum.Value.fromAddress(Address.fromString(CONTROLLER)))
  );

  let addEvent = new ControllerAdded(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    controllerParams,
    mockEvent.receipt
  );
  handleControllerAdded(addEvent);
  assert.fieldEquals("RegistrarController", controllerId(CONTRACT, CONTROLLER), "active", "true");

  let removeEvent = new ControllerRemoved(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    controllerParams,
    mockEvent.receipt
  );
  handleControllerRemoved(removeEvent);
  assert.fieldEquals("RegistrarController", controllerId(CONTRACT, CONTROLLER), "active", "false");
});

test("ethRegistrar: handleLegacyControllerOwnershipTransferred records the new owner (shared shape with wrapped/unwrapped controllers)", () => {
  let mockEvent = newMockEvent();
  let event = new LegacyControllerOwnershipTransferred(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    ownershipParams(OWNER, NEW_OWNER),
    mockEvent.receipt
  );
  handleLegacyControllerOwnershipTransferred(event);
  assert.fieldEquals(
    "ContractOwnership",
    Address.fromString(CONTRACT).toHexString(),
    "owner",
    Address.fromString(NEW_OWNER).toHexString()
  );
});

test("ethRegistrar: handleWrappedControllerOwnershipTransferred records the new owner (same shared shape)", () => {
  let mockEvent = newMockEvent();
  let event = new WrappedControllerOwnershipTransferred(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    ownershipParams(OWNER, NEW_OWNER),
    mockEvent.receipt
  );
  handleWrappedControllerOwnershipTransferred(event);
  assert.fieldEquals(
    "ContractOwnership",
    Address.fromString(CONTRACT).toHexString(),
    "owner",
    Address.fromString(NEW_OWNER).toHexString()
  );
});

test("ethRegistrar: handleUnwrappedControllerOwnershipTransferred records the new owner (same shared shape)", () => {
  let mockEvent = newMockEvent();
  let event = new UnwrappedControllerOwnershipTransferred(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    ownershipParams(OWNER, NEW_OWNER),
    mockEvent.receipt
  );
  handleUnwrappedControllerOwnershipTransferred(event);
  assert.fieldEquals(
    "ContractOwnership",
    Address.fromString(CONTRACT).toHexString(),
    "owner",
    Address.fromString(NEW_OWNER).toHexString()
  );
});

test("nameWrapper: handleNameWrapperApprovalForAll unpacks NameWrapper's `account` param (not `owner`)", () => {
  let mockEvent = newMockEvent();
  let event = new NameWrapperApprovalForAll(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    approvalParams("account", OWNER, OPERATOR, true),
    mockEvent.receipt
  );
  handleNameWrapperApprovalForAll(event);
  assert.fieldEquals("OperatorApproval", operatorApprovalId(CONTRACT, OWNER, OPERATOR), "approved", "true");
});

test("nameWrapper: handleNameWrapperOwnershipTransferred records the new owner", () => {
  let mockEvent = newMockEvent();
  let event = new NameWrapperOwnershipTransferred(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    ownershipParams(OWNER, NEW_OWNER),
    mockEvent.receipt
  );
  handleNameWrapperOwnershipTransferred(event);
  assert.fieldEquals(
    "ContractOwnership",
    Address.fromString(CONTRACT).toHexString(),
    "owner",
    Address.fromString(NEW_OWNER).toHexString()
  );
});

test("nameWrapper: handleControllerChanged sets RegistrarController.active from the event's own `active` param", () => {
  let mockEvent = newMockEvent();
  let params = new Array<ethereum.EventParam>();
  params.push(
    new ethereum.EventParam("controller", ethereum.Value.fromAddress(Address.fromString(CONTROLLER)))
  );
  params.push(new ethereum.EventParam("active", ethereum.Value.fromBoolean(true)));
  let event = new ControllerChanged(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    params,
    mockEvent.receipt
  );
  handleControllerChanged(event);
  assert.fieldEquals("RegistrarController", controllerId(CONTRACT, CONTROLLER), "active", "true");
});

test("ensv2Registry: handleENSv2ApprovalForAll bootstraps the registry and unpacks PermissionedRegistry's `account` param", () => {
  dataSourceMock.setNetwork("sepolia");
  let mockEvent = newMockEvent();
  let event = new ENSv2ApprovalForAll(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    approvalParams("account", OWNER, OPERATOR, true),
    mockEvent.receipt
  );
  handleENSv2ApprovalForAll(event);
  assert.fieldEquals("OperatorApproval", operatorApprovalId(CONTRACT, OWNER, OPERATOR), "approved", "true");
  assert.entityCount("ENSv2Registry", 1);
});

test("ensv2Registrar: handleETHRegistrarOwnershipTransferred records the new owner", () => {
  let mockEvent = newMockEvent();
  let event = new ETHRegistrarOwnershipTransferred(
    Address.fromString(CONTRACT),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    ownershipParams(OWNER, NEW_OWNER),
    mockEvent.receipt
  );
  handleETHRegistrarOwnershipTransferred(event);
  assert.fieldEquals(
    "ContractOwnership",
    Address.fromString(CONTRACT).toHexString(),
    "owner",
    Address.fromString(NEW_OWNER).toHexString()
  );
});
