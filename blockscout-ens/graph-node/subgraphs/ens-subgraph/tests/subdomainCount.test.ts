// v2.3.5: Domain.subdomainCount is the number of a name's direct children
// that are live, and changes only when a child becomes live or stops being
// so (src/subdomainCount.ts).
//
// The original mapping took a child off its parent on every save of a row in
// the deleted state, and never counted a re-created subname again, so counts
// drifted, below zero for some names.
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
  handleNewOwner,
  handleNewResolver,
  handleTransfer,
} from "../src/ensRegistry";
import {
  handleLabelRegistered,
  handleLabelUnregistered,
  handleSubregistryUpdated,
  handleTokenResource,
  handleTransferSingle,
} from "../src/ensv2Registry";
import {
  MAX_BACKFILL_SLOTS_PER_EVENT,
  setMaxBackfillSlotsPerEventForTesting,
} from "../src/ensv2Paths";
import {
  MAX_LEGACY_ROWS_VOIDED,
  setMaxLegacyRowsVoidedForTesting,
} from "../src/legacyGovernance";
import { countsAsChild } from "../src/subdomainCount";
import { nameSlotId, pathNamehash, toSlotId } from "../src/ensv2Utils";
import { Domain, ENSv2NameSlot } from "../src/types/schema";
import { NewOwner, NewResolver, Transfer } from "../src/types/ENSRegistry/EnsRegistry";
import {
  createLabelRegisteredEvent,
  createLabelUnregisteredEvent,
  createSubregistryUpdatedEvent,
  createTokenResourceEvent,
  createTransferSingleEvent,
} from "./ensv2TestEvents";

const ROOT_REGISTRY = "0xB458D6a3a77919449d03e7A6903C26827c1eC43f";
const ETH_REGISTRY = "0xD4eBcbBdF463C9c45784603Db0dDD499BC44A8B4";
const R1 = "0xa1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
const R2 = "0xa2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2";
const OWNER = "0x89205A3A3b2A69De6Dbf7f01ED13B2108B2c43e7";
const RESOLVER = "0x4976fb03C32e5B8cfe2b6cCB31c09Ba78EBaBa41";
const EMPTY = "0x0000000000000000000000000000000000000000";
const ROOT_NODE = Bytes.fromHexString(
  "0x0000000000000000000000000000000000000000000000000000000000000000"
);
// After the ENSv2 deployment block, so the ENSv1 handlers apply the ENSv2
// governance rules where the two meet.
const V2_ERA_BLOCK = BigInt.fromI32(20000000);

function lh(label: string): Bytes {
  return Bytes.fromByteArray(crypto.keccak256(ByteArray.fromUTF8(label)));
}

function child(parent: Bytes, label: string): Bytes {
  return pathNamehash(parent, lh(label));
}

function tokenOf(label: string): BigInt {
  let hash = lh(label);
  let le = new Bytes(32);
  for (let i = 0; i < 32; i++) {
    le[i] = hash[31 - i];
  }
  return BigInt.fromUnsignedBytes(le);
}

// ENSv1 registry events.

function newOwner(parent: Bytes, label: string, owner: string): void {
  let mockEvent = newMockEvent();
  let event = new NewOwner(
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
    new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(parent))
  );
  event.parameters.push(
    new ethereum.EventParam("label", ethereum.Value.fromFixedBytes(lh(label)))
  );
  event.parameters.push(
    new ethereum.EventParam("owner", ethereum.Value.fromAddress(Address.fromString(owner)))
  );
  handleNewOwner(event);
}

function transfer(node: Bytes, owner: string): void {
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
    new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(node))
  );
  event.parameters.push(
    new ethereum.EventParam("owner", ethereum.Value.fromAddress(Address.fromString(owner)))
  );
  handleTransfer(event);
}

function newResolver(node: Bytes, resolver: string): void {
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
    new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(node))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "resolver",
      ethereum.Value.fromAddress(Address.fromString(resolver))
    )
  );
  handleNewResolver(event);
}

// ENSv2 registry events.

function register(registry: string, label: string): void {
  handleLabelRegistered(
    createLabelRegisteredEvent(registry, tokenOf(label), lh(label), label, OWNER)
  );
}

function unregister(registry: string, label: string): void {
  handleLabelUnregistered(createLabelUnregisteredEvent(registry, tokenOf(label)));
}

function link(registry: string, label: string, subregistry: string): void {
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(registry, tokenOf(label), subregistry)
  );
}

function isRegistered(registry: string, label: string): boolean {
  let slot = ENSv2NameSlot.load(
    nameSlotId(Address.fromString(registry), toSlotId(tokenOf(label)))
  );
  return slot != null && slot.status == "REGISTERED";
}

function count(node: Bytes): i32 {
  return Domain.load(node.toHexString())!.subdomainCount;
}

function exists(node: Bytes): boolean {
  return Domain.load(node.toHexString()) != null;
}

// eth and x.eth in ENSv1; returns namehash("x.eth").
function setUpV1(): Bytes {
  dataSourceMock.setNetwork("sepolia");
  newOwner(ROOT_NODE, "eth", OWNER);
  let eth = child(ROOT_NODE, "eth");
  newOwner(eth, "x", OWNER);
  return child(eth, "x");
}

afterEach(() => {
  setMaxLegacyRowsVoidedForTesting(MAX_LEGACY_ROWS_VOIDED);
  setMaxBackfillSlotsPerEventForTesting(MAX_BACKFILL_SLOTS_PER_EVENT);
  dataSourceMock.resetValues();
  clearStore();
});

// ---------------------------------------------------------------------------
// ENSv1
// ---------------------------------------------------------------------------

test("a subname that is already deleted is not taken off its parent again", () => {
  let x = setUpV1();
  newOwner(x, "a", OWNER);
  newOwner(x, "b", OWNER);
  let a = child(x, "a");
  assert.i32Equals(2, count(x));

  newOwner(x, "a", EMPTY);
  assert.i32Equals(1, count(x));

  // Each of these saved the deleted row again, and each used to cost the
  // parent another child: two more and x.eth read -1 with "b" still there.
  newOwner(x, "a", EMPTY);
  assert.i32Equals(1, count(x));
  transfer(a, EMPTY);
  assert.i32Equals(1, count(x));
  newResolver(a, EMPTY);
  assert.i32Equals(1, count(x));
});

test("a subname that is made again counts again", () => {
  let x = setUpV1();
  newOwner(x, "a", OWNER);
  newOwner(x, "a", EMPTY);
  assert.i32Equals(0, count(x));

  // Used to stay at 0: only a row's first NewOwner counted it.
  newOwner(x, "a", OWNER);
  assert.i32Equals(1, count(x));
  newOwner(x, "a", EMPTY);
  assert.i32Equals(0, count(x));
  transfer(child(x, "a"), OWNER);
  assert.i32Equals(1, count(x));
});

test("a subname created for the zero address is not counted", () => {
  let x = setUpV1();
  newOwner(x, "a", EMPTY);
  assert.assertTrue(exists(child(x, "a")));
  assert.i32Equals(0, count(x));
});

test("a subname without an owner counts while it has a resolver", () => {
  let x = setUpV1();
  newOwner(x, "a", OWNER);
  let a = child(x, "a");
  newResolver(a, RESOLVER);
  newOwner(x, "a", EMPTY);
  assert.i32Equals(1, count(x));

  newResolver(a, EMPTY);
  assert.i32Equals(0, count(x));
  newResolver(a, RESOLVER);
  assert.i32Equals(1, count(x));
  // Setting the same state again changes nothing.
  newResolver(a, RESOLVER);
  assert.i32Equals(1, count(x));
});

test("a deleted name counts while it has a live child, and the change carries upward", () => {
  let x = setUpV1();
  let eth = child(ROOT_NODE, "eth");
  newOwner(x, "a", OWNER);
  let a = child(x, "a");
  newOwner(a, "deep", OWNER);
  assert.i32Equals(1, count(eth));
  assert.i32Equals(1, count(ROOT_NODE));

  // x.eth and a.x.eth lose their owners; deep.a.x.eth keeps both alive.
  transfer(x, EMPTY);
  transfer(a, EMPTY);
  assert.i32Equals(1, count(eth));
  assert.i32Equals(1, count(x));
  assert.i32Equals(1, count(a));

  // The last live name goes, and takes the two above it along.
  newOwner(a, "deep", EMPTY);
  assert.i32Equals(0, count(a));
  assert.i32Equals(0, count(x));
  assert.i32Equals(0, count(eth));
  assert.i32Equals(1, count(ROOT_NODE));

  // And comes back.
  newOwner(a, "deep", OWNER);
  assert.i32Equals(1, count(a));
  assert.i32Equals(1, count(x));
  assert.i32Equals(1, count(eth));
});

// The ENSv1 registry is a tree of hashes with no depth limit.
test("a change carries upward through more levels than any name has labels", () => {
  let x = setUpV1();
  let nodes = new Array<Bytes>();
  let parent = x;
  for (let level = 0; level < 140; level++) {
    newOwner(parent, "l", OWNER);
    parent = child(parent, "l");
    nodes.push(parent);
  }
  // Every level but the last gives up its owner and lives on through the
  // one below.
  for (let level = 0; level < 139; level++) {
    transfer(nodes[level], EMPTY);
  }
  assert.i32Equals(1, count(x));
  assert.i32Equals(1, count(nodes[0]));

  newOwner(nodes[138], "l", EMPTY);
  assert.i32Equals(0, count(nodes[138]));
  assert.i32Equals(0, count(nodes[0]));
  assert.i32Equals(0, count(x));
});

// ---------------------------------------------------------------------------
// Where ENSv1 and ENSv2 meet
// ---------------------------------------------------------------------------

function setUpBoth(): Bytes {
  dataSourceMock.setNetwork("sepolia");
  newOwner(ROOT_NODE, "eth", OWNER);
  register(ROOT_REGISTRY, "eth");
  link(ROOT_REGISTRY, "eth", ETH_REGISTRY);
  return child(ROOT_NODE, "eth");
}

// A retired ENSv2 name reads as deleted. If ENSv1 then gives the row a
// resolver it counts again, through the resolver alone, and a
// re-registration, which first takes the old resolver off, must not count it
// a second time.
test("a retired ENSv2 name that ENSv1 gave a resolver is counted once when it is registered again", () => {
  let eth = setUpBoth();
  register(ETH_REGISTRY, "x");
  let x = child(eth, "x");
  assert.i32Equals(1, count(eth));
  unregister(ETH_REGISTRY, "x");
  assert.i32Equals(0, count(eth));

  newResolver(x, RESOLVER);
  assert.i32Equals(1, count(eth));

  register(ETH_REGISTRY, "x");
  assert.i32Equals(1, count(eth));
  unregister(ETH_REGISTRY, "x");
  assert.i32Equals(0, count(eth));
});

// A registry never mints to the zero address, but a contract that only
// emits the events can register a name to it: the row then becomes live
// when the token is transferred.
test("a name registered to the zero address counts once it has a holder", () => {
  let eth = setUpBoth();
  register(ETH_REGISTRY, "x");
  let x = child(eth, "x");
  link(ETH_REGISTRY, "x", R1);
  register(R1, "keep");
  handleLabelRegistered(
    createLabelRegisteredEvent(R1, tokenOf("a"), lh("a"), "a", EMPTY)
  );
  assert.i32Equals(1, count(x));

  handleTokenResource(createTokenResourceEvent(R1, tokenOf("a"), BigInt.fromI32(1)));
  handleTransferSingle(createTransferSingleEvent(R1, tokenOf("a"), EMPTY, OWNER));
  assert.i32Equals(2, count(x));
  unregister(R1, "a");
  assert.i32Equals(1, count(x));
});

// Reading a name's children loads every row it has ever had, so the sweep
// that clears ENSv1 names below an ENSv2 name must decide from that number
// whether it may read them: the live ones can be few while the rows are
// many.
test("the ENSv1 sweep's limit is on the child rows a name has had, not on the live ones", () => {
  let eth = setUpBoth();
  let names = ["x", "y"];
  for (let n = 0; n < names.length; n++) {
    newOwner(eth, names[n], OWNER);
    let node = child(eth, names[n]);
    newOwner(node, "a", OWNER);
    newOwner(node, "b", OWNER);
    newOwner(node, "c", OWNER);
    newResolver(child(node, "c"), RESOLVER);
    newOwner(node, "a", EMPTY);
    newOwner(node, "b", EMPTY);
    assert.i32Equals(1, count(node));
    assert.fieldEquals("DomainChildRows", node.toHexString(), "rows", "3");
  }
  let underX = child(child(eth, "x"), "c").toHexString();
  let underY = child(child(eth, "y"), "c").toHexString();

  // Three rows, limit three: read and cleared.
  setMaxLegacyRowsVoidedForTesting(3);
  register(ETH_REGISTRY, "x");
  assert.assertNull(Domain.load(underX)!.resolver);

  // Three rows, limit two: not read at all, although only one is live. The
  // row keeps what it had (the documented outcome past the limit), and the
  // name is marked, so ENSv1 cannot write below it again.
  setMaxLegacyRowsVoidedForTesting(2);
  register(ETH_REGISTRY, "y");
  assert.assertNotNull(Domain.load(underY)!.resolver);
  assert.fieldEquals(
    "ENSv2NamePath",
    child(eth, "y").toHexString(),
    "legacySubtreeVoided",
    "true"
  );
});

// ---------------------------------------------------------------------------
// ENSv1 and ENSv2 together, against a recount
// ---------------------------------------------------------------------------

// Walks every row reachable from the root through the stored parent links
// and checks that its subdomainCount is the number of its children that
// count. Returns the largest count seen.
function assertCountsMatchRecount(step: i32): i32 {
  let largest = 0;
  let pending = new Array<string>();
  pending.push(ROOT_NODE.toHexString());
  while (pending.length > 0) {
    let domain = Domain.load(pending.pop())!;
    let children = domain.subdomains.load();
    let want = 0;
    for (let i = 0; i < children.length; i++) {
      if (countsAsChild(children[i])) {
        want++;
      }
      pending.push(children[i].id);
    }
    if (domain.subdomainCount != want) {
      log.error("step {}: {} has subdomainCount {}, {} of its {} children count", [
        step.toString(),
        domain.name === null ? domain.id : (domain.name as string),
        domain.subdomainCount.toString(),
        want.toString(),
        children.length.toString(),
      ]);
    }
    assert.i32Equals(want, domain.subdomainCount);
    if (want > largest) {
      largest = want;
    }
  }
  return largest;
}

function runRandomEvents(seed: u32, steps: i32): void {
  dataSourceMock.setNetwork("sepolia");
  // Small, so that unfinished backfills are part of the run.
  setMaxBackfillSlotsPerEventForTesting(2);
  newOwner(ROOT_NODE, "eth", OWNER);
  let eth = child(ROOT_NODE, "eth");
  register(ROOT_REGISTRY, "eth");
  link(ROOT_REGISTRY, "eth", ETH_REGISTRY);

  let second = ["x", "y"];
  let lower = ["a", "b"];
  let registries = [ETH_REGISTRY, R1, R2];
  let state: u32 = seed;
  let sawTwo = false;
  for (let step = 0; step < steps; step++) {
    state = state * 1664525 + 1013904223;
    let op = (state >> 8) % 12;
    state = state * 1664525 + 1013904223;
    let s = second[(state >> 8) % 2];
    state = state * 1664525 + 1013904223;
    let l1 = lower[(state >> 8) % 2];
    state = state * 1664525 + 1013904223;
    let l2 = lower[(state >> 8) % 2];
    state = state * 1664525 + 1013904223;
    let depth = (state >> 8) % 3;
    state = state * 1664525 + 1013904223;
    let registry = registries[(state >> 8) % 3];

    // The ENSv1 name this step is about, and its parent.
    let parent = eth;
    let label = s;
    if (depth >= 1) {
      parent = child(eth, s);
      label = l1;
    }
    if (depth == 2) {
      parent = child(parent, l1);
      label = l2;
    }
    let node = child(parent, label);
    // The ENSv2 label: .eth names in the ETHRegistry, the rest in R1 and R2.
    let v2Label = registry == ETH_REGISTRY ? s : l1;

    if (op < 2) {
      // The registry only lets a name's owner create names below it.
      if (exists(parent)) {
        newOwner(parent, label, OWNER);
      }
    } else if (op < 4) {
      if (exists(parent)) {
        newOwner(parent, label, EMPTY);
      }
    } else if (op < 5) {
      if (exists(node)) {
        transfer(node, (state >> 20) % 2 == 0 ? OWNER : EMPTY);
      }
    } else if (op < 7) {
      if (exists(node)) {
        newResolver(node, (state >> 20) % 2 == 0 ? RESOLVER : EMPTY);
      }
    } else if (op < 9) {
      register(registry, v2Label);
    } else if (op < 10) {
      if (isRegistered(registry, v2Label)) {
        unregister(registry, v2Label);
      }
    } else if (op < 11) {
      if (isRegistered(registry, v2Label)) {
        link(registry, v2Label, (state >> 20) % 2 == 0 ? R1 : R2);
      }
    } else {
      if (isRegistered(registry, v2Label)) {
        link(registry, v2Label, EMPTY);
      }
    }
    if (assertCountsMatchRecount(step) >= 2) {
      sawTwo = true;
    }
  }
  // The run had names with several live children, not a bare tree.
  assert.assertTrue(sawTwo);
}

test("every subdomainCount agrees with a recount after each of 600 random ENSv1 and ENSv2 events", () => {
  runRandomEvents(2024, 600);
});

test("the same with another sequence of events", () => {
  runRandomEvents(77777, 600);
});
