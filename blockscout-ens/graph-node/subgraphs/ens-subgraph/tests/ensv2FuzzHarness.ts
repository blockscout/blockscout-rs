// Shared helpers for the randomized projection tests: event shorthands for
// the registry handlers, and checks that recount what the mappings keep
// (ENSv2SubtreeSize, the namespace path lists, canonical flags) from the
// store after each event. Used by ensv2SubtreeCap.test.ts and
// ensv2CanonicalFuzz.test.ts, each of which runs in its own instance: a
// matchstick run never gives memory back, so the long runs are split up.
import {
  Address,
  BigInt,
  ByteArray,
  Bytes,
  crypto,
  ethereum,
  log,
} from "@graphprotocol/graph-ts";
import { assert, dataSourceMock, newMockEvent } from "matchstick-as/assembly/index";
import {
  handleLabelRegistered,
  handleLabelUnregistered,
  handleParentUpdated,
  handleSubregistryUpdated,
} from "../src/ensv2Registry";
import { ParentUpdated } from "../src/types/RootRegistry/PermissionedRegistry";
import { NAMESPACE_UNITS } from "../src/ensv2Paths";
import {
  namespaceId,
  namespacePathIndexId,
  nameSlotId,
  pathNamehash,
  toSlotId,
} from "../src/ensv2Utils";
import {
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
} from "./ensv2TestEvents";

export const ROOT_REGISTRY = "0xB458D6a3a77919449d03e7A6903C26827c1eC43f";
export const ETH_REGISTRY = "0xD4eBcbBdF463C9c45784603Db0dDD499BC44A8B4";
export const R1 = "0xa1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
export const R2 = "0xa2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2";
export const OWNER = "0x89205A3A3b2A69De6Dbf7f01ED13B2108B2c43e7";
export const EMPTY = "0x0000000000000000000000000000000000000000";
export const ROOT_NODE = Bytes.fromHexString(
  "0x0000000000000000000000000000000000000000000000000000000000000000"
);

export function lh(label: string): Bytes {
  return Bytes.fromByteArray(crypto.keccak256(ByteArray.fromUTF8(label)));
}

export function tokenOf(label: string): BigInt {
  let hash = lh(label);
  let le = new Bytes(32);
  for (let i = 0; i < 32; i++) {
    le[i] = hash[31 - i];
  }
  return BigInt.fromUnsignedBytes(le);
}

export function child(parent: Bytes, label: string): Bytes {
  return pathNamehash(parent, lh(label));
}

export function register(registry: string, label: string): void {
  handleLabelRegistered(
    createLabelRegisteredEvent(registry, tokenOf(label), lh(label), label, OWNER)
  );
}

export function unregister(registry: string, label: string): void {
  handleLabelUnregistered(createLabelUnregisteredEvent(registry, tokenOf(label)));
}

export function link(registry: string, label: string, subregistry: string): void {
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(registry, tokenOf(label), subregistry)
  );
}

export function slotId(registry: string, label: string): Bytes {
  return nameSlotId(Address.fromString(registry), toSlotId(tokenOf(label)));
}

export function pathCountOf(registry: string, label: string): i32 {
  return ENSv2NameSlot.load(slotId(registry, label))!.pathCount;
}

// Names counted below a path (by namehash) or a slot (by slot id).
export function below(id: Bytes): i32 {
  let row = ENSv2SubtreeSize.load(id);
  return row == null ? 0 : row.paths;
}

export function ns(registry: string, base: Bytes): string {
  return namespaceId(Address.fromString(registry), base).toHexString();
}

// "eth" in the RootRegistry with the ETHRegistry below it; returns namehash("eth").
export function setUpEth(): Bytes {
  dataSourceMock.setNetwork("sepolia");
  register(ROOT_REGISTRY, "eth");
  link(ROOT_REGISTRY, "eth", ETH_REGISTRY);
  return child(ROOT_NODE, "eth");
}

export const R3 = "0xa3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3";
export const RUN_REGISTRIES = [ETH_REGISTRY, R1, R2, R3];
export const RUN_LABELS = ["a", "b", "c", "d"];

// Every path of the names the random run registers, active or retired,
// through the slots they belong to.
export function collectPaths(): Array<Bytes> {
  let out = new Array<Bytes>();
  for (let r = 0; r < RUN_REGISTRIES.length; r++) {
    for (let l = 0; l < RUN_LABELS.length; l++) {
      let slot = ENSv2NameSlot.load(slotId(RUN_REGISTRIES[r], RUN_LABELS[l]));
      if (slot == null) {
        continue;
      }
      let paths = slot.paths.load();
      for (let i = 0; i < paths.length; i++) {
        out.push(paths[i].id);
      }
    }
  }
  return out;
}

// A namespace lists exactly its active paths (ENSv2NamespacePathIndex,
// positions 0 to pathCount - 1), each at the position the path records.
// Retiring a namespace walks that list, so a path missing from it would stay
// live, and one left in it would make the list grow with the namespace's
// history.
export function assertNamespaceListsMatch(paths: Array<Bytes>, step: i32): void {
  let activeIn = new Map<string, i32>();
  let namespaces = new Array<Bytes>();
  for (let i = 0; i < paths.length; i++) {
    let path = ENSv2NamePath.load(paths[i])!;
    let key = path.namespace.toHexString();
    if (!activeIn.has(key)) {
      activeIn.set(key, 0);
      namespaces.push(path.namespace);
    }
    if (!path.active) {
      assert.i32Equals(-1, path.namespaceIndex);
      continue;
    }
    activeIn.set(key, activeIn.get(key) + 1);
    let namespace = ENSv2Namespace.load(path.namespace)!;
    if (path.namespaceIndex < 0 || path.namespaceIndex >= namespace.pathCount) {
      log.error("step {}: {} is active at position {} of a list of {}", [
        step.toString(),
        path.name === null ? "?" : (path.name as string),
        path.namespaceIndex.toString(),
        namespace.pathCount.toString(),
      ]);
    }
    assert.assertTrue(path.namespaceIndex >= 0);
    assert.assertTrue(path.namespaceIndex < namespace.pathCount);
    let entry = ENSv2NamespacePathIndex.load(
      namespacePathIndexId(namespace.id, path.namespaceIndex)
    )!;
    assert.bytesEquals(path.id, entry.path);
  }
  for (let i = 0; i < namespaces.length; i++) {
    let namespace = ENSv2Namespace.load(namespaces[i])!;
    assert.i32Equals(activeIn.get(namespaces[i].toHexString()), namespace.pathCount);
  }
}

// Recounts the active paths below every path from the parent pointers and
// compares with ENSv2SubtreeSize, per path and per slot. Returns the number
// of active paths, for the caller to check the run did something.
export function assertCountsMatchRecount(eth: Bytes, step: i32, limit: i32): i32 {
  let paths = collectPaths();
  assertNamespaceListsMatch(paths, step);

  let expected = new Map<string, i32>();
  let active = 0;
  for (let i = 0; i < paths.length; i++) {
    let path = ENSv2NamePath.load(paths[i])!;
    if (!path.active) {
      continue;
    }
    active++;
    // A subregistry projected under the path is a unit below the path
    // itself and below everything above it.
    let linkedId = path.activeNamespace;
    if (linkedId && path.parent) {
      let linked = ENSv2Namespace.load(linkedId)!;
      if (linked.active && linked.materialized) {
        let own = path.id.toHexString();
        expected.set(
          own,
          (expected.has(own) ? expected.get(own) : 0) + NAMESPACE_UNITS
        );
      }
    }
    let unitsHere = 1;
    if (linkedId) {
      let linkedHere = ENSv2Namespace.load(linkedId)!;
      if (linkedHere.active && linkedHere.materialized) {
        unitsHere = 1 + NAMESPACE_UNITS;
      }
    }
    let parentId = path.parent;
    while (parentId) {
      let parent = ENSv2NamePath.load(parentId)!;
      // No name stays live below one that was retired.
      if (!parent.active) {
        log.error("step {}: {} is active below the retired {}", [
          step.toString(),
          path.name === null ? "?" : (path.name as string),
          parent.name === null ? "?" : (parent.name as string),
        ]);
      }
      assert.assertTrue(parent.active);
      // RootRegistry names are not counted.
      if (!parent.parent) {
        break;
      }
      let key = parent.id.toHexString();
      expected.set(key, (expected.has(key) ? expected.get(key) : 0) + unitsHere);
      parentId = parent.parent;
    }
  }

  let slotTotals = new Map<string, i32>();
  let slotIds = new Array<Bytes>();
  for (let i = 0; i < paths.length; i++) {
    let path = ENSv2NamePath.load(paths[i])!;
    let key = path.id.toHexString();
    let want = expected.has(key) ? expected.get(key) : 0;
    if (below(path.id) != want) {
      log.error("step {}: path {} counts {} below it, recount says {}", [
        step.toString(),
        path.name === null ? key : (path.name as string),
        below(path.id).toString(),
        want.toString(),
      ]);
    }
    assert.i32Equals(want, below(path.id));
    let slotKey = path.slot.toHexString();
    if (!slotTotals.has(slotKey)) {
      slotIds.push(path.slot);
      slotTotals.set(slotKey, 0);
    }
    slotTotals.set(slotKey, slotTotals.get(slotKey) + want);
  }
  for (let i = 0; i < slotIds.length; i++) {
    // The eth slot itself is exempt and has no row.
    assert.i32Equals(slotTotals.get(slotIds[i].toHexString()), below(slotIds[i]));
    if (below(slotIds[i]) > limit) {
      log.error("step {}: slot {} has {} names below it, limit {}", [
        step.toString(),
        slotIds[i].toHexString(),
        below(slotIds[i]).toString(),
        limit.toString(),
      ]);
    }
    assert.assertTrue(below(slotIds[i]) <= limit);
  }
  return active;
}

export function isRegistered(registry: string, label: string): boolean {
  let slot = ENSv2NameSlot.load(slotId(registry, label));
  return slot != null && slot.status == "REGISTERED";
}

export function declareParent(registry: string, parent: string, label: string): void {
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
  event.parameters.push(
    new ethereum.EventParam("label", ethereum.Value.fromString(label))
  );
  event.parameters.push(
    new ethereum.EventParam("sender", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  handleParentUpdated(event);
}

// A materialised namespace is canonical exactly when its registry declares
// the name it hangs off and that name is itself under a canonical namespace,
// whatever order the links and the declarations came in. And the ETHRegistry
// and RootRegistry are never projected under one of the run's names.
//
// Returns how many canonical namespaces sit two or more names down, i.e.
// are canonical through a canonical namespace above them: the case that
// depends on a declaration higher up being carried down.
export function assertCanonicalFlagsMatch(step: i32): i32 {
  let deepCanonical = 0;
  let paths = collectPaths();
  for (let i = 0; i < paths.length; i++) {
    let path = ENSv2NamePath.load(paths[i])!;
    let linkedId = path.activeNamespace;
    if (!linkedId) {
      continue;
    }
    let linked = ENSv2Namespace.load(linkedId)!;
    if (!linked.active || !linked.materialized) {
      continue;
    }
    let registry = ENSv2Registry.load(linked.registry)!;
    if (registry.kind == "ETH" || registry.kind == "ROOT") {
      log.error("step {}: the {} registry is projected under {}", [
        step.toString(),
        registry.kind,
        path.name === null ? "?" : (path.name as string),
      ]);
    }
    assert.assertTrue(registry.kind != "ETH" && registry.kind != "ROOT");
    let slot = ENSv2NameSlot.load(path.slot)!;
    let expected = false;
    let declaredParent = registry.canonicalParentRegistry;
    let declaredLabel = registry.canonicalParentLabel;
    if (declaredParent && declaredLabel !== null) {
      if (
        declaredParent.equals(slot.registry) &&
        lh(declaredLabel as string).equals(slot.labelhash)
      ) {
        expected = ENSv2Namespace.load(path.namespace)!.canonical;
      }
    }
    if (linked.canonical != expected) {
      log.error("step {}: the namespace under {} has canonical {}, expected {}", [
        step.toString(),
        path.name === null ? "?" : (path.name as string),
        linked.canonical ? "true" : "false",
        expected ? "true" : "false",
      ]);
    }
    assert.booleanEquals(expected, linked.canonical);
    if (linked.canonical && path.depth >= 2) {
      deepCanonical++;
    }
  }
  return deepCanonical;
}

