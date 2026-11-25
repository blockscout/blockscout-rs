// v2.3.5: a registry that was not deployed through VerifiableFactory is
// indexed from its own first LabelRegistered, LabelReserved or ParentUpdated,
// which the addressless ENSv2RegistryDiscovery source sees
// (ensv2Discovery.ts). Being named by another registry is not enough: any
// address can be named.
import { Address, ByteArray, Bytes, BigInt, crypto, ethereum } from "@graphprotocol/graph-ts";
import {
  assert,
  dataSourceMock,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import {
  handleProxyDeployed,
  handleRegistryLabelRegistered,
  handleRegistryParentUpdated,
} from "../src/ensv2Discovery";
import {
  handleLabelRegistered,
  handleParentUpdated,
  handleSubregistryUpdated,
} from "../src/ensv2Registry";
import { ParentUpdated } from "../src/types/RootRegistry/PermissionedRegistry";
import { pathNamehash } from "../src/ensv2Utils";
import { ProxyDeployed } from "../src/types/VerifiableFactory/VerifiableFactory";
import {
  createLabelRegisteredEvent,
  createSubregistryUpdatedEvent,
  slotToken,
} from "./ensv2TestEvents";

const ROOT_REGISTRY = "0xB458D6a3a77919449d03e7A6903C26827c1eC43f";
const ETH_REGISTRY = "0xD4eBcbBdF463C9c45784603Db0dDD499BC44A8B4";
const FACTORY_ADDRESS = "0xD2a632D8a8b67c2c4398c255CbD7aF8dd7236198";
const USER_REGISTRY_IMPL = "0x9BD8a88719068D09ecee662f36C0E3856708366a";
const OWNER = "0x89205A3A3b2A69De6Dbf7f01ED13B2108B2c43e7";
const ROOT_NODE = Bytes.fromHexString(
  "0x0000000000000000000000000000000000000000000000000000000000000000"
);
const TEMPLATE = "ENSv2Registry";

function lh(label: string): Bytes {
  return Bytes.fromByteArray(crypto.keccak256(ByteArray.fromUTF8(label)));
}

function child(parent: Bytes, label: string): Bytes {
  return pathNamehash(parent, lh(label));
}

function hex(address: string): string {
  return Address.fromString(address).toHexString();
}

function proxyDeployed(proxy: string, implementation: string): ProxyDeployed {
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
    new ethereum.EventParam("sender", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  event.parameters.push(
    new ethereum.EventParam("proxyAddress", ethereum.Value.fromAddress(Address.fromString(proxy)))
  );
  event.parameters.push(
    new ethereum.EventParam("salt", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(1)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "implementation",
      ethereum.Value.fromAddress(Address.fromString(implementation))
    )
  );
  return event;
}

// This file never clears the store or the created data sources: the tests
// build on each other and each uses its own addresses.

test("the static RootRegistry and ETHRegistry never get a dynamic data source", () => {
  dataSourceMock.setNetwork("sepolia");
  // Their first name events also reach the discovery source.
  handleRegistryLabelRegistered(
    createLabelRegisteredEvent(ROOT_REGISTRY, slotToken(1), lh("eth"), "eth")
  );
  handleLabelRegistered(
    createLabelRegisteredEvent(ROOT_REGISTRY, slotToken(1), lh("eth"), "eth")
  );
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), ETH_REGISTRY)
  );
  handleRegistryLabelRegistered(
    createLabelRegisteredEvent(ETH_REGISTRY, slotToken(1), lh("x"), "x")
  );
  handleLabelRegistered(
    createLabelRegisteredEvent(ETH_REGISTRY, slotToken(1), lh("x"), "x")
  );
  // Someone pointing their own name at the ETHRegistry does not start one
  // either.
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ETH_REGISTRY, slotToken(1), ETH_REGISTRY)
  );

  assert.fieldEquals("ENSv2Registry", hex(ETH_REGISTRY), "kind", "ETH");
  assert.dataSourceCount(TEMPLATE, 0);
});

function parentUpdated(registry: string, parent: string, label: string): ParentUpdated {
  let mockEvent = newMockEvent();
  let event = new ParentUpdated(
    Address.fromString(registry),
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
  event.parameters.push(new ethereum.EventParam("label", ethereum.Value.fromString(label)));
  event.parameters.push(
    new ethereum.EventParam("sender", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  return event;
}

test("an address a name points at gets a row but no data source until it emits a name event", () => {
  dataSourceMock.setNetwork("sepolia");
  const CUSTOM = "0xd1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1";
  handleLabelRegistered(
    createLabelRegisteredEvent(ETH_REGISTRY, slotToken(2), lh("y"), "y")
  );
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ETH_REGISTRY, slotToken(2), CUSTOM)
  );

  // Any address can be named, so naming one starts nothing.
  assert.fieldEquals("ENSv2Registry", hex(CUSTOM), "kind", "UNKNOWN");
  assert.fieldEquals("ENSv2Registry", hex(CUSTOM), "indexed", "false");
  assert.dataSourceCount(TEMPLATE, 0);

  // Its first name event does. graph-node hands the block's events to the
  // new data source, so the same LabelRegistered then reaches the registry
  // mapping.
  handleRegistryLabelRegistered(
    createLabelRegisteredEvent(CUSTOM, slotToken(1), lh("sub"), "sub")
  );
  assert.fieldEquals("ENSv2Registry", hex(CUSTOM), "indexed", "true");
  assert.dataSourceCount(TEMPLATE, 1);
  assert.dataSourceExists(TEMPLATE, hex(CUSTOM));
  handleLabelRegistered(
    createLabelRegisteredEvent(CUSTOM, slotToken(1), lh("sub"), "sub")
  );
  let yNode = child(child(ROOT_NODE, "eth"), "y");
  assert.fieldEquals("Domain", child(yNode, "sub").toHexString(), "name", "sub.y.eth");

  // It is not started twice.
  handleRegistryLabelRegistered(
    createLabelRegisteredEvent(CUSTOM, slotToken(2), lh("sub2"), "sub2")
  );
  assert.dataSourceCount(TEMPLATE, 1);
});

test("a registry that registers names before anything links it is discovered by its first name event", () => {
  dataSourceMock.setNetwork("sepolia");
  const EARLY = "0xd2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2";
  assert.notInStore("ENSv2Registry", hex(EARLY));

  handleRegistryLabelRegistered(
    createLabelRegisteredEvent(EARLY, slotToken(1), lh("first"), "first")
  );
  assert.fieldEquals("ENSv2Registry", hex(EARLY), "kind", "UNKNOWN");
  assert.fieldEquals("ENSv2Registry", hex(EARLY), "indexed", "true");
  assert.dataSourceCount(TEMPLATE, 2);
  assert.dataSourceExists(TEMPLATE, hex(EARLY));
  handleLabelRegistered(
    createLabelRegisteredEvent(EARLY, slotToken(1), lh("first"), "first")
  );

  // The late link then finds the name.
  handleLabelRegistered(
    createLabelRegisteredEvent(ETH_REGISTRY, slotToken(3), lh("z"), "z")
  );
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ETH_REGISTRY, slotToken(3), EARLY)
  );
  assert.dataSourceCount(TEMPLATE, 2);
  let zNode = child(child(ROOT_NODE, "eth"), "z");
  assert.fieldEquals("Domain", child(zNode, "first").toHexString(), "name", "first.z.eth");
});

test("a registry that declares its parent before it has a name is discovered by that declaration", () => {
  dataSourceMock.setNetwork("sepolia");
  const DECLARING = "0xd5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5";
  handleRegistryParentUpdated(parentUpdated(DECLARING, ETH_REGISTRY, "v"));
  assert.dataSourceCount(TEMPLATE, 3);
  assert.dataSourceExists(TEMPLATE, hex(DECLARING));
  handleParentUpdated(parentUpdated(DECLARING, ETH_REGISTRY, "v"));
  assert.fieldEquals("ENSv2Registry", hex(DECLARING), "canonicalParentLabel", "v");
  // The parent it names is only named.
  assert.dataSourceCount(TEMPLATE, 3);
});

test("ProxyDeployed starts a registry that was only named, and leaves one that is already indexed alone", () => {
  dataSourceMock.setNetwork("sepolia");
  // A name can point at a proxy's address before the proxy is deployed.
  const COUNTERFACTUAL = "0xd3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3";
  handleLabelRegistered(
    createLabelRegisteredEvent(ETH_REGISTRY, slotToken(4), lh("w"), "w")
  );
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ETH_REGISTRY, slotToken(4), COUNTERFACTUAL)
  );
  assert.dataSourceCount(TEMPLATE, 3);

  handleProxyDeployed(proxyDeployed(COUNTERFACTUAL, USER_REGISTRY_IMPL));
  assert.fieldEquals("ENSv2Registry", hex(COUNTERFACTUAL), "kind", "USER");
  assert.fieldEquals("ENSv2Registry", hex(COUNTERFACTUAL), "indexed", "true");
  assert.dataSourceCount(TEMPLATE, 4);
  assert.dataSourceExists(TEMPLATE, hex(COUNTERFACTUAL));

  // A proxy whose initializer emitted a name or parent event before the
  // factory's ProxyDeployed was started by that event.
  const INITIALISED = "0xd4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4";
  handleRegistryParentUpdated(parentUpdated(INITIALISED, ETH_REGISTRY, "w"));
  assert.dataSourceCount(TEMPLATE, 5);
  handleProxyDeployed(proxyDeployed(INITIALISED, USER_REGISTRY_IMPL));
  assert.fieldEquals("ENSv2Registry", hex(INITIALISED), "kind", "USER");
  assert.dataSourceCount(TEMPLATE, 5);
});
