// v2.2.0 legacy-entity contract (src/legacyGovernance.ts): ENSv2 governs a
// live ENSv2 name, a sink never becomes a holder, and the ENSv2 projection is
// complete in every registry. Each test replays the on-chain event order of a
// case found on mainnet at block 26,389,002 (named in the test).
import { Address, BigInt, ByteArray, Bytes, crypto, ethereum } from "@graphprotocol/graph-ts";
import {
  afterEach,
  assert,
  clearStore,
  dataSourceMock,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import { handleNewOwner, handleNewResolver } from "../src/ensRegistry";
import {
  handleNameRegistered,
  handleNameRenewed,
  handleNameTransferred,
} from "../src/ethRegistrar";
import {
  handleExpiryUpdated,
  handleLabelRegistered,
  handleLabelUnregistered,
  handleResolverUpdated,
  handleSubregistryUpdated,
  handleTokenRegenerated,
  handleTokenResource,
  handleTransferSingle,
} from "../src/ensv2Registry";
import { pathNamehash, tokenEntityId } from "../src/ensv2Utils";
import { createResolverID } from "../src/resolver";
import { NameRegistered, NameRenewed, Transfer } from "../src/types/BaseRegistrar/BaseRegistrar";
import { NewResolver } from "../src/types/ENSRegistry/EnsRegistry";
import { Domain, WrappedDomain } from "../src/types/schema";
import { concat, EMPTY_ADDRESS, ETH_NODE, uint256ToByteArray } from "../src/utils";
import {
  createExpiryUpdatedEvent,
  createLabelRegisteredEvent,
  createLabelUnregisteredEvent,
  createResolverUpdatedEvent,
  createSubregistryUpdatedEvent,
  createTokenRegeneratedEvent,
  createTokenResourceEvent,
  createTransferSingleEvent,
  slotToken,
} from "./ensv2TestEvents";
import { createNewOwnerEvent, setEthOwner } from "./testUtils";

// Sepolia deployment (ensv2Constants.ts).
const ROOT_REGISTRY = "0xB458D6a3a77919449d03e7A6903C26827c1eC43f";
const ETH_REGISTRY = "0xD4eBcbBdF463C9c45784603Db0dDD499BC44A8B4";
const GRAVEYARD = "0xb58a90a39d13cce1d0e192b5da5c47640855b04d";
const LOCKED_MIGRATION_CONTROLLER = "0x6029a063d69b09D23c52a754a90E4FE43aDac3A8";
const NAME_WRAPPER = "0xd4416b13d2b3a9abae7acd5d6c2bbdbe25686401";
const ZERO = "0x0000000000000000000000000000000000000000";
const ROOT_NAMEHASH =
  "0x0000000000000000000000000000000000000000000000000000000000000000";

const HOLDER = "0x1111111111111111111111111111111111111111";
const V2_OWNER = "0x2222222222222222222222222222222222222222";
const V2_OWNER_2 = "0x3333333333333333333333333333333333333333";
const USER_REGISTRY = "0x4444444444444444444444444444444444444444";
const USER_REGISTRY_B = "0x5555555555555555555555555555555555555555";
const WRAPPER_REGISTRY = "0x6666666666666666666666666666666666666666";
const V1_RESOLVER = "0x231b0ee14048e9dccd1d247744d114a4eb5e8e63";
const V2_RESOLVER = "0x7777777777777777777777777777777777777777";

const RETIREMENT_EXPIRY = "18446744073701775615";
const V2_REGISTRATION_TIME = "1793213027";

function a(addr: string): string {
  return Address.fromString(addr).toHexString();
}
function labelhash(label: string): Bytes {
  return Bytes.fromByteArray(crypto.keccak256(ByteArray.fromUTF8(label)));
}
function tokenIdOf(lh: Bytes): BigInt {
  // uint256(labelhash): graph-ts BigInt bytes are little-endian.
  let le = new Bytes(32);
  for (let i = 0; i < 32; i++) {
    le[i] = lh[31 - i];
  }
  return BigInt.fromUnsignedBytes(le);
}
function ethNode(lh: Bytes): Bytes {
  return Bytes.fromByteArray(crypto.keccak256(concat(ETH_NODE, lh)));
}
function subNode(parent: Bytes, lh: Bytes): Bytes {
  return Bytes.fromByteArray(crypto.keccak256(concat(parent, lh)));
}

let logIndex = 0;
// Every event gets its own log index and a post-ENSv2-deployment block
// number (legacyGovernance only looks for ENSv2 paths from the first ENSv2
// block on; Sepolia's is 11,820,291).
function stamp<T extends ethereum.Event>(event: T, timestamp: string): T {
  event.logIndex = BigInt.fromI32(++logIndex);
  event.block.timestamp = BigInt.fromString(timestamp);
  event.block.number = BigInt.fromI32(12000000);
  return event;
}

function nameRegistered(lh: Bytes, owner: string, expires: string, timestamp: string): NameRegistered {
  let m = newMockEvent();
  let e = new NameRegistered(m.address, m.logIndex, m.transactionLogIndex, m.logType, m.block, m.transaction, new Array<ethereum.EventParam>(), m.receipt);
  e.parameters.push(new ethereum.EventParam("id", ethereum.Value.fromUnsignedBigInt(tokenIdOf(lh))));
  e.parameters.push(new ethereum.EventParam("owner", ethereum.Value.fromAddress(Address.fromString(owner))));
  e.parameters.push(new ethereum.EventParam("expires", ethereum.Value.fromUnsignedBigInt(BigInt.fromString(expires))));
  return stamp(e, timestamp);
}
function nameRenewed(lh: Bytes, expires: string): NameRenewed {
  let m = newMockEvent();
  let e = new NameRenewed(m.address, m.logIndex, m.transactionLogIndex, m.logType, m.block, m.transaction, new Array<ethereum.EventParam>(), m.receipt);
  e.parameters.push(new ethereum.EventParam("id", ethereum.Value.fromUnsignedBigInt(tokenIdOf(lh))));
  e.parameters.push(new ethereum.EventParam("expires", ethereum.Value.fromUnsignedBigInt(BigInt.fromString(expires))));
  return stamp(e, "1794000000");
}
function tokenTransfer(lh: Bytes, from: string, to: string): Transfer {
  let m = newMockEvent();
  let e = new Transfer(m.address, m.logIndex, m.transactionLogIndex, m.logType, m.block, m.transaction, new Array<ethereum.EventParam>(), m.receipt);
  e.parameters.push(new ethereum.EventParam("from", ethereum.Value.fromAddress(Address.fromString(from))));
  e.parameters.push(new ethereum.EventParam("to", ethereum.Value.fromAddress(Address.fromString(to))));
  e.parameters.push(new ethereum.EventParam("tokenId", ethereum.Value.fromUnsignedBigInt(tokenIdOf(lh))));
  return stamp(e, "1794000000");
}
function newResolver(node: Bytes, resolver: string): NewResolver {
  let m = newMockEvent();
  let e = new NewResolver(m.address, m.logIndex, m.transactionLogIndex, m.logType, m.block, m.transaction, new Array<ethereum.EventParam>(), m.receipt);
  e.parameters.push(new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(node)));
  e.parameters.push(new ethereum.EventParam("resolver", ethereum.Value.fromAddress(Address.fromString(resolver))));
  return stamp(e, "1794000000");
}
function newOwner(parent: Bytes, lh: Bytes, owner: string): void {
  handleNewOwner(stamp(createNewOwnerEvent(parent.toHexString(), lh.toHexString(), owner), "1794000000"));
}

// An ENSv1 .eth name registered in 2022 by HOLDER, with a v1 resolver.
function seedV1EthName(lh: Bytes): Bytes {
  newOwner(ETH_NODE, lh, HOLDER);
  handleNameTransferred(tokenTransfer(lh, ZERO, HOLDER));
  handleNameRegistered(nameRegistered(lh, HOLDER, "1700000000", "1651959447"));
  let node = ethNode(lh);
  handleNewResolver(newResolver(node, V1_RESOLVER));
  return node;
}

// Graveyard.clear() of an expired .eth name: burn, mint to the Graveyard,
// registry NewOwner, NameRegistered, resolver cleared.
function sweepEthName(lh: Bytes, lastHolder: string): void {
  handleNameTransferred(tokenTransfer(lh, lastHolder, ZERO));
  handleNameTransferred(tokenTransfer(lh, ZERO, GRAVEYARD));
  newOwner(ETH_NODE, lh, GRAVEYARD);
  handleNameRegistered(nameRegistered(lh, GRAVEYARD, RETIREMENT_EXPIRY, "1794000000"));
  handleNewResolver(newResolver(ethNode(lh), ZERO));
}

function setupEnsv2Eth(): void {
  dataSourceMock.setNetwork("sepolia");
  setEthOwner();
  let ethLh = labelhash("eth");
  handleLabelRegistered(createLabelRegisteredEvent(ROOT_REGISTRY, slotToken(999), ethLh, "eth"));
  handleSubregistryUpdated(createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(999), ETH_REGISTRY));
  // The ENSv2 "eth" namespace base is the ENSv1 eth node.
  assert.assertTrue(pathNamehash(Bytes.fromHexString(ROOT_NAMEHASH), ethLh).equals(ETH_NODE));
}

function registerV2(registry: string, slot: i32, lh: Bytes, label: string, owner: string, sender: string = "0x11111111111111111111111111111111111111aa"): void {
  handleLabelRegistered(stamp(createLabelRegisteredEvent(registry, slotToken(slot), lh, label, owner, sender), V2_REGISTRATION_TIME));
  handleTokenResource(createTokenResourceEvent(registry, slotToken(slot), BigInt.fromI32(slot)));
}

afterEach(() => {
  dataSourceMock.resetValues();
  clearStore();
});

test("visigoth.eth: a v1 Graveyard clear after a native ENSv2 registration leaves the ENSv2 state alone", () => {
  setupEnsv2Eth();
  let lh = labelhash("visigoth");
  let node = seedV1EthName(lh);
  let id = node.toHexString();

  registerV2(ETH_REGISTRY, 1, lh, "visigoth", V2_OWNER);
  handleResolverUpdated(createResolverUpdatedEvent(ETH_REGISTRY, slotToken(1), V2_RESOLVER));

  sweepEthName(lh, HOLDER);

  assert.fieldEquals("Domain", id, "owner", a(V2_OWNER));
  assert.fieldEquals("Domain", id, "registrant", a(V2_OWNER));
  assert.fieldEquals("Domain", id, "resolver", createResolverID(node, Address.fromString(V2_RESOLVER)));
  assert.fieldEquals("Registration", lh.toHexString(), "registrant", a(V2_OWNER));
  assert.fieldEquals("Registration", lh.toHexString(), "expiryDate", "2000000000");
  // A native registration is a new registration.
  assert.fieldEquals("Registration", lh.toHexString(), "registrationDate", V2_REGISTRATION_TIME);
  // History still records every v1 event.
  assert.entityCount("NameRegistered", 2);
});

test("tonchain.eth: a native ENSv2 registration drops the ENSv1 resolver and wrapped state the label still had", () => {
  setupEnsv2Eth();
  let lh = labelhash("tonchain");
  let node = seedV1EthName(lh);
  let id = node.toHexString();
  let wrapped = new WrappedDomain(id);
  wrapped.domain = id;
  wrapped.owner = a(HOLDER);
  wrapped.fuses = 0;
  wrapped.expiryDate = BigInt.fromString("1707776000");
  wrapped.save();
  let domain = Domain.load(id)!;
  domain.wrappedOwner = a(HOLDER);
  domain.save();

  // Registered without a resolver: no ResolverUpdated follows.
  registerV2(ETH_REGISTRY, 2, lh, "tonchain", V2_OWNER);

  domain = Domain.load(id)!;
  assert.assertTrue(!domain.resolver);
  assert.assertTrue(!domain.resolvedAddress);
  assert.assertTrue(!domain.wrappedOwner);
  assert.notInStore("WrappedDomain", id);
  assert.fieldEquals("Domain", id, "owner", a(V2_OWNER));
});

test("hyrcan.eth: a swept expired wrapped name keeps its last owner and wrapped holder", () => {
  setupEnsv2Eth();
  let lh = labelhash("hyrcan");
  newOwner(ETH_NODE, lh, NAME_WRAPPER);
  handleNameTransferred(tokenTransfer(lh, ZERO, NAME_WRAPPER));
  handleNameRegistered(nameRegistered(lh, NAME_WRAPPER, "1700000000", "1651959447"));
  let id = ethNode(lh).toHexString();
  let wrapped = new WrappedDomain(id);
  wrapped.domain = id;
  wrapped.owner = a(HOLDER);
  wrapped.fuses = 0;
  wrapped.expiryDate = BigInt.fromString("1707776000");
  wrapped.save();
  let domain = Domain.load(id)!;
  domain.wrappedOwner = a(HOLDER);
  domain.save();

  sweepEthName(lh, NAME_WRAPPER);

  assert.fieldEquals("Domain", id, "owner", a(NAME_WRAPPER));
  assert.fieldEquals("Domain", id, "wrappedOwner", a(HOLDER));
  assert.fieldEquals("WrappedDomain", id, "owner", a(HOLDER));
  assert.fieldEquals("Domain", id, "expiryDate", "1707776000");
  assert.fieldEquals("Registration", lh.toHexString(), "expiryDate", "1700000000");
});

test("pipe.gnat777.eth: a v1 subname retired with its expired parent is deleted and leaves the parent's count", () => {
  setupEnsv2Eth();
  let parentLh = labelhash("gnat777");
  let parent = seedV1EthName(parentLh);
  let subLh = labelhash("pipe");
  newOwner(parent, subLh, NAME_WRAPPER);
  let sub = subNode(parent, subLh);
  handleNewResolver(newResolver(sub, V1_RESOLVER));
  let subId = sub.toHexString();
  let wrapped = new WrappedDomain(subId);
  wrapped.domain = subId;
  wrapped.owner = a(HOLDER);
  wrapped.fuses = 0;
  wrapped.expiryDate = BigInt.fromString("1707776000");
  wrapped.save();
  let domain = Domain.load(subId)!;
  domain.wrappedOwner = a(HOLDER);
  domain.save();
  assert.fieldEquals("Domain", parent.toHexString(), "subdomainCount", "1");

  sweepEthName(parentLh, HOLDER);
  // _clear's OWNED branch: setSubnodeRecord(parent, label, graveyard, 0, 0).
  newOwner(parent, subLh, GRAVEYARD);
  handleNewResolver(newResolver(sub, ZERO));

  assert.fieldEquals("Domain", subId, "owner", EMPTY_ADDRESS);
  assert.notInStore("WrappedDomain", subId);
  domain = Domain.load(subId)!;
  assert.assertTrue(!domain.wrappedOwner);
  assert.assertTrue(!domain.resolver);
  assert.fieldEquals("Domain", parent.toHexString(), "subdomainCount", "0");
  // The parent itself is a lapsed name, not Graveyard-owned.
  assert.fieldEquals("Domain", parent.toHexString(), "owner", a(HOLDER));
  assert.fieldEquals("Domain", parent.toHexString(), "registrant", a(HOLDER));
});

// tpa.eth registered natively in ENSv2 with a user registry; bmw-000.tpa.eth
// existed in ENSv1 and was re-created in the user registry.
function setupUserRegistryName(): Bytes {
  setupEnsv2Eth();
  let parentLh = labelhash("tpa");
  let parent = seedV1EthName(parentLh);
  newOwner(parent, labelhash("bmw-000"), HOLDER);
  registerV2(ETH_REGISTRY, 3, parentLh, "tpa", V2_OWNER);
  handleSubregistryUpdated(createSubregistryUpdatedEvent(ETH_REGISTRY, slotToken(3), USER_REGISTRY));
  registerV2(USER_REGISTRY, 1, labelhash("bmw-000"), "bmw-000", V2_OWNER_2);
  handleResolverUpdated(createResolverUpdatedEvent(USER_REGISTRY, slotToken(1), V1_RESOLVER));
  return parent;
}

test("bmw-000.tpa.eth: an ENSv2 subname survives a later Graveyard clear of its v1 record", () => {
  let parent = setupUserRegistryName();
  let sub = subNode(parent, labelhash("bmw-000"));
  let subId = sub.toHexString();
  assert.fieldEquals("Domain", subId, "owner", a(V2_OWNER_2));
  assert.fieldEquals("Domain", parent.toHexString(), "subdomainCount", "1");

  newOwner(parent, labelhash("bmw-000"), GRAVEYARD);
  handleNewResolver(newResolver(sub, ZERO));

  assert.fieldEquals("Domain", subId, "owner", a(V2_OWNER_2));
  assert.fieldEquals("Domain", subId, "resolver", createResolverID(sub, Address.fromString(V1_RESOLVER)));
  assert.fieldEquals("Domain", parent.toHexString(), "subdomainCount", "1");
});

test("m.etaverse.eth, 🔴safe.ismat.eth: transfers and renewals in a user registry reach the legacy row", () => {
  let parent = setupUserRegistryName();
  let subId = subNode(parent, labelhash("bmw-000")).toHexString();

  handleTransferSingle(createTransferSingleEvent(USER_REGISTRY, slotToken(1), V2_OWNER_2, HOLDER));
  assert.fieldEquals("Domain", subId, "owner", a(HOLDER));
  assert.fieldEquals("Domain", subId, "registrant", a(HOLDER));

  handleExpiryUpdated(createExpiryUpdatedEvent(USER_REGISTRY, slotToken(1), BigInt.fromString("2100000000")));
  assert.fieldEquals("Domain", subId, "expiryDate", "2100000000");
});

test("hodl.nic619.eth: unregistering a subname deletes its legacy row", () => {
  let parent = setupUserRegistryName();
  let subId = subNode(parent, labelhash("bmw-000")).toHexString();

  handleLabelUnregistered(stamp(createLabelUnregisteredEvent(USER_REGISTRY, slotToken(1)), "1795000000"));

  assert.fieldEquals("Domain", subId, "owner", EMPTY_ADDRESS);
  assert.fieldEquals("Domain", subId, "expiryDate", "1795000000");
  let domain = Domain.load(subId)!;
  assert.assertTrue(!domain.registrant);
  assert.assertTrue(!domain.resolver);
  assert.fieldEquals("Domain", parent.toHexString(), "subdomainCount", "0");

  // Re-registration revives it and counts it once.
  registerV2(USER_REGISTRY, 1, labelhash("bmw-000"), "bmw-000", V2_OWNER);
  assert.fieldEquals("Domain", subId, "owner", a(V2_OWNER));
  assert.fieldEquals("Domain", parent.toHexString(), "subdomainCount", "1");
});

test("vault.defenryx.eth: a path taken over by a new subregistry follows its new slot", () => {
  let parent = setupUserRegistryName();
  let sub = subNode(parent, labelhash("bmw-000"));
  let subId = sub.toHexString();

  // The parent switches to a new user registry: the old one's names go.
  handleSubregistryUpdated(createSubregistryUpdatedEvent(ETH_REGISTRY, slotToken(3), USER_REGISTRY_B));
  assert.fieldEquals("ENSv2NamePath", subId, "active", "false");
  assert.fieldEquals("Domain", subId, "owner", EMPTY_ADDRESS);

  registerV2(USER_REGISTRY_B, 7, labelhash("bmw-000"), "bmw-000", V2_OWNER);
  assert.fieldEquals("ENSv2NamePath", subId, "active", "true");
  assert.fieldEquals("Domain", subId, "owner", a(V2_OWNER));
  assert.fieldEquals("Domain", parent.toHexString(), "subdomainCount", "1");

  // The old registry's slot no longer reaches the path...
  handleLabelUnregistered(createLabelUnregisteredEvent(USER_REGISTRY, slotToken(1)));
  assert.fieldEquals("ENSv2NamePath", subId, "active", "true");
  assert.fieldEquals("Domain", subId, "owner", a(V2_OWNER));
  // ...the new one does.
  handleTokenResource(createTokenResourceEvent(USER_REGISTRY_B, slotToken(7), BigInt.fromI32(7)));
  handleTransferSingle(createTransferSingleEvent(USER_REGISTRY_B, slotToken(7), V2_OWNER, V2_OWNER_2));
  assert.fieldEquals("Domain", subId, "owner", a(V2_OWNER_2));
  handleLabelUnregistered(createLabelUnregisteredEvent(USER_REGISTRY_B, slotToken(7)));
  assert.fieldEquals("ENSv2NamePath", subId, "active", "false");
  assert.fieldEquals("Domain", subId, "owner", EMPTY_ADDRESS);
  assert.fieldEquals("Domain", parent.toHexString(), "subdomainCount", "0");
});

test("a v1 subname deleted before its ENSv2 re-creation is counted again", () => {
  setupEnsv2Eth();
  let parentLh = labelhash("tpa");
  let parent = seedV1EthName(parentLh);
  newOwner(parent, labelhash("old"), HOLDER);
  newOwner(parent, labelhash("old"), ZERO);
  assert.fieldEquals("Domain", parent.toHexString(), "subdomainCount", "0");

  registerV2(ETH_REGISTRY, 3, parentLh, "tpa", V2_OWNER);
  handleSubregistryUpdated(createSubregistryUpdatedEvent(ETH_REGISTRY, slotToken(3), USER_REGISTRY));
  registerV2(USER_REGISTRY, 1, labelhash("old"), "old", V2_OWNER_2);

  assert.fieldEquals("Domain", parent.toHexString(), "subdomainCount", "1");
  assert.fieldEquals("Domain", subNode(parent, labelhash("old")).toHexString(), "owner", a(V2_OWNER_2));
});

test("emancipated child migration: the v1 voiding deletes the subname and the WrapperRegistry registration revives it", () => {
  setupEnsv2Eth();
  let parentLh = labelhash("lockd");
  newOwner(ETH_NODE, parentLh, NAME_WRAPPER);
  handleNameTransferred(tokenTransfer(parentLh, ZERO, NAME_WRAPPER));
  handleNameRegistered(nameRegistered(parentLh, NAME_WRAPPER, "1900000000", "1651959447"));
  let parent = ethNode(parentLh);
  let kidLh = labelhash("kid");
  newOwner(parent, kidLh, NAME_WRAPPER);
  let kid = subNode(parent, kidLh);
  let kidId = kid.toHexString();
  handleNewResolver(newResolver(kid, V1_RESOLVER));
  let wrapped = new WrappedDomain(kidId);
  wrapped.domain = kidId;
  wrapped.owner = a(HOLDER);
  wrapped.fuses = 327680;
  wrapped.expiryDate = BigInt.fromString("1900000000");
  wrapped.save();
  let domain = Domain.load(kidId)!;
  domain.wrappedOwner = a(HOLDER);
  domain.save();

  // Locked parent migration, then its WrapperRegistry.
  registerV2(ETH_REGISTRY, 4, parentLh, "lockd", HOLDER, LOCKED_MIGRATION_CONTROLLER);
  handleSubregistryUpdated(createSubregistryUpdatedEvent(ETH_REGISTRY, slotToken(4), WRAPPER_REGISTRY));
  assert.fieldEquals("Domain", parent.toHexString(), "owner", a(HOLDER));
  assert.notInStore("WrappedDomain", parent.toHexString());

  // Child migration (LockedWrapperReceiver, emancipated branch):
  // setResolver(node, 0), unwrap(parent, label, GRAVEYARD), then _inject.
  handleNewResolver(newResolver(kid, ZERO));
  newOwner(parent, kidLh, GRAVEYARD);
  assert.fieldEquals("Domain", kidId, "owner", EMPTY_ADDRESS);
  registerV2(WRAPPER_REGISTRY, 1, kidLh, "kid", HOLDER, WRAPPER_REGISTRY);

  assert.fieldEquals("Domain", kidId, "owner", a(HOLDER));
  assert.fieldEquals("Domain", kidId, "registrant", a(HOLDER));
  assert.notInStore("WrappedDomain", kidId);
  assert.fieldEquals("Domain", parent.toHexString(), "subdomainCount", "1");
});

test("joyuri.eth: a migrated name's v1 token renewals do not override its ENSv2 expiry", () => {
  setupEnsv2Eth();
  let lh = labelhash("joyuri");
  let node = seedV1EthName(lh);
  let id = node.toHexString();
  // UnlockedMigrationController: token and record to the Graveyard, then _inject.
  handleNameTransferred(tokenTransfer(lh, HOLDER, "0xab821ca676cf2e0790fa273526a165cfa86aeb64"));
  handleNameTransferred(tokenTransfer(lh, "0xab821ca676cf2e0790fa273526a165cfa86aeb64", GRAVEYARD));
  newOwner(ETH_NODE, lh, GRAVEYARD);
  registerV2(ETH_REGISTRY, 5, lh, "joyuri", HOLDER, LOCKED_MIGRATION_CONTROLLER);
  assert.fieldEquals("Domain", id, "owner", a(HOLDER));
  assert.fieldEquals("Registration", lh.toHexString(), "registrationDate", "1651959447");
  let expiry = Domain.load(id)!.expiryDate!.toString();

  handleNameRenewed(nameRenewed(lh, RETIREMENT_EXPIRY));

  assert.fieldEquals("Domain", id, "expiryDate", expiry);
  assert.fieldEquals("Registration", lh.toHexString(), "expiryDate", "2000000000");
  assert.fieldEquals("Domain", id, "registrant", a(HOLDER));
});

test("Domain.registryToken follows the name's current ENSv2 token", () => {
  let parent = setupUserRegistryName();
  let subId = subNode(parent, labelhash("bmw-000")).toHexString();
  let registry = Address.fromString(USER_REGISTRY);

  // Set by the registration's TokenResource.
  assert.fieldEquals(
    "Domain",
    subId,
    "registryToken",
    tokenEntityId(registry, slotToken(1)).toHexString()
  );
  // The parent, registered natively in the ETH registry, has its own.
  assert.fieldEquals(
    "Domain",
    parent.toHexString(),
    "registryToken",
    tokenEntityId(Address.fromString(ETH_REGISTRY), slotToken(3)).toHexString()
  );

  // A regenerated token replaces it.
  let regenerated = slotToken(1).plus(BigInt.fromI32(1));
  handleTokenRegenerated(createTokenRegeneratedEvent(USER_REGISTRY, slotToken(1), regenerated));
  assert.fieldEquals(
    "Domain",
    subId,
    "registryToken",
    tokenEntityId(registry, regenerated).toHexString()
  );

  // Unregistering clears it.
  handleLabelUnregistered(createLabelUnregisteredEvent(USER_REGISTRY, slotToken(1)));
  assert.assertTrue(!Domain.load(subId)!.registryToken);
});

test("an ENSv1 name has no registryToken", () => {
  setupEnsv2Eth();
  let node = seedV1EthName(labelhash("v1only"));
  assert.assertTrue(!Domain.load(node.toHexString())!.registryToken);
});
