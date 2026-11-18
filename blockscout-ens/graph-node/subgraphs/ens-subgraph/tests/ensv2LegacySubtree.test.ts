// v2.3.5: ENSv1 names below an ENSv2 name (legacyGovernance.ts rule 3).
//
// ENSv2 resolves a name from its registries. Once a name is REGISTERED in
// ENSv2, an ENSv1 name below it that ENSv2 does not hold is answered by the
// ENSv2 name's resolver or not at all, never by its ENSv1 resolver. Its
// Domain row must stop showing the ENSv1 resolver and address. The one case
// that keeps resolving through ENSv1 is an emancipated NameWrapper subname
// below a locked name that migrated into a WrapperRegistry.
import {
  Address,
  BigInt,
  ByteArray,
  Bytes,
  crypto,
  ethereum,
} from "@graphprotocol/graph-ts";
import {
  afterEach,
  assert,
  clearStore,
  dataSourceMock,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import {
  handleLabelRegistered,
  handleLabelReserved,
  handleLabelUnregistered,
  handleParentUpdated,
  handleSubregistryUpdated,
} from "../src/ensv2Registry";
import {
  legacySweepBudgetForTesting,
  MAX_LEGACY_ROWS_VOIDED,
  setMaxLegacyRowsVoidedForTesting,
} from "../src/legacyGovernance";
import {
  handleProxyDeployed,
  handleRegistryParentUpdated,
} from "../src/ensv2Discovery";
import {
  LabelReserved,
  ParentUpdated,
} from "../src/types/RootRegistry/PermissionedRegistry";
import { handleNewResolver, handleTransfer } from "../src/ensRegistry";
import { createResolverID } from "../src/resolver";
import {
  MAX_NAMES_BELOW_NAME,
  setMaxNamesBelowNameForTesting,
} from "../src/ensv2Paths";
import { pathNamehash } from "../src/ensv2Utils";
import {
  Account,
  Domain,
  Resolver,
  WrappedDomain,
} from "../src/types/schema";
import { NewResolver, Transfer } from "../src/types/ENSRegistry/EnsRegistry";
import { ProxyDeployed } from "../src/types/VerifiableFactory/VerifiableFactory";
import {
  createLabelRegisteredEvent,
  createLabelUnregisteredEvent,
  createSubregistryUpdatedEvent,
  SENDER,
} from "./ensv2TestEvents";

const ROOT_REGISTRY = "0xB458D6a3a77919449d03e7A6903C26827c1eC43f";
const ETH_REGISTRY = "0xD4eBcbBdF463C9c45784603Db0dDD499BC44A8B4";
const FACTORY = "0xD2a632D8a8b67c2c4398c255CbD7aF8dd7236198";
// Must match ensv2Constants.ts's sepolia values.
const WRAPPER_REGISTRY_IMPL = "0xBe768b63E5fBBFBB0Ae97E9064E0002dF8001880";
const LOCKED_MIGRATION_CONTROLLER = "0x6029a063d69b09D23c52a754a90E4FE43aDac3A8";
const UNLOCKED_MIGRATION_CONTROLLER = "0x2a35B94DF22cc7354570be2284655E2CDC0e64A2";
const NAME_WRAPPER = "0xd4416b13d2b3a9abae7acd5d6c2bbdbe25686401";
const WRAPPER_REGISTRY = "0xa1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
const USER_REGISTRY = "0xb2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2";
const OWNER = "0x89205A3A3b2A69De6Dbf7f01ED13B2108B2c43e7";
const OWNER_2 = "0xF0205A3A3b2A69De6Dbf7f01ED13B2108B2c4321";
const V1_RESOLVER = "0x4976fb03C32e5B8cfe2b6cCB31c09Ba78EBaBa41";
const OTHER_V1_RESOLVER = "0x231b0Ee14048e9dCcD1d247744d114a4EB5E8E63";
const V1_ADDRESS = "0x5555555555555555555555555555555555555501";
const EMPTY = "0x0000000000000000000000000000000000000000";
const ROOT_NODE = Bytes.fromHexString(
  "0x0000000000000000000000000000000000000000000000000000000000000000"
);

// NameWrapper fuses.
const CANNOT_UNWRAP = 1;
const PARENT_CANNOT_CONTROL = 65536;
const IS_DOT_ETH = 131072;
const LOCKED_ETH_NAME = CANNOT_UNWRAP | PARENT_CANNOT_CONTROL | IS_DOT_ETH;
const FAR_FUTURE = BigInt.fromI64(4000000000);
// After the ENSv2 deployment block: the governance rules do not apply to the
// ENSv1 history before it.
const V2_ERA_BLOCK = BigInt.fromI32(20000000);

function lh(label: string): Bytes {
  return Bytes.fromByteArray(crypto.keccak256(ByteArray.fromUTF8(label)));
}

function tokenOf(label: string): BigInt {
  let hash = lh(label);
  let le = new Bytes(32);
  for (let i = 0; i < 32; i++) {
    le[i] = hash[31 - i];
  }
  return BigInt.fromUnsignedBytes(le);
}

function child(parent: Bytes, label: string): Bytes {
  return pathNamehash(parent, lh(label));
}

function register(
  registry: string,
  label: string,
  owner: string = OWNER,
  sender: string = SENDER
): void {
  handleLabelRegistered(
    createLabelRegisteredEvent(registry, tokenOf(label), lh(label), label, owner, sender)
  );
}

function link(registry: string, label: string, subregistry: string): void {
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(registry, tokenOf(label), subregistry)
  );
}

function deployWrapperRegistry(address: string): void {
  let mockEvent = newMockEvent();
  let event = new ProxyDeployed(
    Address.fromString(FACTORY),
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
      ethereum.Value.fromAddress(Address.fromString(address))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("salt", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(1)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "implementation",
      ethereum.Value.fromAddress(Address.fromString(WRAPPER_REGISTRY_IMPL))
    )
  );
  handleProxyDeployed(event);
}

// An ENSv1 Domain row as the ENSv1 handlers leave it. `withAddress` gives it
// a resolver and the address it resolves to.
function seedLegacyName(parent: Bytes, label: string, withAddress: boolean = true): string {
  let node = child(parent, label);
  let owner = new Account(OWNER.toLowerCase());
  owner.save();
  let domain = new Domain(node.toHexString());
  domain.storedOffchain = false; // Blockscout/BENS extension
  domain.resolvedWithWildcard = false; // Blockscout/BENS extension
  domain.labelName = label;
  domain.labelhash = lh(label);
  domain.parent = parent.toHexString();
  domain.subdomainCount = 0;
  domain.isMigrated = true;
  domain.createdAt = BigInt.zero();
  domain.owner = owner.id;
  if (withAddress) {
    let target = new Account(V1_ADDRESS);
    target.save();
    let resolverId = createResolverID(node, Address.fromString(V1_RESOLVER));
    let legacy = new Resolver(resolverId);
    legacy.domain = node.toHexString();
    legacy.address = Address.fromString(V1_RESOLVER);
    legacy.addr = target.id;
    legacy.save();
    domain.resolver = resolverId;
    domain.resolvedAddress = target.id;
  }
  domain.save();
  let parentDomain = Domain.load(parent.toHexString());
  if (parentDomain != null) {
    parentDomain.subdomainCount = parentDomain.subdomainCount + 1;
    parentDomain.save();
  }
  return node.toHexString();
}

// Wraps a seeded ENSv1 name: the registry owner is the NameWrapper.
function wrap(node: string, fuses: i32, expiry: BigInt = FAR_FUTURE): void {
  let wrapperAccount = new Account(NAME_WRAPPER);
  wrapperAccount.save();
  let domain = Domain.load(node)!;
  domain.owner = NAME_WRAPPER;
  domain.wrappedOwner = OWNER.toLowerCase();
  domain.expiryDate = expiry;
  domain.save();
  let wrapped = new WrappedDomain(node);
  wrapped.domain = node;
  wrapped.expiryDate = expiry;
  wrapped.fuses = fuses;
  wrapped.owner = OWNER.toLowerCase();
  wrapped.save();
}

function v1NewResolver(node: string, resolver: string): NewResolver {
  let mockEvent = newMockEvent();
  let event = new NewResolver(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.block.number = V2_ERA_BLOCK;
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "node",
      ethereum.Value.fromFixedBytes(Bytes.fromHexString(node))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "resolver",
      ethereum.Value.fromAddress(Address.fromString(resolver))
    )
  );
  return event;
}

function v1Transfer(node: string, owner: string): Transfer {
  let mockEvent = newMockEvent();
  let event = new Transfer(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.block.number = V2_ERA_BLOCK;
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "node",
      ethereum.Value.fromFixedBytes(Bytes.fromHexString(node))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("owner", ethereum.Value.fromAddress(Address.fromString(owner)))
  );
  return event;
}

function resolverIdOf(node: string, resolver: string): string {
  return createResolverID(Bytes.fromHexString(node), Address.fromString(resolver));
}

function assertHasV1Address(node: string): void {
  assert.fieldEquals("Domain", node, "resolver", resolverIdOf(node, V1_RESOLVER));
  assert.fieldEquals("Domain", node, "resolvedAddress", V1_ADDRESS);
}

function assertNoAddress(node: string): void {
  assert.fieldEquals("Domain", node, "resolver", "null");
  assert.fieldEquals("Domain", node, "resolvedAddress", "null");
}

// "eth" in the RootRegistry, the ETHRegistry below it, and an ENSv1 row for
// x.eth that ENSv2 has not registered yet.
function setUpV1Name(): Bytes {
  dataSourceMock.setNetwork("sepolia");
  let ethNode = child(ROOT_NODE, "eth");
  let eth = new Domain(ethNode.toHexString());
  eth.storedOffchain = false; // Blockscout/BENS extension
  eth.resolvedWithWildcard = false; // Blockscout/BENS extension
  eth.subdomainCount = 0;
  eth.isMigrated = true;
  eth.createdAt = BigInt.zero();
  eth.owner = EMPTY;
  eth.parent = ROOT_NODE.toHexString();
  let zero = new Account(EMPTY);
  zero.save();
  eth.save();
  register(ROOT_REGISTRY, "eth");
  link(ROOT_REGISTRY, "eth", ETH_REGISTRY);
  return Bytes.fromHexString(seedLegacyName(ethNode, "x", false));
}

afterEach(() => {
  setMaxLegacyRowsVoidedForTesting(MAX_LEGACY_ROWS_VOIDED);
  setMaxNamesBelowNameForTesting(MAX_NAMES_BELOW_NAME);
  clearStore();
  dataSourceMock.resetValues();
});

// ---------------------------------------------------------------------------
// A name registered in ENSv2
// ---------------------------------------------------------------------------

test("an unmigrated name leaves its ENSv1 subnames alone", () => {
  let xNode = setUpV1Name();
  let sub = seedLegacyName(xNode, "sub");

  handleNewResolver(v1NewResolver(sub, OTHER_V1_RESOLVER));

  assert.fieldEquals("Domain", sub, "resolver", resolverIdOf(sub, OTHER_V1_RESOLVER));
});

test("a migration clears the ENSv1 names below the migrated name, at every depth, for good", () => {
  let xNode = setUpV1Name();
  let sub = seedLegacyName(xNode, "sub");
  let deep = seedLegacyName(Bytes.fromHexString(sub), "deep");
  let deeper = seedLegacyName(Bytes.fromHexString(deep), "deeper");
  let bare = seedLegacyName(xNode, "bare", false);

  register(ETH_REGISTRY, "x", OWNER, UNLOCKED_MIGRATION_CONTROLLER);

  assert.fieldEquals("ENSv2NamePath", xNode.toHexString(), "legacySubtreeVoided", "true");
  assertNoAddress(sub);
  assertNoAddress(deep);
  assertNoAddress(deeper);
  // The rows themselves stay: who owned them in ENSv1 is still true.
  assert.fieldEquals("Domain", sub, "owner", OWNER.toLowerCase());
  assert.fieldEquals("Domain", bare, "owner", OWNER.toLowerCase());
  assert.fieldEquals("Domain", xNode.toHexString(), "owner", OWNER.toLowerCase());

  // ENSv1 cannot put a resolver back, at any depth. The event is recorded.
  handleNewResolver(v1NewResolver(sub, OTHER_V1_RESOLVER));
  assert.entityCount("NewResolver", 1);
  handleNewResolver(v1NewResolver(deeper, OTHER_V1_RESOLVER));
  handleNewResolver(v1NewResolver(bare, V1_RESOLVER));
  assertNoAddress(sub);
  assertNoAddress(deeper);
  // (A row that never had a resolver has no such field to compare.)
  assert.assertNull(Domain.load(bare)!.resolver);
});

test("a name registered again in ENSv2 after it lapsed clears its old ENSv1 subnames too", () => {
  let xNode = setUpV1Name();
  let sub = seedLegacyName(xNode, "sub");

  register(ETH_REGISTRY, "x", OWNER_2);

  assert.fieldEquals("ENSv2NamePath", xNode.toHexString(), "legacySubtreeVoided", "true");
  assertNoAddress(sub);
});

test("a subname registered in ENSv2 is written by ENSv2, and the ENSv1 names below it are cleared", () => {
  let xNode = setUpV1Name();
  let sub = seedLegacyName(xNode, "sub");
  let deep = seedLegacyName(Bytes.fromHexString(sub), "deep");
  register(ETH_REGISTRY, "x", OWNER, UNLOCKED_MIGRATION_CONTROLLER);

  register(USER_REGISTRY, "sub", OWNER_2);
  link(ETH_REGISTRY, "x", USER_REGISTRY);

  assert.fieldEquals("ENSv2NamePath", sub, "active", "true");
  assert.fieldEquals("Domain", sub, "owner", OWNER_2.toLowerCase());
  assert.fieldEquals("ENSv2NamePath", sub, "legacySubtreeVoided", "true");
  assertNoAddress(deep);
  handleNewResolver(v1NewResolver(deep, OTHER_V1_RESOLVER));
  assertNoAddress(deep);
});

test("the mark outlives the name: ENSv1 names below a retired ENSv2 name still take no resolver", () => {
  let xNode = setUpV1Name();
  let sub = seedLegacyName(xNode, "sub");
  register(ETH_REGISTRY, "x");
  handleLabelUnregistered(createLabelUnregisteredEvent(ETH_REGISTRY, tokenOf("x")));
  assert.fieldEquals("ENSv2NamePath", xNode.toHexString(), "active", "false");

  handleNewResolver(v1NewResolver(sub, OTHER_V1_RESOLVER));

  assertNoAddress(sub);
});

test("a name that is new in ENSv2 is marked without looking for ENSv1 names", () => {
  dataSourceMock.setNetwork("sepolia");
  register(ROOT_REGISTRY, "eth");
  link(ROOT_REGISTRY, "eth", ETH_REGISTRY);
  register(ETH_REGISTRY, "fresh");

  let node = child(child(ROOT_NODE, "eth"), "fresh");
  assert.fieldEquals("ENSv2NamePath", node.toHexString(), "legacySubtreeVoided", "true");
  // The RootRegistry name has no projection and marks nothing.
  assert.fieldEquals(
    "ENSv2NamePath",
    child(ROOT_NODE, "eth").toHexString(),
    "legacySubtreeVoided",
    "false"
  );
});

// ---------------------------------------------------------------------------
// A locked NameWrapper name migrated into a WrapperRegistry
// ---------------------------------------------------------------------------

class LockedFixture {
  xNode: Bytes = ROOT_NODE;
  free: string = "";
  belowFree: string = "";
  plain: string = "";
  belowPlain: string = "";
}

// x.eth is locked in the NameWrapper. free.x.eth is emancipated, plain.x.eth
// is wrapped but not emancipated; both have an ENSv1 name below them.
function seedLockedName(): LockedFixture {
  let f = new LockedFixture();
  f.xNode = setUpV1Name();
  wrap(f.xNode.toHexString(), LOCKED_ETH_NAME);
  f.free = seedLegacyName(f.xNode, "free");
  wrap(f.free, PARENT_CANNOT_CONTROL);
  f.belowFree = seedLegacyName(Bytes.fromHexString(f.free), "below");
  f.plain = seedLegacyName(f.xNode, "plain");
  wrap(f.plain, 0);
  f.belowPlain = seedLegacyName(Bytes.fromHexString(f.plain), "below");
  return f;
}

// LockedMigrationController: deploy the WrapperRegistry, register the name,
// link the registry, in that order.
function migrateLocked(): void {
  deployWrapperRegistry(WRAPPER_REGISTRY);
  register(ETH_REGISTRY, "x", OWNER, LOCKED_MIGRATION_CONTROLLER);
  link(ETH_REGISTRY, "x", WRAPPER_REGISTRY);
}

test("a locked migration keeps its emancipated ENSv1 subnames and clears the rest", () => {
  let f = seedLockedName();

  migrateLocked();

  assert.fieldEquals("ENSv2Registry", WRAPPER_REGISTRY, "kind", "WRAPPER");
  assert.fieldEquals("ENSv2NamePath", f.xNode.toHexString(), "legacySubtreeVoided", "false");
  assertHasV1Address(f.free);
  assertHasV1Address(f.belowFree);
  assertNoAddress(f.plain);
  assertNoAddress(f.belowPlain);

  // ENSv1 still writes the emancipated subname and what is below it...
  handleNewResolver(v1NewResolver(f.free, OTHER_V1_RESOLVER));
  handleNewResolver(v1NewResolver(f.belowFree, OTHER_V1_RESOLVER));
  assert.fieldEquals("Domain", f.free, "resolver", resolverIdOf(f.free, OTHER_V1_RESOLVER));
  assert.fieldEquals(
    "Domain",
    f.belowFree,
    "resolver",
    resolverIdOf(f.belowFree, OTHER_V1_RESOLVER)
  );
  // ...and nothing else below the migrated name.
  handleNewResolver(v1NewResolver(f.plain, OTHER_V1_RESOLVER));
  handleNewResolver(v1NewResolver(f.belowPlain, OTHER_V1_RESOLVER));
  assertNoAddress(f.plain);
  assertNoAddress(f.belowPlain);
});

// Mainnet order (tx 0x1171951a…0619): the WrapperRegistry's initialize()
// emits ParentUpdated before the factory emits ProxyDeployed, so the registry
// is discovered without an implementation first.
test("a WrapperRegistry discovered through its own ParentUpdated before its ProxyDeployed is still recognised", () => {
  let f = seedLockedName();
  let mockEvent = newMockEvent();
  let declared = new ParentUpdated(
    Address.fromString(WRAPPER_REGISTRY),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  declared.parameters = new Array();
  declared.parameters.push(
    new ethereum.EventParam(
      "parent",
      ethereum.Value.fromAddress(Address.fromString(ETH_REGISTRY))
    )
  );
  declared.parameters.push(
    new ethereum.EventParam("label", ethereum.Value.fromString("x"))
  );
  declared.parameters.push(
    new ethereum.EventParam("sender", ethereum.Value.fromAddress(Address.fromString(EMPTY)))
  );
  handleRegistryParentUpdated(declared);
  handleParentUpdated(declared);
  assert.fieldEquals("ENSv2Registry", WRAPPER_REGISTRY, "kind", "UNKNOWN");

  migrateLocked();

  assert.fieldEquals("ENSv2Registry", WRAPPER_REGISTRY, "kind", "WRAPPER");
  assert.fieldEquals("ENSv2NamePath", f.xNode.toHexString(), "legacySubtreeVoided", "false");
  assertHasV1Address(f.free);
  assertNoAddress(f.plain);
});

test("an emancipated subname whose wrapper expiry has passed is not kept", () => {
  let f = seedLockedName();
  let lapsed = seedLegacyName(f.xNode, "lapsed");
  wrap(lapsed, PARENT_CANNOT_CONTROL, BigInt.zero());

  migrateLocked();

  assertNoAddress(lapsed);
  assertHasV1Address(f.free);
});

test("an emancipated subname that leaves the NameWrapper is cut off, with everything below it, for good", () => {
  let f = seedLockedName();
  migrateLocked();

  // NameWrapper.unwrap: the registry owner moves from the wrapper to the
  // controller, which detaches the wrapped state.
  handleTransfer(v1Transfer(f.free, OWNER_2));

  assert.fieldEquals("ENSv2LegacyCutoff", f.free, "blockNumber", V2_ERA_BLOCK.toString());
  assert.fieldEquals("Domain", f.free, "owner", OWNER_2.toLowerCase());
  assertNoAddress(f.free);
  assertNoAddress(f.belowFree);

  // Wrapped again with the same fuses: still cut off.
  wrap(f.free, PARENT_CANNOT_CONTROL);
  handleNewResolver(v1NewResolver(f.free, OTHER_V1_RESOLVER));
  handleNewResolver(v1NewResolver(f.belowFree, OTHER_V1_RESOLVER));
  assertNoAddress(f.free);
  assertNoAddress(f.belowFree);
});

test("an emancipated subname that migrates is written by ENSv2, and the ENSv1 names below it are cleared", () => {
  let f = seedLockedName();
  migrateLocked();

  // WrapperRegistry._inject: the registry registers in itself.
  register(WRAPPER_REGISTRY, "free", OWNER_2, WRAPPER_REGISTRY);

  assert.fieldEquals("ENSv2NamePath", f.free, "active", "true");
  assert.fieldEquals("ENSv2NamePath", f.free, "legacySubtreeVoided", "true");
  assert.fieldEquals("Domain", f.free, "owner", OWNER_2.toLowerCase());
  assertNoAddress(f.free);
  assertNoAddress(f.belowFree);
  handleNewResolver(v1NewResolver(f.belowFree, OTHER_V1_RESOLVER));
  assertNoAddress(f.belowFree);
});

// A name left out of the projection because a name above it is full
// (ensv2Paths.ts, ENSv2SubtreeSize) is still registered in the
// WrapperRegistry, which stops sending it to ENSv1. Its ENSv1 row cannot
// stay, and with a name missing the locked name is treated like any other.
test("an emancipated subname that migrates but is not projected loses its ENSv1 address all the same", () => {
  let f = seedLockedName();
  let other = seedLegacyName(f.xNode, "other");
  wrap(other, PARENT_CANNOT_CONTROL);
  migrateLocked();
  assertHasV1Address(f.free);
  assertHasV1Address(other);

  setMaxNamesBelowNameForTesting(0);
  register(WRAPPER_REGISTRY, "free", OWNER_2, WRAPPER_REGISTRY);

  assert.notInStore("ENSv2NamePath", f.free);
  assertNoAddress(f.free);
  assertNoAddress(f.belowFree);
  assertNoAddress(other);
  assert.fieldEquals("ENSv2NamePath", f.xNode.toHexString(), "legacySubtreeVoided", "true");
  handleNewResolver(v1NewResolver(f.free, OTHER_V1_RESOLVER));
  handleNewResolver(v1NewResolver(other, OTHER_V1_RESOLVER));
  assertNoAddress(f.free);
  assertNoAddress(other);
});

test("a locked subname that migrates into the WrapperRegistry keeps its own emancipated subnames", () => {
  let f = seedLockedName();
  // free.x.eth is locked itself, and below.free.x.eth is emancipated.
  wrap(f.free, PARENT_CANNOT_CONTROL | CANNOT_UNWRAP);
  wrap(f.belowFree, PARENT_CANNOT_CONTROL);
  let other = seedLegacyName(Bytes.fromHexString(f.free), "other");
  migrateLocked();
  const CHILD_WRAPPER_REGISTRY = "0xc3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3";

  deployWrapperRegistry(CHILD_WRAPPER_REGISTRY);
  register(WRAPPER_REGISTRY, "free", OWNER, WRAPPER_REGISTRY);
  link(WRAPPER_REGISTRY, "free", CHILD_WRAPPER_REGISTRY);

  assert.fieldEquals("ENSv2NameSlot", slotIdOf(WRAPPER_REGISTRY, "free"), "migratedFromV1", "true");
  assert.fieldEquals("ENSv2NamePath", f.free, "legacySubtreeVoided", "false");
  assertHasV1Address(f.belowFree);
  assertNoAddress(other);
  handleNewResolver(v1NewResolver(f.belowFree, OTHER_V1_RESOLVER));
  assert.fieldEquals(
    "Domain",
    f.belowFree,
    "resolver",
    resolverIdOf(f.belowFree, OTHER_V1_RESOLVER)
  );
});

function slotIdOf(registry: string, label: string): string {
  let labelhash = lh(label);
  let slot = new Bytes(52);
  let registryBytes = Address.fromString(registry);
  for (let i = 0; i < 20; i++) {
    slot[i] = registryBytes[i];
  }
  for (let i = 0; i < 28; i++) {
    slot[20 + i] = labelhash[i];
  }
  return slot.toHexString();
}

test("a locked name that stops pointing at its WrapperRegistry loses its emancipated ENSv1 subnames", () => {
  let f = seedLockedName();
  migrateLocked();
  assertHasV1Address(f.free);

  link(ETH_REGISTRY, "x", USER_REGISTRY);

  assert.fieldEquals("ENSv2NamePath", f.xNode.toHexString(), "legacySubtreeVoided", "true");
  assertNoAddress(f.free);
  assertNoAddress(f.belowFree);
  handleNewResolver(v1NewResolver(f.free, OTHER_V1_RESOLVER));
  assertNoAddress(f.free);
});

// A WrapperRegistry sends to ENSv1 the children of the one name it was
// deployed for. Another WrapperRegistry under the name is not that one.
test("a locked name moved to another WrapperRegistry loses its emancipated ENSv1 subnames; its own link emitted again keeps them", () => {
  let f = seedLockedName();
  migrateLocked();

  link(ETH_REGISTRY, "x", WRAPPER_REGISTRY);
  assertHasV1Address(f.free);
  assert.fieldEquals("ENSv2NamePath", f.xNode.toHexString(), "legacySubtreeVoided", "false");

  const OTHER_WRAPPER_REGISTRY = "0xa9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9";
  deployWrapperRegistry(OTHER_WRAPPER_REGISTRY);
  link(ETH_REGISTRY, "x", OTHER_WRAPPER_REGISTRY);
  assertNoAddress(f.free);
  assertNoAddress(f.belowFree);
  assert.fieldEquals("ENSv2NamePath", f.xNode.toHexString(), "legacySubtreeVoided", "true");
  handleNewResolver(v1NewResolver(f.free, OTHER_V1_RESOLVER));
  assertNoAddress(f.free);
});

function labelReserved(registry: string, label: string): LabelReserved {
  let mockEvent = newMockEvent();
  let event = new LabelReserved(
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
    new ethereum.EventParam("tokenId", ethereum.Value.fromUnsignedBigInt(tokenOf(label)))
  );
  event.parameters.push(
    new ethereum.EventParam("labelHash", ethereum.Value.fromFixedBytes(lh(label)))
  );
  event.parameters.push(
    new ethereum.EventParam("label", ethereum.Value.fromString(label))
  );
  event.parameters.push(
    new ethereum.EventParam("expiry", ethereum.Value.fromUnsignedBigInt(FAR_FUTURE))
  );
  event.parameters.push(
    new ethereum.EventParam("sender", ethereum.Value.fromAddress(Address.fromString(SENDER)))
  );
  return event;
}

// The registry reads any expiry on a label as "ENSv2 is the authority", a
// reservation included, and a reservation projects nothing that would clear
// the ENSv1 row.
test("a label reserved in the WrapperRegistry ends the exception for that ENSv1 name only", () => {
  let f = seedLockedName();
  let other = seedLegacyName(f.xNode, "other");
  wrap(other, PARENT_CANNOT_CONTROL);
  migrateLocked();
  assertHasV1Address(f.free);

  handleLabelReserved(labelReserved(WRAPPER_REGISTRY, "free"));

  assertNoAddress(f.free);
  assertNoAddress(f.belowFree);
  assert.entityCount("ENSv2LegacyCutoff", 1);
  assertHasV1Address(other);
  assert.fieldEquals("ENSv2NamePath", f.xNode.toHexString(), "legacySubtreeVoided", "false");
});

// One event can project thousands of names, each with ENSv1 rows below it.
// They share one allowance of rows.
test("the sweeps of one event share one row limit", () => {
  let xNode = setUpV1Name();
  let s1 = seedLegacyName(xNode, "s1", false);
  seedLegacyName(Bytes.fromHexString(s1), "c1");
  seedLegacyName(Bytes.fromHexString(s1), "c2");
  let s2 = seedLegacyName(xNode, "s2", false);
  seedLegacyName(Bytes.fromHexString(s2), "c1");
  seedLegacyName(Bytes.fromHexString(s2), "c2");
  register(ETH_REGISTRY, "x");
  register(USER_REGISTRY, "s1");
  register(USER_REGISTRY, "s2");

  setMaxLegacyRowsVoidedForTesting(3);
  link(ETH_REGISTRY, "x", USER_REGISTRY);

  // Both names are projected; the two rows below the first were read, the
  // two below the second would have gone past what was left.
  assert.fieldEquals("ENSv2NamePath", s1, "active", "true");
  assert.fieldEquals("ENSv2NamePath", s2, "active", "true");
  assert.i32Equals(1, legacySweepBudgetForTesting());
});

test("a locked name that is retired loses its emancipated ENSv1 subnames", () => {
  let f = seedLockedName();
  migrateLocked();

  handleLabelUnregistered(createLabelUnregisteredEvent(ETH_REGISTRY, tokenOf("x")));

  assert.fieldEquals("ENSv2NamePath", f.xNode.toHexString(), "legacySubtreeVoided", "true");
  assertNoAddress(f.free);
  assertNoAddress(f.belowFree);
});

test("a locked name registered again natively keeps nothing from ENSv1", () => {
  let f = seedLockedName();
  migrateLocked();
  handleLabelUnregistered(createLabelUnregisteredEvent(ETH_REGISTRY, tokenOf("x")));

  register(ETH_REGISTRY, "x", OWNER_2);
  // Even pointed back at the old WrapperRegistry.
  link(ETH_REGISTRY, "x", WRAPPER_REGISTRY);

  handleNewResolver(v1NewResolver(f.free, OTHER_V1_RESOLVER));
  assertNoAddress(f.free);
});

test("a native registration over a name whose stale NameWrapper row says locked keeps nothing", () => {
  let f = seedLockedName();

  // Not a migration: the name lapsed and someone else registered it.
  register(ETH_REGISTRY, "x", OWNER_2);

  assert.fieldEquals("ENSv2NamePath", f.xNode.toHexString(), "legacySubtreeVoided", "true");
  assertNoAddress(f.free);
  assertNoAddress(f.belowFree);
  assertNoAddress(f.plain);
});
