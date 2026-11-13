import { Address, BigInt, Bytes, ethereum } from "@graphprotocol/graph-ts";
import {
  afterEach,
  assert,
  clearStore,
  dataSourceMock,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import { handleEACRolesChanged } from "../src/ensv2Registry";
import { handleProxyDeployed } from "../src/ensv2Discovery";
import {
  handleEACRolesChanged as handleResolverEACRolesChanged,
  handleResourceArgument,
} from "../src/ensv2Resolver";
import { EACRolesChanged } from "../src/types/RootRegistry/PermissionedRegistry";
import {
  EACRolesChanged as ResolverEACRolesChanged,
  ResourceArgument,
} from "../src/types/PermissionedResolver/PermissionedResolver";
import { ProxyDeployed } from "../src/types/VerifiableFactory/VerifiableFactory";
import { ENSv2Registry, ENSv2RoleAssignment } from "../src/types/schema";

const REGISTRY_ADDRESS = "0x33333333333333333333333333333333333333cc";
const RESOLVER_ADDRESS = "0x44444444444444444444444444444444444444dd";
const ACCOUNT = "0x55555555555555555555555555555555555555ee";
const FACTORY_ADDRESS = "0xD2a632D8a8b67c2c4398c255CbD7aF8dd7236198";
const SENDER = "0x89205A3A3b2A69De6Dbf7f01ED13B2108B2c43e7";

const createRegistryEACRolesChangedEvent = (
  contract: string,
  resource: BigInt,
  account: string,
  oldRoleBitmap: BigInt,
  newRoleBitmap: BigInt
): EACRolesChanged => {
  let mockEvent = newMockEvent();
  let event = new EACRolesChanged(
    Address.fromString(contract),
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
    new ethereum.EventParam("resource", ethereum.Value.fromUnsignedBigInt(resource))
  );
  event.parameters.push(
    new ethereum.EventParam("account", ethereum.Value.fromAddress(Address.fromString(account)))
  );
  event.parameters.push(
    new ethereum.EventParam("oldRoleBitmap", ethereum.Value.fromUnsignedBigInt(oldRoleBitmap))
  );
  event.parameters.push(
    new ethereum.EventParam("newRoleBitmap", ethereum.Value.fromUnsignedBigInt(newRoleBitmap))
  );
  return event;
};

const createResolverEACRolesChangedEvent = (
  contract: string,
  resource: BigInt,
  account: string,
  oldRoleBitmap: BigInt,
  newRoleBitmap: BigInt
): ResolverEACRolesChanged => {
  let mockEvent = newMockEvent();
  let event = new ResolverEACRolesChanged(
    Address.fromString(contract),
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
    new ethereum.EventParam("resource", ethereum.Value.fromUnsignedBigInt(resource))
  );
  event.parameters.push(
    new ethereum.EventParam("account", ethereum.Value.fromAddress(Address.fromString(account)))
  );
  event.parameters.push(
    new ethereum.EventParam("oldRoleBitmap", ethereum.Value.fromUnsignedBigInt(oldRoleBitmap))
  );
  event.parameters.push(
    new ethereum.EventParam("newRoleBitmap", ethereum.Value.fromUnsignedBigInt(newRoleBitmap))
  );
  return event;
};

const createResourceArgumentEvent = (
  resource: BigInt,
  arg: Bytes
): ResourceArgument => {
  let mockEvent = newMockEvent();
  let event = new ResourceArgument(
    Address.fromString(RESOLVER_ADDRESS),
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
      "resource",
      ethereum.Value.fromUnsignedBigInt(resource)
    )
  );
  event.parameters.push(
    new ethereum.EventParam("arg", ethereum.Value.fromBytes(arg))
  );
  return event;
};

const createProxyDeployedEvent = (proxyAddress: string): ProxyDeployed => {
  let mockEvent = newMockEvent();
  let event = new ProxyDeployed(
    Address.fromString(FACTORY_ADDRESS),
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
    new ethereum.EventParam("sender", ethereum.Value.fromAddress(Address.fromString(SENDER)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "proxyAddress",
      ethereum.Value.fromAddress(Address.fromString(proxyAddress))
    )
  );
  // "salt" (the real ABI's 3rd param, between proxyAddress and
  // implementation) was previously missing here, so event.params.implementation
  // (generated as a fixed-index accessor, not looked up by name) read past
  // the end of a 3-element array once handleProxyDeployed reads it.
  event.parameters.push(
    new ethereum.EventParam(
      "salt",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(1))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "implementation",
      ethereum.Value.fromAddress(Address.fromString(proxyAddress))
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
// 32-byte big-endian value (src/utils.ts::uint256ToByteArray) — this mirrors
// that exact encoding to reproduce the same hex string.
function bigIntHex32(i: BigInt): string {
  return i.toHex().slice(2).padStart(64, "0");
}

test("grant then revoke on the same (contract, resource, account) leaves the assignment at the latest bitmap and two history rows", () => {
  dataSourceMock.setNetwork("sepolia");
  let resource = BigInt.fromI32(1);
  let assignmentId = Address.fromString(REGISTRY_ADDRESS)
    .toHexString()
    .concat(bigIntHex32(resource))
    .concat(Address.fromString(ACCOUNT).toHexString().slice(2));

  let grantEvent = createRegistryEACRolesChangedEvent(
    REGISTRY_ADDRESS,
    resource,
    ACCOUNT,
    BigInt.fromI32(0),
    BigInt.fromI32(1)
  );
  handleEACRolesChanged(grantEvent);
  assert.fieldEquals("ENSv2RoleAssignment", assignmentId, "roleBitmap", "1");

  // newMockEvent() defaults to a fixed block.number/logIndex on every call,
  // so the second event needs a distinct logIndex or its ENSv2RoleChange
  // history row (id: block.number-logIndex) would collide with the first.
  let revokeEvent = createRegistryEACRolesChangedEvent(
    REGISTRY_ADDRESS,
    resource,
    ACCOUNT,
    BigInt.fromI32(1),
    BigInt.fromI32(0)
  );
  revokeEvent.logIndex = grantEvent.logIndex.plus(BigInt.fromI32(1));
  handleEACRolesChanged(revokeEvent);
  assert.fieldEquals("ENSv2RoleAssignment", assignmentId, "roleBitmap", "0");

  assert.entityCount("ENSv2RoleChange", 2);
  let firstHistoryId = "0x"
    .concat(bigIntHex32(grantEvent.block.number))
    .concat(bigIntHex32(grantEvent.logIndex));
  let secondHistoryId = "0x"
    .concat(bigIntHex32(revokeEvent.block.number))
    .concat(bigIntHex32(revokeEvent.logIndex));
  assert.fieldEquals("ENSv2RoleChange", firstHistoryId, "newRoleBitmap", "1");
  assert.fieldEquals("ENSv2RoleChange", secondHistoryId, "newRoleBitmap", "0");
});

test("a registry-side role event with no matching ENSv2Resource indexes with resourceEntity null", () => {
  dataSourceMock.setNetwork("sepolia");
  let resource = BigInt.fromI32(2);
  let assignmentId = Address.fromString(REGISTRY_ADDRESS)
    .toHexString()
    .concat(bigIntHex32(resource))
    .concat(Address.fromString(ACCOUNT).toHexString().slice(2));

  handleEACRolesChanged(
    createRegistryEACRolesChangedEvent(
      REGISTRY_ADDRESS,
      resource,
      ACCOUNT,
      BigInt.fromI32(0),
      BigInt.fromI32(1)
    )
  );

  let assignment = ENSv2RoleAssignment.load(Bytes.fromHexString(assignmentId));
  assert.assertNotNull(assignment);
  if (assignment != null) {
    let resourceEntityId = assignment.resourceEntity;
    assert.assertTrue(!resourceEntityId);
  }
});

test("a resolver-side role event also indexes with resourceEntity null, proving no contract-type branching is needed", () => {
  dataSourceMock.setNetwork("sepolia");
  let resource = BigInt.fromI32(3);
  let assignmentId = Address.fromString(RESOLVER_ADDRESS)
    .toHexString()
    .concat(bigIntHex32(resource))
    .concat(Address.fromString(ACCOUNT).toHexString().slice(2));

  handleResolverEACRolesChanged(
    createResolverEACRolesChangedEvent(
      RESOLVER_ADDRESS,
      resource,
      ACCOUNT,
      BigInt.fromI32(0),
      BigInt.fromI32(1)
    )
  );

  let assignment = ENSv2RoleAssignment.load(Bytes.fromHexString(assignmentId));
  assert.assertNotNull(assignment);
  if (assignment != null) {
    let resourceEntityId = assignment.resourceEntity;
    assert.assertTrue(!resourceEntityId);
  }
  assert.fieldEquals("ENSv2RoleAssignment", assignmentId, "roleBitmap", "1");
});

test("ResourceArgument immediately preceding a resolver role event enriches the assignment", () => {
  dataSourceMock.setNetwork("sepolia");
  let resource = BigInt.fromI32(30);
  let assignmentId = Address.fromString(RESOLVER_ADDRESS)
    .toHexString()
    .concat(bigIntHex32(resource))
    .concat(Address.fromString(ACCOUNT).toHexString().slice(2));
  let argumentId = Address.fromString(RESOLVER_ADDRESS)
    .toHexString()
    .concat(bigIntHex32(resource));

  handleResourceArgument(
    createResourceArgumentEvent(resource, Bytes.fromUTF8("avatar"))
  );
  handleResolverEACRolesChanged(
    createResolverEACRolesChangedEvent(
      RESOLVER_ADDRESS,
      resource,
      ACCOUNT,
      BigInt.zero(),
      BigInt.fromI32(1)
    )
  );

  assert.fieldEquals(
    "ENSv2RoleAssignment",
    assignmentId,
    "resourceArgument",
    argumentId
  );
});

test("root resolver roles legitimately have no ResourceArgument relation", () => {
  dataSourceMock.setNetwork("sepolia");
  let resource = BigInt.zero();
  let assignmentId = Address.fromString(RESOLVER_ADDRESS)
    .toHexString()
    .concat(bigIntHex32(resource))
    .concat(Address.fromString(ACCOUNT).toHexString().slice(2));

  handleResolverEACRolesChanged(
    createResolverEACRolesChangedEvent(
      RESOLVER_ADDRESS,
      resource,
      ACCOUNT,
      BigInt.zero(),
      BigInt.fromI32(1)
    )
  );
  let assignment = ENSv2RoleAssignment.load(Bytes.fromHexString(assignmentId));
  assert.assertNotNull(assignment);
  if (assignment != null) {
    assert.assertTrue(!assignment.resourceArgument);
  }
});

test("role-event-then-ProxyDeployed and ProxyDeployed-then-role-event produce identical final registry/assignment state", () => {
  dataSourceMock.setNetwork("sepolia");
  let resource = BigInt.fromI32(4);
  let registryId = Address.fromString(REGISTRY_ADDRESS).toHexString();
  let assignmentId = registryId
    .concat(bigIntHex32(resource))
    .concat(Address.fromString(ACCOUNT).toHexString().slice(2));

  // Order 1: role event first, then discovery.
  handleEACRolesChanged(
    createRegistryEACRolesChangedEvent(
      REGISTRY_ADDRESS,
      resource,
      ACCOUNT,
      BigInt.fromI32(0),
      BigInt.fromI32(1)
    )
  );
  handleProxyDeployed(createProxyDeployedEvent(REGISTRY_ADDRESS));

  assert.entityCount("ENSv2Registry", 1);
  assert.fieldEquals("ENSv2RoleAssignment", assignmentId, "roleBitmap", "1");

  clearStore();

  // Order 2: discovery first, then role event.
  handleProxyDeployed(createProxyDeployedEvent(REGISTRY_ADDRESS));
  handleEACRolesChanged(
    createRegistryEACRolesChangedEvent(
      REGISTRY_ADDRESS,
      resource,
      ACCOUNT,
      BigInt.fromI32(0),
      BigInt.fromI32(1)
    )
  );

  assert.entityCount("ENSv2Registry", 1);
  assert.fieldEquals("ENSv2RoleAssignment", assignmentId, "roleBitmap", "1");
});
