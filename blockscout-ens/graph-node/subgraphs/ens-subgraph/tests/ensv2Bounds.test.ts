// v2.3.5: the limits that keep one event's work bounded, and the two
// correctness fixes that came with them.
//
//   - a registry's names are materialised under its canonical namespace and
//     at most 8 aliases (ensv2Paths.ts::tryMaterializeNamespace),
//   - one event backfills a bounded number of names, and the following
//     events finish a larger registry,
//   - a registry is projected a bounded number of times over its life,
//   - a re-registration retires the names of the previous subregistry,
//   - a registry that was not deployed through VerifiableFactory is indexed.
//
// Labels here are real keccak256 labelhashes and token ids are derived from
// them, as on chain: the canonical-parent check goes from a declared label to
// its slot.
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
  handleParentUpdated,
  handleSubregistryUpdated,
} from "../src/ensv2Registry";
import {
  MAX_ALIAS_NAMESPACES,
  MAX_BACKFILL_SLOTS_PER_EVENT,
  MAX_CANONICAL_RECONCILE_PATHS,
  setMaxCanonicalReconcilePathsForTesting,
  MAX_LIFETIME_MATERIALIZATIONS,
  setMaxBackfillSlotsPerEventForTesting,
} from "../src/ensv2Paths";
import { handleNewResolver } from "../src/ensRegistry";
import { createResolverID } from "../src/resolver";
import { namespaceId, nameSlotId, pathNamehash, toSlotId } from "../src/ensv2Utils";
import {
  Account,
  Domain,
  ENSv2NameSlot,
  ENSv2Registry,
  Resolver,
} from "../src/types/schema";
import { NewResolver } from "../src/types/ENSRegistry/EnsRegistry";
import { ParentUpdated } from "../src/types/RootRegistry/PermissionedRegistry";
import {
  createLabelRegisteredEvent,
  createSubregistryUpdatedEvent,
  SENDER,
} from "./ensv2TestEvents";

const ROOT_REGISTRY = "0xB458D6a3a77919449d03e7A6903C26827c1eC43f";
const ETH_REGISTRY = "0xD4eBcbBdF463C9c45784603Db0dDD499BC44A8B4";
const OWNER = "0x89205A3A3b2A69De6Dbf7f01ED13B2108B2c43e7";
const OWNER_2 = "0xF0205A3A3b2A69De6Dbf7f01ED13B2108B2c4321";
const EMPTY = "0x0000000000000000000000000000000000000000";
const ROOT_NODE = Bytes.fromHexString(
  "0x0000000000000000000000000000000000000000000000000000000000000000"
);

function lh(label: string): Bytes {
  return Bytes.fromByteArray(crypto.keccak256(ByteArray.fromUTF8(label)));
}

// uint256(labelhash), the token id a registry mints for a label (version 0).
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

function register(registry: string, label: string, owner: string = OWNER): void {
  handleLabelRegistered(
    createLabelRegisteredEvent(registry, tokenOf(label), lh(label), label, owner)
  );
}

function link(registry: string, label: string, subregistry: string): void {
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(registry, tokenOf(label), subregistry)
  );
}

function declareParent(registry: string, parent: string, label: string): void {
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
    new ethereum.EventParam(
      "parent",
      ethereum.Value.fromAddress(Address.fromString(parent))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("label", ethereum.Value.fromString(label))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "sender",
      ethereum.Value.fromAddress(Address.fromString(SENDER))
    )
  );
  handleParentUpdated(event);
}

function slotOf(registry: string, label: string): ENSv2NameSlot {
  return ENSv2NameSlot.load(
    nameSlotId(Address.fromString(registry), toSlotId(tokenOf(label)))
  )!;
}

function materializedCount(registry: string): i32 {
  return ENSv2Registry.load(Address.fromString(registry))!.materializedNamespaces
    .length;
}

function ns(registry: string, base: Bytes): string {
  return namespaceId(Address.fromString(registry), base).toHexString();
}

function registryAddress(n: i32): string {
  let hex = (n + 16).toString(16);
  let out = "0x";
  for (let i = 0; i < 20; i++) {
    out += hex;
  }
  return out;
}

afterEach(() => {
  setMaxCanonicalReconcilePathsForTesting(MAX_CANONICAL_RECONCILE_PATHS);
  setMaxBackfillSlotsPerEventForTesting(MAX_BACKFILL_SLOTS_PER_EVENT);
  dataSourceMock.resetValues();
  clearStore();
});

// ---------------------------------------------------------------------------
// Namespace cap
// ---------------------------------------------------------------------------

// Before the cap, three names pointing back at their own registry gave
// 3 + 9 + 27 + ... + 3^16 paths (about 64 million) from one link.
test("a registry whose names all point back at it is materialised under 8 namespaces", () => {
  dataSourceMock.setNetwork("sepolia");
  const SELF = "0xfafafafafafafafafafafafafafafafafafafafa";
  let labels = ["n1", "n2", "n3"];
  register(ROOT_REGISTRY, "top");
  for (let i = 0; i < labels.length; i++) {
    register(SELF, labels[i]);
    link(SELF, labels[i], SELF);
  }

  link(ROOT_REGISTRY, "top", SELF);

  assert.i32Equals(MAX_ALIAS_NAMESPACES, materializedCount(SELF));
  let paths = 0;
  for (let i = 0; i < labels.length; i++) {
    paths += slotOf(SELF, labels[i]).pathCount;
  }
  assert.i32Equals(MAX_ALIAS_NAMESPACES * labels.length, paths);

  // The links beyond the cap are recorded, not projected.
  let self = ENSv2Registry.load(Address.fromString(SELF))!;
  assert.i32Equals(MAX_ALIAS_NAMESPACES * labels.length + 1, self.namespaceCount);
  let top = child(ROOT_NODE, "top");
  assert.fieldEquals("ENSv2Namespace", ns(SELF, top), "materialized", "true");
  // n1.top is where the second namespace hangs; n2.top came after the cap
  // was reached further down.
  assert.fieldEquals("ENSv2Namespace", ns(SELF, child(top, "n1")), "materialized", "true");
  assert.fieldEquals("ENSv2Namespace", ns(SELF, child(top, "n2")), "active", "true");
  assert.fieldEquals("ENSv2Namespace", ns(SELF, child(top, "n2")), "materialized", "false");

  // A later registration costs one path per materialised namespace.
  register(SELF, "late");
  assert.i32Equals(MAX_ALIAS_NAMESPACES, slotOf(SELF, "late").pathCount);
});

// Before the cap: 5 + 25 + 125 + 625 = 780 paths.
test("a chain of registries whose names all point at the next one stays linear", () => {
  dataSourceMock.setNetwork("sepolia");
  let labels = ["a", "b", "c", "d", "e"];
  let depth = 4;
  register(ROOT_REGISTRY, "top");
  for (let d = 0; d < depth; d++) {
    for (let i = 0; i < labels.length; i++) {
      register(registryAddress(d), labels[i]);
      if (d + 1 < depth) {
        link(registryAddress(d), labels[i], registryAddress(d + 1));
      }
    }
  }

  link(ROOT_REGISTRY, "top", registryAddress(0));

  assert.i32Equals(1, materializedCount(registryAddress(0)));
  assert.i32Equals(5, materializedCount(registryAddress(1)));
  assert.i32Equals(MAX_ALIAS_NAMESPACES, materializedCount(registryAddress(2)));
  assert.i32Equals(MAX_ALIAS_NAMESPACES, materializedCount(registryAddress(3)));
  let paths = 0;
  for (let d = 0; d < depth; d++) {
    for (let i = 0; i < labels.length; i++) {
      paths += slotOf(registryAddress(d), labels[i]).pathCount;
    }
  }
  assert.i32Equals(5 + 25 + 40 + 40, paths);
});

test("unlinking a namespace frees its place for another alias", () => {
  dataSourceMock.setNetwork("sepolia");
  const SHARED = "0xa0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0";
  register(SHARED, "x");
  for (let i = 0; i < MAX_ALIAS_NAMESPACES + 1; i++) {
    let label = "p" + i.toString();
    register(ROOT_REGISTRY, label);
    link(ROOT_REGISTRY, label, SHARED);
  }
  let ninth = child(ROOT_NODE, "p" + MAX_ALIAS_NAMESPACES.toString());
  assert.i32Equals(MAX_ALIAS_NAMESPACES, materializedCount(SHARED));
  assert.fieldEquals("ENSv2Namespace", ns(SHARED, ninth), "materialized", "false");
  assert.notInStore("ENSv2NamePath", child(ninth, "x").toHexString());

  link(ROOT_REGISTRY, "p0", EMPTY);
  assert.i32Equals(MAX_ALIAS_NAMESPACES - 1, materializedCount(SHARED));
  assert.fieldEquals(
    "ENSv2NamePath",
    child(child(ROOT_NODE, "p0"), "x").toHexString(),
    "active",
    "false"
  );

  // Emitting the refused link again retries it.
  link(ROOT_REGISTRY, "p" + MAX_ALIAS_NAMESPACES.toString(), SHARED);
  assert.i32Equals(MAX_ALIAS_NAMESPACES, materializedCount(SHARED));
  assert.fieldEquals("ENSv2Namespace", ns(SHARED, ninth), "materialized", "true");
  assert.fieldEquals("Domain", child(ninth, "x").toHexString(), "owner", OWNER.toLowerCase());
});

// Anyone can point a name at someone else's registry. Without a reserved
// place, eight such links made before the owner's own would keep the owner's
// names out of the projection.
function squat(parent: string, target: string): void {
  for (let i = 0; i < MAX_ALIAS_NAMESPACES; i++) {
    let label = "squat" + i.toString();
    register(parent, label, OWNER_2);
    link(parent, label, target);
  }
}

function setUpCanonicalParent(parent: string): Bytes {
  register(ROOT_REGISTRY, "base");
  declareParent(parent, ROOT_REGISTRY, "base");
  link(ROOT_REGISTRY, "base", parent);
  let base = child(ROOT_NODE, "base");
  assert.fieldEquals("ENSv2Namespace", ns(parent, base), "canonical", "true");
  return base;
}

test("the namespace a registry declares as its parent is materialised even when its aliases are taken", () => {
  dataSourceMock.setNetwork("sepolia");
  const PARENT = "0xb0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0";
  const VICTIM = "0xb1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1";
  let base = setUpCanonicalParent(PARENT);
  register(VICTIM, "sub");
  declareParent(VICTIM, PARENT, "victim");

  squat(PARENT, VICTIM);
  assert.i32Equals(MAX_ALIAS_NAMESPACES, materializedCount(VICTIM));

  register(PARENT, "victim");
  link(PARENT, "victim", VICTIM);

  let victim = child(base, "victim");
  assert.fieldEquals("ENSv2Namespace", ns(VICTIM, victim), "canonical", "true");
  assert.fieldEquals("ENSv2Namespace", ns(VICTIM, victim), "materialized", "true");
  assert.i32Equals(MAX_ALIAS_NAMESPACES + 1, materializedCount(VICTIM));
  assert.fieldEquals("Domain", child(victim, "sub").toHexString(), "name", "sub.victim.base");
  assert.fieldEquals("Domain", child(victim, "sub").toHexString(), "owner", OWNER.toLowerCase());

  // One more alias is still refused.
  register(PARENT, "extra", OWNER_2);
  link(PARENT, "extra", VICTIM);
  assert.fieldEquals(
    "ENSv2Namespace",
    ns(VICTIM, child(base, "extra")),
    "materialized",
    "false"
  );
});

test("a link refused as an alias is materialised once the registry declares that parent", () => {
  dataSourceMock.setNetwork("sepolia");
  const PARENT = "0xb2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2";
  const VICTIM = "0xb3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3";
  let base = setUpCanonicalParent(PARENT);
  register(VICTIM, "sub");
  squat(PARENT, VICTIM);

  // Linked before the declaration: an alias like any other, and refused.
  register(PARENT, "victim");
  link(PARENT, "victim", VICTIM);
  let victim = child(base, "victim");
  assert.fieldEquals("ENSv2Namespace", ns(VICTIM, victim), "active", "true");
  assert.fieldEquals("ENSv2Namespace", ns(VICTIM, victim), "materialized", "false");
  assert.notInStore("ENSv2NamePath", child(victim, "sub").toHexString());

  declareParent(VICTIM, PARENT, "victim");

  assert.fieldEquals("ENSv2Namespace", ns(VICTIM, victim), "canonical", "true");
  assert.fieldEquals("ENSv2Namespace", ns(VICTIM, victim), "materialized", "true");
  assert.fieldEquals("Domain", child(victim, "sub").toHexString(), "owner", OWNER.toLowerCase());
});

test("a parent declaration is canonical only through a canonical parent name", () => {
  dataSourceMock.setNetwork("sepolia");
  const PARENT = "0xb4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4";
  const CHILD = "0xb5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5";
  const GRANDCHILD = "0xb6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6";
  // PARENT declares no parent of its own, so its namespace is an alias.
  register(ROOT_REGISTRY, "base");
  link(ROOT_REGISTRY, "base", PARENT);
  let base = child(ROOT_NODE, "base");
  assert.fieldEquals("ENSv2Namespace", ns(PARENT, base), "canonical", "false");

  register(PARENT, "name");
  declareParent(CHILD, PARENT, "name");
  link(PARENT, "name", CHILD);
  let name = child(base, "name");
  assert.fieldEquals("ENSv2Namespace", ns(CHILD, name), "canonical", "false");
  assert.fieldEquals("ENSv2Namespace", ns(CHILD, name), "materialized", "true");
  register(CHILD, "deep");
  declareParent(GRANDCHILD, CHILD, "deep");
  link(CHILD, "deep", GRANDCHILD);
  let deep = child(name, "deep");
  assert.fieldEquals("ENSv2Namespace", ns(GRANDCHILD, deep), "canonical", "false");

  // When the registry at the top declares its own parent, the ones below
  // that had already declared theirs become canonical with it, at every
  // level, and stop again when it withdraws the declaration.
  declareParent(PARENT, ROOT_REGISTRY, "base");
  assert.fieldEquals("ENSv2Namespace", ns(PARENT, base), "canonical", "true");
  assert.fieldEquals("ENSv2Namespace", ns(CHILD, name), "canonical", "true");
  assert.fieldEquals("ENSv2Namespace", ns(GRANDCHILD, deep), "canonical", "true");

  declareParent(PARENT, EMPTY, "");
  assert.fieldEquals("ENSv2Namespace", ns(PARENT, base), "canonical", "false");
  assert.fieldEquals("ENSv2Namespace", ns(CHILD, name), "canonical", "false");
  assert.fieldEquals("ENSv2Namespace", ns(GRANDCHILD, deep), "canonical", "false");
});

// The walk down from a declaration is bounded: below one name it can cover
// everything there is, but the ETHRegistry's names are not limited.
test("carrying a declaration down stops after a bounded number of names", () => {
  dataSourceMock.setNetwork("sepolia");
  const PARENT = "0xb9b9b9b9b9b9b9b9b9b9b9b9b9b9b9b9b9b9b9b9";
  const CHILD = "0xbabababababababababababababababababababa";
  register(ROOT_REGISTRY, "base");
  link(ROOT_REGISTRY, "base", PARENT);
  let base = child(ROOT_NODE, "base");
  register(PARENT, "n1");
  register(PARENT, "n2");
  register(PARENT, "n3");
  declareParent(CHILD, PARENT, "n3");
  link(PARENT, "n3", CHILD);
  let n3 = child(base, "n3");

  // Two names are looked at; the third, which has the registry, is not.
  setMaxCanonicalReconcilePathsForTesting(2);
  declareParent(PARENT, ROOT_REGISTRY, "base");
  assert.fieldEquals("ENSv2Namespace", ns(PARENT, base), "canonical", "true");
  assert.fieldEquals("ENSv2Namespace", ns(CHILD, n3), "canonical", "false");

  // Its own link or declaration brings it up to date.
  declareParent(CHILD, PARENT, "n3");
  assert.fieldEquals("ENSv2Namespace", ns(CHILD, n3), "canonical", "true");
});

// A registry that declared its parent before the registry above declared
// its own counted as an alias. If its alias places were already taken, its
// own name was refused, and stayed refused after the declaration above made
// it canonical.
test("a registry refused as an alias is projected once a declaration above makes its name canonical", () => {
  dataSourceMock.setNetwork("sepolia");
  const PARENT = "0xb7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7";
  const CHILD = "0xb8b8b8b8b8b8b8b8b8b8b8b8b8b8b8b8b8b8b8b8";
  register(CHILD, "sub");
  // Eight other names already point at CHILD.
  for (let i = 0; i < MAX_ALIAS_NAMESPACES; i++) {
    register(ROOT_REGISTRY, "squat" + i.toString());
    link(ROOT_REGISTRY, "squat" + i.toString(), CHILD);
  }
  register(ROOT_REGISTRY, "base");
  link(ROOT_REGISTRY, "base", PARENT);
  let base = child(ROOT_NODE, "base");
  register(PARENT, "name");
  let name = child(base, "name");
  declareParent(CHILD, PARENT, "name");
  link(PARENT, "name", CHILD);
  assert.fieldEquals("ENSv2Namespace", ns(CHILD, name), "active", "true");
  assert.fieldEquals("ENSv2Namespace", ns(CHILD, name), "materialized", "false");
  assert.notInStore("ENSv2NamePath", child(name, "sub").toHexString());

  declareParent(PARENT, ROOT_REGISTRY, "base");
  assert.fieldEquals("ENSv2Namespace", ns(CHILD, name), "canonical", "true");
  assert.fieldEquals("ENSv2Namespace", ns(CHILD, name), "materialized", "true");
  assert.fieldEquals("Domain", child(name, "sub").toHexString(), "name", "sub.name.base");
});

// ---------------------------------------------------------------------------
// Backfill budget
// ---------------------------------------------------------------------------

// One event projects a bounded number of names. A registry with more than
// that is admitted all the same and finished by the registry events that
// follow, any registry's, each with a budget of its own.
test("a registry with more names than one event projects is finished by the events that follow", () => {
  dataSourceMock.setNetwork("sepolia");
  const BIG = "0xc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0";
  let labels = ["a", "b", "c", "d", "e", "f", "g"];
  for (let i = 0; i < labels.length; i++) {
    register(BIG, labels[i]);
  }
  register(ROOT_REGISTRY, "top");
  let top = child(ROOT_NODE, "top");

  setMaxBackfillSlotsPerEventForTesting(3);
  link(ROOT_REGISTRY, "top", BIG);
  assert.fieldEquals("ENSv2Namespace", ns(BIG, top), "materialized", "true");
  assert.fieldEquals("ENSv2Namespace", ns(BIG, top), "backfilled", "false");
  assert.fieldEquals("ENSv2Namespace", ns(BIG, top), "backfillCursor", "3");
  assert.fieldEquals("ENSv2Namespace", ns(BIG, top), "backfillQueued", "true");
  assert.entityCount("ENSv2NamePath", 4);
  assert.fieldEquals("Domain", child(top, "c").toHexString(), "name", "c.top");
  assert.notInStore("Domain", child(top, "d").toHexString());

  // A registration in the registry is projected at once, and its event
  // carries the backfill three slots further: d, e, f.
  register(BIG, "late");
  assert.i32Equals(1, slotOf(BIG, "late").pathCount);
  assert.fieldEquals("Domain", child(top, "late").toHexString(), "name", "late.top");
  assert.fieldEquals("ENSv2Namespace", ns(BIG, top), "backfillCursor", "6");
  assert.fieldEquals("ENSv2Namespace", ns(BIG, top), "backfilled", "false");
  assert.fieldEquals("Domain", child(top, "f").toHexString(), "name", "f.top");
  assert.notInStore("Domain", child(top, "g").toHexString());

  // An event of an unrelated registry finishes it: g, and "late", which is
  // found already projected.
  register(ROOT_REGISTRY, "elsewhere");
  assert.fieldEquals("ENSv2Namespace", ns(BIG, top), "backfilled", "true");
  assert.fieldEquals("ENSv2Namespace", ns(BIG, top), "backfillQueued", "false");
  assert.fieldEquals("Domain", child(top, "g").toHexString(), "name", "g.top");
  assert.i32Equals(1, slotOf(BIG, "late").pathCount);
  // top, elsewhere, and the registry's eight names.
  assert.entityCount("ENSv2NamePath", 10);
  assert.entityCount("ENSv2BackfillTask", 0);
});

test("emitting the link again also takes an unfinished backfill further", () => {
  dataSourceMock.setNetwork("sepolia");
  const BIG = "0xc3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3";
  let labels = ["a", "b", "c", "d"];
  for (let i = 0; i < labels.length; i++) {
    register(BIG, labels[i]);
  }
  register(ROOT_REGISTRY, "top");
  let top = child(ROOT_NODE, "top");

  setMaxBackfillSlotsPerEventForTesting(3);
  link(ROOT_REGISTRY, "top", BIG);
  assert.fieldEquals("ENSv2Namespace", ns(BIG, top), "backfilled", "false");
  assert.i32Equals(1, ENSv2Registry.load(Address.fromString(BIG))!.aliasMaterializations);

  link(ROOT_REGISTRY, "top", BIG);
  assert.fieldEquals("ENSv2Namespace", ns(BIG, top), "backfilled", "true");
  assert.fieldEquals("Domain", child(top, "d").toHexString(), "name", "d.top");
  // Resuming is not a new projection.
  assert.i32Equals(1, ENSv2Registry.load(Address.fromString(BIG))!.aliasMaterializations);
});

test("a link removed while its backfill is unfinished is dropped from the queue", () => {
  dataSourceMock.setNetwork("sepolia");
  const BIG = "0xc4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4";
  let labels = ["a", "b", "c", "d"];
  for (let i = 0; i < labels.length; i++) {
    register(BIG, labels[i]);
  }
  register(ROOT_REGISTRY, "top");
  let top = child(ROOT_NODE, "top");

  setMaxBackfillSlotsPerEventForTesting(3);
  link(ROOT_REGISTRY, "top", BIG);
  link(ROOT_REGISTRY, "top", EMPTY);
  assert.fieldEquals("ENSv2NamePath", child(top, "a").toHexString(), "active", "false");

  register(ROOT_REGISTRY, "elsewhere");
  assert.entityCount("ENSv2BackfillTask", 0);
  assert.fieldEquals("ENSv2Namespace", ns(BIG, top), "backfillQueued", "false");
  assert.fieldEquals("ENSv2Namespace", ns(BIG, top), "materialized", "false");
  assert.notInStore("ENSv2NamePath", child(top, "d").toHexString());
});

// Retiring everything below a name and backfilling a large registry are each
// sized to fit one handler, not both together: what an event retires comes
// off its backfill budget.
test("an event that retires a large subtree leaves the new registry's backfill to the next one", () => {
  dataSourceMock.setNetwork("sepolia");
  const OLD = "0xc5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5";
  const NEW = "0xc6c6c6c6c6c6c6c6c6c6c6c6c6c6c6c6c6c6c6c6";
  register(ROOT_REGISTRY, "top");
  let top = child(ROOT_NODE, "top");
  link(ROOT_REGISTRY, "top", OLD);
  for (let i = 0; i < 80; i++) {
    register(OLD, "old" + i.toString());
  }
  for (let i = 0; i < 5; i++) {
    register(NEW, "new" + i.toString());
  }

  // Eighty names and their registry retired take ten units, all there are.
  setMaxBackfillSlotsPerEventForTesting(10);
  link(ROOT_REGISTRY, "top", NEW);
  assert.fieldEquals("ENSv2NamePath", child(top, "old0").toHexString(), "active", "false");
  assert.fieldEquals("ENSv2NamePath", child(top, "old79").toHexString(), "active", "false");
  assert.fieldEquals("ENSv2Namespace", ns(NEW, top), "materialized", "true");
  assert.fieldEquals("ENSv2Namespace", ns(NEW, top), "backfilled", "false");
  assert.fieldEquals("ENSv2Namespace", ns(NEW, top), "backfillCursor", "0");
  assert.notInStore("ENSv2NamePath", child(top, "new0").toHexString());

  register(ROOT_REGISTRY, "elsewhere");
  assert.fieldEquals("ENSv2Namespace", ns(NEW, top), "backfilled", "true");
  assert.fieldEquals("Domain", child(top, "new4").toHexString(), "name", "new4.top");
});

test("the backfill budget is shared by every level below one link", () => {
  dataSourceMock.setNetwork("sepolia");
  const UPPER = "0xc1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1";
  const LOWER = "0xc2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2";
  register(UPPER, "a");
  register(UPPER, "b");
  link(UPPER, "a", LOWER);
  register(LOWER, "x");
  register(LOWER, "y");
  register(ROOT_REGISTRY, "top");
  let top = child(ROOT_NODE, "top");

  // One slot for UPPER's "a", two for LOWER below it: none left for "b".
  setMaxBackfillSlotsPerEventForTesting(3);
  link(ROOT_REGISTRY, "top", UPPER);
  assert.fieldEquals("Domain", child(top, "a").toHexString(), "name", "a.top");
  assert.fieldEquals("ENSv2Namespace", ns(LOWER, child(top, "a")), "backfilled", "true");
  assert.fieldEquals("Domain", child(child(top, "a"), "y").toHexString(), "name", "y.a.top");
  assert.fieldEquals("ENSv2Namespace", ns(UPPER, top), "backfilled", "false");
  assert.notInStore("ENSv2NamePath", child(top, "b").toHexString());

  // The next registry event has a budget of its own.
  register(ROOT_REGISTRY, "elsewhere");
  assert.fieldEquals("ENSv2Namespace", ns(UPPER, top), "backfilled", "true");
  assert.fieldEquals("Domain", child(top, "b").toHexString(), "name", "b.top");
});

// ---------------------------------------------------------------------------
// Retirement has no limit
// ---------------------------------------------------------------------------

// The limits above only ever leave a name unprojected. Retiring is the
// opposite case: a name left live below a link that is gone would keep
// showing an owner and an address it no longer has.
test("removing a link retires the names below it at every depth", () => {
  dataSourceMock.setNetwork("sepolia");
  let levels = 20;
  register(ROOT_REGISTRY, "top");
  link(ROOT_REGISTRY, "top", registryAddress(0));
  // Built top-down, one link per event, so every level is projected.
  let node = child(ROOT_NODE, "top");
  let nodes = new Array<Bytes>();
  for (let d = 0; d < levels; d++) {
    register(registryAddress(d), "n");
    node = child(node, "n");
    nodes.push(node);
    if (d + 1 < levels) {
      link(registryAddress(d), "n", registryAddress(d + 1));
    }
  }
  let deepest = nodes[levels - 1].toHexString();
  assert.fieldEquals("ENSv2NamePath", deepest, "active", "true");
  assert.fieldEquals("Domain", deepest, "owner", OWNER.toLowerCase());

  link(ROOT_REGISTRY, "top", EMPTY);

  for (let d = 0; d < levels; d++) {
    assert.fieldEquals("ENSv2NamePath", nodes[d].toHexString(), "active", "false");
    assert.fieldEquals("Domain", nodes[d].toHexString(), "owner", EMPTY);
    assert.fieldEquals("Domain", nodes[d].toHexString(), "resolvedAddress", "null");
    assert.i32Equals(0, materializedCount(registryAddress(d)));
  }

  // Making the link again projects down to the depth limit of one event and
  // leaves the rest unprojected, not live.
  link(ROOT_REGISTRY, "top", registryAddress(0));
  assert.fieldEquals("ENSv2NamePath", nodes[0].toHexString(), "active", "true");
  assert.fieldEquals("ENSv2NamePath", deepest, "active", "false");
  assert.fieldEquals("Domain", deepest, "owner", EMPTY);
});

// A name can be pointed at any number of registries over time. Retiring it
// follows the one that is linked now (ENSv2NamePath.activeNamespace), not
// the list of all it ever had.
test("a name that has pointed at many registries tracks the one linked now", () => {
  dataSourceMock.setNetwork("sepolia");
  let count = 40;
  register(ROOT_REGISTRY, "top");
  let top = child(ROOT_NODE, "top");
  for (let i = 0; i < count; i++) {
    register(registryAddress(i), "n" + i.toString());
    link(ROOT_REGISTRY, "top", registryAddress(i));
  }
  let last = count - 1;
  assert.fieldEquals("ENSv2NamePath", top.toHexString(), "namespaceCount", count.toString());
  assert.fieldEquals(
    "ENSv2NamePath",
    top.toHexString(),
    "activeNamespace",
    ns(registryAddress(last), top)
  );
  for (let i = 0; i < count; i++) {
    let node = child(top, "n" + i.toString()).toHexString();
    assert.fieldEquals("ENSv2NamePath", node, "active", i == last ? "true" : "false");
    assert.fieldEquals(
      "ENSv2Namespace",
      ns(registryAddress(i), top),
      "active",
      i == last ? "true" : "false"
    );
  }

  link(ROOT_REGISTRY, "top", EMPTY);
  assert.fieldEquals("ENSv2NamePath", top.toHexString(), "activeNamespace", "null");
  assert.fieldEquals(
    "ENSv2NamePath",
    child(top, "n" + last.toString()).toHexString(),
    "active",
    "false"
  );
});

// The single pointer is safe because linking a namespace retires whatever
// else is still linked there, whether or not an earlier event unlinked it.
test("linking a namespace under a name retires another one still active there", () => {
  dataSourceMock.setNetwork("sepolia");
  const FIRST = "0xa4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4";
  const SECOND = "0xa5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5";
  register(ROOT_REGISTRY, "top");
  let top = child(ROOT_NODE, "top");
  register(FIRST, "a");
  register(SECOND, "b");
  link(ROOT_REGISTRY, "top", FIRST);
  assert.fieldEquals("ENSv2NamePath", child(top, "a").toHexString(), "active", "true");

  // Make the slot forget its subregistry, as a missed event would: the
  // handler then has no previous subregistry to unlink.
  let slot = slotOf(ROOT_REGISTRY, "top");
  slot.subregistry = null;
  slot.subregistryAddress = null;
  slot.save();

  link(ROOT_REGISTRY, "top", SECOND);

  assert.fieldEquals("ENSv2Namespace", ns(FIRST, top), "active", "false");
  assert.fieldEquals("ENSv2NamePath", child(top, "a").toHexString(), "active", "false");
  assert.fieldEquals("Domain", child(top, "a").toHexString(), "owner", EMPTY);
  assert.fieldEquals("ENSv2NamePath", child(top, "b").toHexString(), "active", "true");
  assert.fieldEquals("ENSv2NamePath", top.toHexString(), "activeNamespace", ns(SECOND, top));
});

// ---------------------------------------------------------------------------
// A refused link over ENSv1 names
// ---------------------------------------------------------------------------

const V1_RESOLVER = "0x4976fb03C32e5B8cfe2b6cCB31c09Ba78EBaBa41";
const V1_ADDRESS = "0x5555555555555555555555555555555555555501";

// An ENSv1 subname row as the ENSv1 handlers leave it: a resolver and the
// address it resolves to.
function seedLegacyChild(parent: Bytes, label: string): string {
  let node = child(parent, label);
  let owner = new Account(OWNER.toLowerCase());
  owner.save();
  let target = new Account(V1_ADDRESS);
  target.save();
  let resolverId = createResolverID(node, Address.fromString(V1_RESOLVER));
  let legacy = new Resolver(resolverId);
  legacy.domain = node.toHexString();
  legacy.address = Address.fromString(V1_RESOLVER);
  legacy.addr = target.id;
  legacy.save();
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
  domain.resolver = resolverId;
  domain.resolvedAddress = target.id;
  domain.save();
  let parentDomain = Domain.load(parent.toHexString())!;
  parentDomain.subdomainCount = parentDomain.subdomainCount + 1;
  parentDomain.save();
  return node.toHexString();
}

function v1NewResolver(node: Bytes, resolver: string): NewResolver {
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
  // After the ENSv2 deployment block: the governance rules do not apply to
  // the ENSv1 history before it.
  event.block.number = BigInt.fromI32(20000000);
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(node))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "resolver",
      ethereum.Value.fromAddress(Address.fromString(resolver))
    )
  );
  return event;
}

function seedLegacyTld(label: string): Bytes {
  let node = child(ROOT_NODE, label);
  let tld = new Domain(node.toHexString());
  tld.storedOffchain = false; // Blockscout/BENS extension
  tld.resolvedWithWildcard = false; // Blockscout/BENS extension
  tld.subdomainCount = 0;
  tld.isMigrated = true;
  tld.createdAt = BigInt.zero();
  let zero = new Account(EMPTY);
  zero.save();
  tld.owner = zero.id;
  tld.save();
  return node;
}

// A name under the RootRegistry has no projection, so nothing marks it when
// it is registered: the ENSv1 names below it keep their rows. When it points
// at a registry that is refused, those rows would go on showing the ENSv1
// address although the registry may hold the names.
test("a refused link clears the ENSv1 names below the linked name, at every depth, for good", () => {
  dataSourceMock.setNetwork("sepolia");
  const BIG = "0xf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0";
  let tldNode = seedLegacyTld("tld");
  register(ROOT_REGISTRY, "tld");
  let sub = seedLegacyChild(tldNode, "sub");
  let deep = seedLegacyChild(Bytes.fromHexString(sub), "deep");
  register(BIG, "sub", OWNER_2);
  // The registry already has its eight aliases.
  for (let i = 0; i < MAX_ALIAS_NAMESPACES; i++) {
    register(ROOT_REGISTRY, "alias" + i.toString());
    link(ROOT_REGISTRY, "alias" + i.toString(), BIG);
  }
  assert.fieldEquals("ENSv2NamePath", tldNode.toHexString(), "legacySubtreeVoided", "false");
  assert.fieldEquals("Domain", sub, "resolvedAddress", V1_ADDRESS);

  link(ROOT_REGISTRY, "tld", BIG);

  assert.fieldEquals("ENSv2Namespace", ns(BIG, tldNode), "materialized", "false");
  assert.fieldEquals("ENSv2NamePath", tldNode.toHexString(), "legacySubtreeVoided", "true");
  assert.fieldEquals("Domain", sub, "resolver", "null");
  assert.fieldEquals("Domain", sub, "resolvedAddress", "null");
  assert.fieldEquals("Domain", deep, "resolver", "null");
  assert.fieldEquals("Domain", deep, "resolvedAddress", "null");

  // ENSv1 cannot put a resolver back; the event is still recorded.
  handleNewResolver(v1NewResolver(Bytes.fromHexString(sub), V1_RESOLVER));
  assert.entityCount("NewResolver", 1);
  assert.fieldEquals("Domain", sub, "resolver", "null");
  assert.fieldEquals("Domain", sub, "resolvedAddress", "null");

  // Once an alias is given up and the link is projected, ENSv2 writes the
  // name.
  link(ROOT_REGISTRY, "alias0", EMPTY);
  link(ROOT_REGISTRY, "tld", BIG);
  assert.fieldEquals("ENSv2Namespace", ns(BIG, tldNode), "materialized", "true");
  assert.fieldEquals("Domain", sub, "owner", OWNER_2.toLowerCase());
  assert.fieldEquals("Domain", sub, "resolvedAddress", "null");
  assert.fieldEquals("Domain", deep, "resolvedAddress", "null");
});

// While a backfill is unfinished, a name of the registry may not be
// projected yet, so the ENSv1 row with its namehash must not go on showing
// the ENSv1 address in the meantime.
test("an unfinished backfill clears the ENSv1 names below the linked name", () => {
  dataSourceMock.setNetwork("sepolia");
  const BIG = "0xf1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1";
  let tldNode = seedLegacyTld("tld");
  register(ROOT_REGISTRY, "tld");
  let first = seedLegacyChild(tldNode, "first");
  let second = seedLegacyChild(tldNode, "second");
  register(BIG, "first", OWNER_2);
  register(BIG, "second", OWNER_2);

  setMaxBackfillSlotsPerEventForTesting(1);
  link(ROOT_REGISTRY, "tld", BIG);

  // "first" is ENSv2's; "second" is not projected yet and shows nothing.
  assert.fieldEquals("ENSv2Namespace", ns(BIG, tldNode), "backfilled", "false");
  assert.fieldEquals("Domain", first, "owner", OWNER_2.toLowerCase());
  assert.fieldEquals("Domain", first, "resolvedAddress", "null");
  assert.fieldEquals("Domain", second, "owner", OWNER.toLowerCase());
  assert.fieldEquals("Domain", second, "resolver", "null");
  assert.fieldEquals("Domain", second, "resolvedAddress", "null");
  assert.fieldEquals("ENSv2NamePath", tldNode.toHexString(), "legacySubtreeVoided", "true");
  handleNewResolver(v1NewResolver(Bytes.fromHexString(second), V1_RESOLVER));
  assert.fieldEquals("Domain", second, "resolver", "null");

  register(ROOT_REGISTRY, "elsewhere");
  assert.fieldEquals("ENSv2Namespace", ns(BIG, tldNode), "backfilled", "true");
  assert.fieldEquals("Domain", second, "owner", OWNER_2.toLowerCase());
  assert.fieldEquals("Domain", second, "resolvedAddress", "null");
});

// ---------------------------------------------------------------------------
// Lifetime allowance
// ---------------------------------------------------------------------------

// Removing a link and making it again walks every name below it, twice, for
// two events. Without a ceiling that can be repeated for as long as someone
// pays for the events.
test("a link that is removed and made again stops being re-projected after its lifetime allowance", () => {
  dataSourceMock.setNetwork("sepolia");
  const TOGGLED = "0xe0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0";
  register(TOGGLED, "x");
  register(ROOT_REGISTRY, "top");
  let top = child(ROOT_NODE, "top");
  let xNode = child(top, "x").toHexString();

  for (let i = 0; i < MAX_LIFETIME_MATERIALIZATIONS; i++) {
    link(ROOT_REGISTRY, "top", TOGGLED);
    assert.fieldEquals("ENSv2NamePath", xNode, "active", "true");
    link(ROOT_REGISTRY, "top", EMPTY);
    assert.fieldEquals("ENSv2NamePath", xNode, "active", "false");
  }
  assert.fieldEquals(
    "ENSv2Registry",
    TOGGLED,
    "aliasMaterializations",
    MAX_LIFETIME_MATERIALIZATIONS.toString()
  );

  link(ROOT_REGISTRY, "top", TOGGLED);
  assert.fieldEquals("ENSv2Namespace", ns(TOGGLED, top), "active", "true");
  assert.fieldEquals("ENSv2Namespace", ns(TOGGLED, top), "materialized", "false");
  assert.fieldEquals("ENSv2NamePath", xNode, "active", "false");
  assert.fieldEquals(
    "ENSv2Registry",
    TOGGLED,
    "aliasMaterializations",
    MAX_LIFETIME_MATERIALIZATIONS.toString()
  );

  // Aliases and the registry's own name draw on separate allowances: using
  // one up, which anyone can do with names of their own, leaves the other.
  declareParent(TOGGLED, ROOT_REGISTRY, "top");
  assert.fieldEquals("ENSv2Namespace", ns(TOGGLED, top), "canonical", "true");
  assert.fieldEquals("ENSv2Namespace", ns(TOGGLED, top), "materialized", "true");
  assert.fieldEquals("ENSv2NamePath", xNode, "active", "true");
  assert.fieldEquals("ENSv2Registry", TOGGLED, "canonicalMaterializations", "1");
});

test("toggling a link higher up draws on the allowance of every registry below it", () => {
  dataSourceMock.setNetwork("sepolia");
  const UPPER = "0xe1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1";
  const LOWER = "0xe2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2";
  register(ROOT_REGISTRY, "top");
  register(UPPER, "a");
  link(UPPER, "a", LOWER);
  register(LOWER, "x");

  link(ROOT_REGISTRY, "top", UPPER);
  link(ROOT_REGISTRY, "top", EMPTY);
  link(ROOT_REGISTRY, "top", UPPER);

  assert.fieldEquals("ENSv2Registry", UPPER, "aliasMaterializations", "2");
  assert.fieldEquals("ENSv2Registry", LOWER, "aliasMaterializations", "2");
  // A registration costs nothing from the allowance.
  register(LOWER, "y");
  assert.fieldEquals("ENSv2Registry", LOWER, "aliasMaterializations", "2");
  assert.fieldEquals(
    "Domain",
    child(child(child(ROOT_NODE, "top"), "a"), "y").toHexString(),
    "name",
    "y.a.top"
  );
});

test("the RootRegistry and ETHRegistry are not counted", () => {
  dataSourceMock.setNetwork("sepolia");
  register(ROOT_REGISTRY, "eth");
  link(ROOT_REGISTRY, "eth", ETH_REGISTRY);
  let eth = Address.fromString(ETH_REGISTRY).toHexString();
  assert.fieldEquals("ENSv2Registry", eth, "aliasMaterializations", "0");
  assert.fieldEquals("ENSv2Registry", eth, "canonicalMaterializations", "0");
  assert.fieldEquals("ENSv2Registry", eth, "indexed", "true");
  assert.i32Equals(1, materializedCount(ETH_REGISTRY));
});

// ---------------------------------------------------------------------------
// Re-registration
// ---------------------------------------------------------------------------

function setUpLapsedNameWithSubregistry(subregistry: string): Bytes {
  register(ROOT_REGISTRY, "eth");
  link(ROOT_REGISTRY, "eth", ETH_REGISTRY);
  register(ETH_REGISTRY, "x", OWNER);
  link(ETH_REGISTRY, "x", subregistry);
  register(subregistry, "sub", OWNER);
  let xNode = child(child(ROOT_NODE, "eth"), "x");
  assert.fieldEquals("Domain", child(xNode, "sub").toHexString(), "owner", OWNER.toLowerCase());
  assert.fieldEquals("Domain", xNode.toHexString(), "subdomainCount", "1");
  return xNode;
}

// x.eth lapses (no event) and someone else registers it. _register()
// overwrites entry.subregistry and emits SubregistryUpdated only for a
// non-zero value.
test("re-registering a name retires the names of its previous subregistry", () => {
  dataSourceMock.setNetwork("sepolia");
  const REG_A = "0xd0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0";
  let xNode = setUpLapsedNameWithSubregistry(REG_A);
  let subNode = child(xNode, "sub");

  handleLabelRegistered(
    createLabelRegisteredEvent(
      ETH_REGISTRY,
      tokenOf("x").plus(BigInt.fromI32(1)),
      lh("x"),
      "x",
      OWNER_2
    )
  );

  assert.fieldEquals("Domain", xNode.toHexString(), "owner", OWNER_2.toLowerCase());
  assert.fieldEquals("Domain", xNode.toHexString(), "subdomainCount", "0");
  assert.fieldEquals("ENSv2Namespace", ns(REG_A, xNode), "active", "false");
  assert.fieldEquals("ENSv2NamePath", subNode.toHexString(), "active", "false");
  assert.fieldEquals("Domain", subNode.toHexString(), "owner", EMPTY);
  assert.i32Equals(0, materializedCount(REG_A));
  let linkId = nameSlotId(Address.fromString(ETH_REGISTRY), toSlotId(tokenOf("x")))
    .concat(Address.fromString(REG_A))
    .toHexString();
  assert.fieldEquals("ENSv2NamespaceLink", linkId, "active", "false");

  // The new holder's own subregistry starts clean.
  const REG_B = "0xd1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1";
  link(ETH_REGISTRY, "x", REG_B);
  register(REG_B, "sub", OWNER_2);
  assert.fieldEquals("ENSv2NamePath", subNode.toHexString(), "active", "true");
  assert.fieldEquals("Domain", subNode.toHexString(), "owner", OWNER_2.toLowerCase());
  assert.fieldEquals("Domain", xNode.toHexString(), "subdomainCount", "1");
});

test("a re-registration that sets the same subregistry again keeps its names", () => {
  dataSourceMock.setNetwork("sepolia");
  const REG_A = "0xd2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2";
  let xNode = setUpLapsedNameWithSubregistry(REG_A);
  let subNode = child(xNode, "sub");

  // register(label, owner, REG_A, ...): LabelRegistered, then
  // SubregistryUpdated in the same call.
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ETH_REGISTRY,
      tokenOf("x").plus(BigInt.fromI32(1)),
      lh("x"),
      "x",
      OWNER_2
    )
  );
  assert.fieldEquals("ENSv2NamePath", subNode.toHexString(), "active", "false");
  link(ETH_REGISTRY, "x", REG_A);

  assert.fieldEquals("ENSv2Namespace", ns(REG_A, xNode), "materialized", "true");
  assert.fieldEquals("ENSv2NamePath", subNode.toHexString(), "active", "true");
  assert.fieldEquals("Domain", subNode.toHexString(), "owner", OWNER.toLowerCase());
  assert.fieldEquals("Domain", xNode.toHexString(), "subdomainCount", "1");
});
