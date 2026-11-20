// v2.3.5: at most MAX_NAMES_BELOW_NAME names are projected below one name
// (ensv2Paths.ts, ENSv2SubtreeSize).
//
// Removing a link, an unregistration and a re-registration retire every name
// below one slot in a single handler, and a handler never gives memory back.
// Without a limit on what can be below a slot, a large enough subtree ends
// in a handler that cannot complete, which stops the subgraph. So the names
// below every name are counted, per path and per slot, and a name that would
// take a slot above it past the limit is not projected.
import {
  Address,
  BigInt,
  ByteArray,
  Bytes,
  crypto,
  ethereum,
  log,
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
  handleLabelUnregistered,
  handleParentUpdated,
  handleSubregistryUpdated,
  handleTokenResource,
  handleTransferBatch,
  handleTransferSingle,
} from "../src/ensv2Registry";
import {
  ParentUpdated,
  TransferBatch,
} from "../src/types/RootRegistry/PermissionedRegistry";
import {
  MAX_BACKFILL_SLOTS_PER_EVENT,
  MAX_NAMES_BELOW_NAME,
  NAMESPACE_UNITS,
  setMaxBackfillSlotsPerEventForTesting,
  setMaxNamesBelowNameForTesting,
} from "../src/ensv2Paths";
import {
  namespaceId,
  namespacePathIndexId,
  nameSlotId,
  pathNamehash,
  toSlotId,
} from "../src/ensv2Utils";
import {
  Domain,
  ENSv2NamePath,
  ENSv2Namespace,
  ENSv2NamespacePathIndex,
  ENSv2NameSlot,
  ENSv2Registry,
  ENSv2SubtreeSize,
} from "../src/types/schema";
import {
  createLabelRegisteredEvent,
  createLabelUnregisteredEvent,
  createSubregistryUpdatedEvent,
  createTokenResourceEvent,
  createTransferSingleEvent,
} from "./ensv2TestEvents";

import {
  assertCountsMatchRecount,
  below,
  child,
  EMPTY,
  ETH_REGISTRY,
  isRegistered,
  lh,
  link,
  ns,
  OWNER,
  pathCountOf,
  R1,
  R2,
  register,
  ROOT_NODE,
  ROOT_REGISTRY,
  RUN_LABELS,
  RUN_REGISTRIES,
  setUpEth,
  slotId,
  tokenOf,
  unregister,
} from "./ensv2FuzzHarness";

afterEach(() => {
  setMaxNamesBelowNameForTesting(MAX_NAMES_BELOW_NAME);
  setMaxBackfillSlotsPerEventForTesting(MAX_BACKFILL_SLOTS_PER_EVENT);
  dataSourceMock.resetValues();
  clearStore();
});

// What is counted below a name: one unit for each active name, and one for
// each subregistry projected under a name (a namespace that is materialised),
// which costs about as much to retire as two names.
test("names and projected subregistries below a name are counted on the name and on its slot, at every depth, and uncounted when they are retired", () => {
  let eth = setUpEth();
  register(ETH_REGISTRY, "x");
  let x = child(eth, "x");
  link(ETH_REGISTRY, "x", R1);
  // The subregistry itself.
  assert.i32Equals(NAMESPACE_UNITS, below(x));
  register(R1, "a");
  register(R1, "b");
  register(R1, "c");

  assert.i32Equals(5, below(x));
  assert.i32Equals(5, below(slotId(ETH_REGISTRY, "x")));
  assert.i32Equals(3, Domain.load(x.toHexString())!.subdomainCount);
  // Names directly under the RootRegistry are not counted: every .eth
  // registration would rewrite the row.
  assert.notInStore("ENSv2SubtreeSize", eth.toHexString());
  assert.notInStore("ENSv2SubtreeSize", slotId(ROOT_REGISTRY, "eth").toHexString());

  let a = child(x, "a");
  link(R1, "a", R2);
  register(R2, "p");
  register(R2, "q");
  assert.i32Equals(9, below(x));
  assert.i32Equals(9, below(slotId(ETH_REGISTRY, "x")));
  assert.i32Equals(4, below(a));
  assert.i32Equals(4, below(slotId(R1, "a")));

  // A link removed: the subregistry and the names below it go, the name
  // itself stays.
  link(R1, "a", EMPTY);
  assert.i32Equals(5, below(x));
  assert.i32Equals(0, below(a));
  assert.i32Equals(0, below(slotId(R1, "a")));
  // Made again: backfilled and counted again.
  link(R1, "a", R2);
  assert.i32Equals(9, below(x));
  assert.i32Equals(4, below(a));

  unregister(R1, "b");
  assert.i32Equals(8, below(x));
  // An unregistered name takes everything below it along.
  unregister(R1, "a");
  assert.i32Equals(3, below(x));
  assert.i32Equals(3, below(slotId(ETH_REGISTRY, "x")));
  assert.i32Equals(0, below(a));
  assert.i32Equals(0, below(slotId(R1, "a")));
  assert.fieldEquals("ENSv2NamePath", child(a, "p").toHexString(), "active", "false");
  assert.fieldEquals("Domain", child(a, "p").toHexString(), "owner", EMPTY);

  link(ETH_REGISTRY, "x", EMPTY);
  assert.i32Equals(0, below(x));
  assert.i32Equals(0, below(slotId(ETH_REGISTRY, "x")));
  assert.i32Equals(0, Domain.load(x.toHexString())!.subdomainCount);
  assert.fieldEquals("Domain", child(x, "c").toHexString(), "owner", EMPTY);
});

test("a name with the maximum below it takes no more, and a freed place is filled when the link is emitted again", () => {
  let eth = setUpEth();
  register(ETH_REGISTRY, "x");
  let x = child(eth, "x");
  link(ETH_REGISTRY, "x", R1);
  // The subregistry and two names.
  setMaxNamesBelowNameForTesting(NAMESPACE_UNITS + 2);

  register(R1, "a");
  register(R1, "b");
  register(R1, "c");

  // "c" is registered, and in the native slot graph, but has no path or
  // Domain.
  assert.fieldEquals("ENSv2NameSlot", slotId(R1, "c").toHexString(), "status", "REGISTERED");
  assert.i32Equals(0, pathCountOf(R1, "c"));
  assert.notInStore("ENSv2NamePath", child(x, "c").toHexString());
  assert.notInStore("Domain", child(x, "c").toHexString());
  assert.i32Equals(4, below(x));
  assert.fieldEquals("ENSv2Namespace", ns(R1, x), "materialized", "true");
  assert.fieldEquals("ENSv2Namespace", ns(R1, x), "backfilled", "false");
  assert.fieldEquals("ENSv2Namespace", ns(R1, x), "backfillCursor", "2");
  // Nothing to do until a place is free: not queued.
  assert.entityCount("ENSv2BackfillTask", 0);

  // A place freed by an unregistration goes to the next registration...
  unregister(R1, "a");
  assert.i32Equals(3, below(x));
  register(R1, "d");
  assert.i32Equals(1, pathCountOf(R1, "d"));
  assert.i32Equals(4, below(x));

  // ...and emitting the link again finds no room for "c" while it is full,
  link(ETH_REGISTRY, "x", R1);
  assert.notInStore("ENSv2NamePath", child(x, "c").toHexString());
  assert.fieldEquals("ENSv2Namespace", ns(R1, x), "backfilled", "false");
  // but projects it once there is.
  unregister(R1, "b");
  link(ETH_REGISTRY, "x", R1);
  assert.fieldEquals("Domain", child(x, "c").toHexString(), "name", "c.x.eth");
  assert.fieldEquals("ENSv2Namespace", ns(R1, x), "backfilled", "true");
  assert.i32Equals(4, below(x));
});

test("the limit covers every depth below a name", () => {
  let eth = setUpEth();
  register(ETH_REGISTRY, "x");
  let x = child(eth, "x");
  link(ETH_REGISTRY, "x", R1);
  register(R1, "a");
  let a = child(x, "a");
  link(R1, "a", R2);
  // Below x.eth so far: its subregistry, a.x.eth, and that name's
  // subregistry.
  assert.i32Equals(5, below(x));
  setMaxNamesBelowNameForTesting(7);

  register(R2, "p");
  register(R2, "q");
  // a.x.eth has four units below it and room for more; x.eth has seven.
  register(R2, "r");

  assert.i32Equals(7, below(x));
  assert.i32Equals(4, below(a));
  assert.notInStore("ENSv2NamePath", child(a, "r").toHexString());
  assert.fieldEquals("ENSv2Namespace", ns(R2, a), "backfilled", "false");
  // The registry directly under x.eth is full as well.
  register(R1, "b");
  assert.notInStore("ENSv2NamePath", child(x, "b").toHexString());
});

// A subregistry is refused like a name when there is no room for it: the
// link is recorded, nothing below it is projected.
test("a subregistry is not projected under a name that is full", () => {
  let eth = setUpEth();
  register(ETH_REGISTRY, "x");
  let x = child(eth, "x");
  link(ETH_REGISTRY, "x", R1);
  register(R1, "a");
  register(R2, "p");
  let a = child(x, "a");
  setMaxNamesBelowNameForTesting(4);

  link(R1, "a", R2);
  assert.fieldEquals("ENSv2Namespace", ns(R2, a), "active", "true");
  assert.fieldEquals("ENSv2Namespace", ns(R2, a), "materialized", "false");
  assert.notInStore("ENSv2NamePath", child(a, "p").toHexString());
  assert.i32Equals(3, below(x));
  assert.i32Equals(0, below(a));
  // It used up none of the registry's lifetime projections.
  assert.fieldEquals("ENSv2Registry", R2, "aliasMaterializations", "0");

  // With room, emitting the link again projects it.
  setMaxNamesBelowNameForTesting(6);
  link(R1, "a", R2);
  assert.fieldEquals("ENSv2Namespace", ns(R2, a), "materialized", "true");
  assert.fieldEquals("Domain", child(a, "p").toHexString(), "name", "p.a.x.eth");
  assert.i32Equals(6, below(x));
});

// A registry reached under several names gives each of its slots several
// paths. One event retires what is below all of them, so the limit is on
// the slot's total.
test("the limit is on a slot's total over every name it is reached under", () => {
  let eth = setUpEth();
  register(ETH_REGISTRY, "x");
  register(ETH_REGISTRY, "y");
  let x = child(eth, "x");
  let y = child(eth, "y");
  link(ETH_REGISTRY, "x", R1);
  link(ETH_REGISTRY, "y", R1);
  register(R1, "s");
  link(R1, "s", R2);
  // R2 is projected under s.x.eth and under s.y.eth: both count on the
  // slot.
  assert.i32Equals(2 * NAMESPACE_UNITS, below(slotId(R1, "s")));
  setMaxNamesBelowNameForTesting(7);

  register(R2, "p");
  assert.i32Equals(6, below(slotId(R1, "s")));
  assert.i32Equals(3, below(child(x, "s")));
  assert.i32Equals(3, below(child(y, "s")));
  // "q" fits under s.x.eth, which takes the slot to seven, and no longer
  // under s.y.eth.
  register(R2, "q");
  assert.i32Equals(1, pathCountOf(R2, "q"));
  assert.i32Equals(7, below(slotId(R1, "s")));
  assert.fieldEquals("Domain", child(child(x, "s"), "q").toHexString(), "name", "q.s.x.eth");
  assert.notInStore("ENSv2NamePath", child(child(y, "s"), "q").toHexString());

  // Unregistering "s" retires all of it in one event. What stays below
  // x.eth and y.eth is the registry they both point at.
  unregister(R1, "s");
  assert.i32Equals(0, below(slotId(R1, "s")));
  assert.i32Equals(NAMESPACE_UNITS, below(x));
  assert.i32Equals(NAMESPACE_UNITS, below(y));
  assert.fieldEquals("Domain", child(child(x, "s"), "q").toHexString(), "owner", EMPTY);
  assert.fieldEquals("Domain", child(child(y, "s"), "p").toHexString(), "owner", EMPTY);
});

test("a registry linked with more names than fit below the name is projected up to the limit", () => {
  let eth = setUpEth();
  register(ETH_REGISTRY, "x");
  let x = child(eth, "x");
  let labels = ["a", "b", "c", "d", "e"];
  for (let i = 0; i < labels.length; i++) {
    register(R1, labels[i]);
  }
  // The subregistry and three names.
  setMaxNamesBelowNameForTesting(NAMESPACE_UNITS + 3);

  link(ETH_REGISTRY, "x", R1);

  assert.i32Equals(5, below(x));
  assert.fieldEquals("Domain", child(x, "c").toHexString(), "name", "c.x.eth");
  assert.notInStore("ENSv2NamePath", child(x, "d").toHexString());
  assert.fieldEquals("ENSv2Namespace", ns(R1, x), "backfilled", "false");
  assert.fieldEquals("ENSv2Namespace", ns(R1, x), "backfillCursor", "3");
  assert.fieldEquals("ENSv2Namespace", ns(R1, x), "backfillQueued", "false");
  assert.entityCount("ENSv2BackfillTask", 0);

  // Removing the link retires exactly what was projected.
  link(ETH_REGISTRY, "x", EMPTY);
  assert.i32Equals(0, below(x));
  assert.fieldEquals("ENSv2NamePath", child(x, "a").toHexString(), "active", "false");
});

test("a .eth name itself is never refused: only ENS governance can unlink the ETHRegistry", () => {
  setUpEth();
  setMaxNamesBelowNameForTesting(0);
  register(ETH_REGISTRY, "x");
  register(ETH_REGISTRY, "y");
  assert.i32Equals(1, pathCountOf(ETH_REGISTRY, "x"));
  assert.i32Equals(1, pathCountOf(ETH_REGISTRY, "y"));
});

// Anyone can point a name of theirs at the ETHRegistry, for one
// transaction. Were it projected there, every .eth name registered from
// then on would exist a second time under that name, and so would
// everything below it: a name in the registry of foo.eth would count twice
// against what may sit below foo.eth and take one of that registry's alias
// places, nine times and all of them with eight such names.
test("the ETHRegistry is not projected under a name that points at it", () => {
  let eth = setUpEth();
  for (let i = 0; i < 8; i++) {
    register(ETH_REGISTRY, "squat" + i.toString());
    link(ETH_REGISTRY, "squat" + i.toString(), ETH_REGISTRY);
  }
  let squat = child(eth, "squat0");
  // The link is recorded, as for any link that is refused.
  assert.fieldEquals("ENSv2Namespace", ns(ETH_REGISTRY, squat), "active", "true");
  assert.fieldEquals("ENSv2Namespace", ns(ETH_REGISTRY, squat), "materialized", "false");
  assert.fieldEquals("ENSv2Registry", ETH_REGISTRY.toLowerCase(), "namespaceCount", "9");

  register(ETH_REGISTRY, "foo");
  let foo = child(eth, "foo");
  link(ETH_REGISTRY, "foo", R1);
  register(R1, "a");

  // One path each, and the registry of foo.eth has its alias places left.
  assert.i32Equals(1, pathCountOf(ETH_REGISTRY, "foo"));
  assert.i32Equals(1, pathCountOf(R1, "a"));
  assert.notInStore("ENSv2NamePath", child(squat, "foo").toHexString());
  assert.i32Equals(NAMESPACE_UNITS + 1, below(foo));
  assert.i32Equals(0, below(squat));
  assert.i32Equals(
    1,
    ENSv2Registry.load(Address.fromString(R1))!.materializedNamespaces.length
  );
  assert.fieldEquals("Domain", child(foo, "a").toHexString(), "name", "a.foo.eth");
});

// The RootRegistry is the top of the tree.
test("the RootRegistry is not projected under a name that points at it", () => {
  let eth = setUpEth();
  register(ETH_REGISTRY, "x");
  let x = child(eth, "x");
  link(ETH_REGISTRY, "x", ROOT_REGISTRY);
  assert.fieldEquals("ENSv2Namespace", ns(ROOT_REGISTRY, x), "materialized", "false");
  assert.notInStore("ENSv2NamePath", child(x, "eth").toHexString());
  assert.i32Equals(0, below(x));
});

// Only "eth" is exempt from the count. Another TLD is limited like any
// name, so that what is below it can be retired in one handler whoever
// comes to hold it.
test("names directly under a TLD other than eth count against that TLD", () => {
  dataSourceMock.setNetwork("sepolia");
  register(ROOT_REGISTRY, "tld");
  let tld = child(ROOT_NODE, "tld");
  link(ROOT_REGISTRY, "tld", R1);
  setMaxNamesBelowNameForTesting(NAMESPACE_UNITS + 2);
  register(R1, "a");
  register(R1, "b");
  register(R1, "c");

  assert.i32Equals(NAMESPACE_UNITS + 2, below(tld));
  assert.i32Equals(NAMESPACE_UNITS + 2, below(slotId(ROOT_REGISTRY, "tld")));
  assert.notInStore("ENSv2NamePath", child(tld, "c").toHexString());

  link(ROOT_REGISTRY, "tld", EMPTY);
  assert.i32Equals(0, below(tld));
  assert.fieldEquals("ENSv2NamePath", child(tld, "a").toHexString(), "active", "false");
});

// ---------------------------------------------------------------------------
// What one name costs
// ---------------------------------------------------------------------------

function registryAddress(n: i32): string {
  let hex = (n + 16).toString(16);
  let out = "0x";
  for (let i = 0; i < 20; i++) {
    out += hex;
  }
  return out;
}

// Every name below a name repeats it, in its path and in its Domain. A label
// of ten thousand characters costs its registrant next to nothing and would
// multiply what it takes to retire or backfill everything below it.
test("a name longer than 255 bytes is not projected; a .eth name itself is exempt", () => {
  let eth = setUpEth();
  let long = "l".repeat(300);
  register(ETH_REGISTRY, long);
  let longNode = child(eth, long);
  assert.fieldEquals("Domain", longNode.toHexString(), "name", long + ".eth");

  // Nothing is projected below it: every name there would carry it.
  link(ETH_REGISTRY, long, R1);
  register(R1, "a");
  assert.i32Equals(0, pathCountOf(R1, "a"));
  assert.notInStore("ENSv2NamePath", child(longNode, "a").toHexString());
  assert.notInStore("Domain", child(longNode, "a").toHexString());
  // Such a name is never projected, so the namespace is not waiting for it.
  assert.fieldEquals("ENSv2Namespace", ns(R1, longNode), "backfilled", "true");

  // Below x.eth (6 bytes with the dot) a label may be 249 bytes. A
  // backfill passes over the longer one and goes on.
  register(ETH_REGISTRY, "x");
  let x = child(eth, "x");
  let tooLong = "s".repeat(250);
  let fits = "t".repeat(249);
  register(R2, tooLong);
  register(R2, fits);
  link(ETH_REGISTRY, "x", R2);
  assert.notInStore("ENSv2NamePath", child(x, tooLong).toHexString());
  assert.fieldEquals("Domain", child(x, fits).toHexString(), "labelName", fits);
  assert.fieldEquals("ENSv2Namespace", ns(R2, x), "backfilled", "true");
  assert.i32Equals(NAMESPACE_UNITS + 1, below(x));
});

test("names stop being projected at 255 bytes however deep the tree goes", () => {
  let eth = setUpEth();
  register(ETH_REGISTRY, "x");
  let node = child(eth, "x");
  let parentRegistry = ETH_REGISTRY;
  let parentLabel = "x";
  // Each level adds "a.": level n is 2n + 5 bytes, so level 125 is the last
  // that fits.
  let deepest = node;
  for (let level = 1; level <= 130; level++) {
    let registry = registryAddress(level);
    link(parentRegistry, parentLabel, registry);
    register(registry, "a");
    node = child(node, "a");
    if (level == 125) {
      deepest = node;
      assert.fieldEquals("ENSv2NamePath", node.toHexString(), "active", "true");
      assert.fieldEquals("ENSv2NamePath", node.toHexString(), "depth", "126");
    }
    if (level == 126) {
      assert.notInStore("ENSv2NamePath", node.toHexString());
    }
    parentRegistry = registry;
    parentLabel = "a";
  }
  // 125 names and the 125 registries they are in, and the registry linked
  // below the last name, which holds nothing that fits.
  assert.i32Equals(125 + 126 * NAMESPACE_UNITS, below(child(eth, "x")));
  assert.i32Equals(NAMESPACE_UNITS, below(deepest));
});

// Counting a name on its ancestors is one load and one save per level, so
// the same number of names costs more the deeper they sit.
test("a backfill deep in the tree projects fewer names per event", () => {
  let eth = setUpEth();
  register(ETH_REGISTRY, "x");
  let node = child(eth, "x");
  let parentRegistry = ETH_REGISTRY;
  let parentLabel = "x";
  for (let level = 1; level <= 17; level++) {
    let registry = registryAddress(level);
    link(parentRegistry, parentLabel, registry);
    register(registry, "a");
    node = child(node, "a");
    parentRegistry = registry;
    parentLabel = "a";
  }
  const WIDE = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
  for (let i = 0; i < 10; i++) {
    register(WIDE, "n" + i.toString());
  }

  // Eighteen names above: two units per slot, five slots for a budget of
  // ten.
  setMaxBackfillSlotsPerEventForTesting(10);
  link(parentRegistry, "a", WIDE);
  assert.fieldEquals("ENSv2Namespace", ns(WIDE, node), "backfillCursor", "5");
  assert.fieldEquals("ENSv2Namespace", ns(WIDE, node), "backfilled", "false");

  // Two levels deep the same budget covers all ten.
  register(ETH_REGISTRY, "y");
  link(ETH_REGISTRY, "y", WIDE);
  assert.fieldEquals("ENSv2Namespace", ns(WIDE, child(eth, "y")), "backfilled", "true");
});

// A contract that only emits the events can register one labelhash under
// two token ids. A live name stays with the slot that has it: handing it to
// the second would bring everything below it onto a slot that never passed
// the limit for it, and with every event the two would trade it again.
test("a second slot with the same labelhash does not take a live name", () => {
  let eth = setUpEth();
  register(ETH_REGISTRY, "x");
  let x = child(eth, "x");
  link(ETH_REGISTRY, "x", R1);
  for (let i = 0; i < 6; i++) {
    handleLabelRegistered(
      createLabelRegisteredEvent(R1, tokenOf("a"), lh("a"), "a", OWNER)
    );
    handleLabelRegistered(
      createLabelRegisteredEvent(R1, tokenOf("b"), lh("a"), "a", OWNER)
    );
  }
  assert.i32Equals(1, pathCountOf(R1, "a"));
  assert.i32Equals(0, pathCountOf(R1, "b"));
  assert.fieldEquals(
    "ENSv2NamePath",
    child(x, "a").toHexString(),
    "slot",
    slotId(R1, "a").toHexString()
  );
  assert.fieldEquals("ENSv2Namespace", ns(R1, x), "pathCount", "1");
  assert.i32Equals(NAMESPACE_UNITS + 1, below(x));

  // Once the first slot's name is retired, the other may have it, and the
  // path moves from one slot's list to the other's.
  unregister(R1, "a");
  handleLabelRegistered(
    createLabelRegisteredEvent(R1, tokenOf("b"), lh("a"), "a", OWNER)
  );
  assert.i32Equals(0, pathCountOf(R1, "a"));
  assert.i32Equals(1, pathCountOf(R1, "b"));
  assert.fieldEquals("ENSv2NamePath", child(x, "a").toHexString(), "slotIndex", "0");
  assert.fieldEquals("ENSv2NamePath", child(x, "a").toHexString(), "active", "true");
  assert.i32Equals(NAMESPACE_UNITS + 1, below(x));
});

// A registry derives a token id from the label, so a slot has one labelhash
// for life. Every other labelhash an emitter gave a slot would be another
// path on it, and each handler for the slot walks them all.
test("a slot keeps the labelhash it was created with", () => {
  let eth = setUpEth();
  register(ETH_REGISTRY, "x");
  let x = child(eth, "x");
  link(ETH_REGISTRY, "x", R1);
  register(R1, "a");
  let labels = ["p", "q", "r", "s"];
  for (let i = 0; i < labels.length; i++) {
    handleLabelRegistered(
      createLabelRegisteredEvent(R1, tokenOf("a"), lh(labels[i]), labels[i], OWNER)
    );
    unregister(R1, "a");
    register(R1, "a");
  }
  assert.i32Equals(1, pathCountOf(R1, "a"));
  assert.fieldEquals(
    "ENSv2NameSlot",
    slotId(R1, "a").toHexString(),
    "labelhash",
    lh("a").toHexString()
  );
  assert.notInStore("ENSv2NamePath", child(x, "p").toHexString());
  assert.notInStore("Domain", child(x, "s").toHexString());
  assert.fieldEquals("Domain", child(x, "a").toHexString(), "name", "a.x.eth");
});

// ---------------------------------------------------------------------------
// Transfers that move nothing
// ---------------------------------------------------------------------------

const OTHER = "0xF0205A3A3b2A69De6Dbf7f01ED13B2108B2c4321";

function transferSingle(registry: string, label: string, to: string, value: i32): void {
  let event = createTransferSingleEvent(registry, tokenOf(label), OWNER, to);
  event.parameters[4] = new ethereum.EventParam(
    "value",
    ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(value))
  );
  handleTransferSingle(event);
}

function transferBatch(registry: string, labels: Array<string>, values: Array<i32>, to: string): void {
  let mockEvent = newMockEvent();
  let event = new TransferBatch(
    Address.fromString(registry),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  let ids = new Array<BigInt>();
  let amounts = new Array<BigInt>();
  for (let i = 0; i < labels.length; i++) {
    ids.push(tokenOf(labels[i]));
    amounts.push(BigInt.fromI32(values[i]));
  }
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("operator", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  event.parameters.push(
    new ethereum.EventParam("from", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  event.parameters.push(
    new ethereum.EventParam("to", ethereum.Value.fromAddress(Address.fromString(to)))
  );
  event.parameters.push(
    new ethereum.EventParam("ids", ethereum.Value.fromUnsignedBigIntArray(ids))
  );
  event.parameters.push(
    new ethereum.EventParam("values", ethereum.Value.fromUnsignedBigIntArray(amounts))
  );
  handleTransferBatch(event);
}

// ERC1155Singleton._update changes the owner only when value > 0 and emits
// the event either way, so a holder can have the registry emit a "transfer"
// of their own name to any address.
test("a transfer of value 0 does not change the owner", () => {
  let eth = setUpEth();
  let labels = ["x", "y", "z"];
  for (let i = 0; i < labels.length; i++) {
    register(ETH_REGISTRY, labels[i]);
    handleTokenResource(
      createTokenResourceEvent(ETH_REGISTRY, tokenOf(labels[i]), BigInt.fromI32(i + 1))
    );
  }
  let x = child(eth, "x").toHexString();
  let y = child(eth, "y").toHexString();
  let z = child(eth, "z").toHexString();

  transferSingle(ETH_REGISTRY, "x", OTHER, 0);
  assert.fieldEquals("Domain", x, "owner", OWNER.toLowerCase());
  assert.fieldEquals("ENSv2NameSlot", slotId(ETH_REGISTRY, "x").toHexString(), "owner", OWNER.toLowerCase());
  transferSingle(ETH_REGISTRY, "x", OTHER, 1);
  assert.fieldEquals("Domain", x, "owner", OTHER.toLowerCase());

  // In a batch: "y" moves, once, and "z" does not.
  transferBatch(ETH_REGISTRY, ["y", "y", "z"], [1, 1, 0], OTHER);
  assert.fieldEquals("Domain", y, "owner", OTHER.toLowerCase());
  assert.fieldEquals("Domain", z, "owner", OWNER.toLowerCase());
});

// Retiring a namespace walks the list of its paths in one handler. Were the
// list append-only, registering and unregistering names in turn would
// lengthen it without limit while the number of live names stayed small.
test("a namespace lists its live names only, however many it has had", () => {
  let eth = setUpEth();
  register(ETH_REGISTRY, "x");
  let x = child(eth, "x");
  link(ETH_REGISTRY, "x", R1);
  let labels = ["a", "b", "c", "d"];
  for (let round = 0; round < 3; round++) {
    for (let i = 0; i < labels.length; i++) {
      register(R1, labels[i]);
      unregister(R1, labels[i]);
    }
  }
  assert.fieldEquals("ENSv2Namespace", ns(R1, x), "pathCount", "0");

  register(R1, "a");
  register(R1, "b");
  register(R1, "c");
  assert.fieldEquals("ENSv2Namespace", ns(R1, x), "pathCount", "3");
  assert.fieldEquals("ENSv2NamePath", child(x, "c").toHexString(), "namespaceIndex", "2");

  // A name retired on its own gives its place to the last one.
  unregister(R1, "a");
  assert.fieldEquals("ENSv2Namespace", ns(R1, x), "pathCount", "2");
  assert.fieldEquals("ENSv2NamePath", child(x, "a").toHexString(), "namespaceIndex", "-1");
  assert.fieldEquals("ENSv2NamePath", child(x, "c").toHexString(), "namespaceIndex", "0");
  assert.fieldEquals("ENSv2NamePath", child(x, "b").toHexString(), "namespaceIndex", "1");

  // Removing the link retires the two that are left and empties the list.
  link(ETH_REGISTRY, "x", EMPTY);
  assert.fieldEquals("ENSv2Namespace", ns(R1, x), "pathCount", "0");
  assert.fieldEquals("ENSv2NamePath", child(x, "b").toHexString(), "active", "false");
  assert.fieldEquals("ENSv2NamePath", child(x, "c").toHexString(), "active", "false");
  assert.fieldEquals("Domain", child(x, "c").toHexString(), "owner", EMPTY);

  // Linked again, the names that are still registered come back.
  link(ETH_REGISTRY, "x", R1);
  assert.fieldEquals("ENSv2Namespace", ns(R1, x), "pathCount", "2");
  assert.fieldEquals("ENSv2NamePath", child(x, "a").toHexString(), "active", "false");
  assert.fieldEquals("ENSv2NamePath", child(x, "b").toHexString(), "active", "true");
});

// ---------------------------------------------------------------------------
// The counts against a recount
// ---------------------------------------------------------------------------

function runRandomEvents(seed: u32, steps: i32, limit: i32): void {
  let eth = setUpEth();
  setMaxNamesBelowNameForTesting(limit);
  // Small, so that unfinished backfills and the queue are part of the run.
  setMaxBackfillSlotsPerEventForTesting(3);
  let registries = RUN_REGISTRIES;
  let labels = RUN_LABELS;
  let state: u32 = seed;
  let mostActive = 0;
  for (let step = 0; step < steps; step++) {
    state = state * 1664525 + 1013904223;
    let op = (state >> 8) % 8;
    state = state * 1664525 + 1013904223;
    let registry = registries[(state >> 8) % 4];
    state = state * 1664525 + 1013904223;
    let label = labels[(state >> 8) % 4];
    state = state * 1664525 + 1013904223;
    // Links point at the three user registries, the one they are in
    // included.
    let target = registries[1 + ((state >> 8) % 3)];

    if (op < 3) {
      register(registry, label);
    } else if (op < 4) {
      if (isRegistered(registry, label)) {
        unregister(registry, label);
      }
    } else if (op < 7) {
      if (isRegistered(registry, label)) {
        link(registry, label, target);
      }
    } else {
      if (isRegistered(registry, label)) {
        link(registry, label, EMPTY);
      }
    }
    let active = assertCountsMatchRecount(eth, step, limit);
    if (active > mostActive) {
      mostActive = active;
    }
  }
  // The run built real trees, not a handful of names.
  assert.assertTrue(mostActive > 20);
}

test("the counts agree with a recount after every one of 400 random events", () => {
  runRandomEvents(12345, 400, MAX_NAMES_BELOW_NAME);
});

test("the counts agree with a recount when the limit keeps refusing names", () => {
  runRandomEvents(987654321, 400, 16);
});
