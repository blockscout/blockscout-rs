// v2.3.5: whether a namespace is canonical, and what is counted below a
// name, hold whatever order links and parent declarations arrive in, and the
// ETHRegistry and RootRegistry are never projected under a name that merely
// points at them (ensv2Paths.ts: reconcileCanonicalBelow,
// isOwnPlaceOfCoreRegistry). Randomized, with a recount after every event.
import { afterEach, assert, clearStore, dataSourceMock, test } from "matchstick-as/assembly/index";
import { Address, log } from "@graphprotocol/graph-ts";
import {
  MAX_BACKFILL_SLOTS_PER_EVENT,
  MAX_NAMES_BELOW_NAME,
  setMaxBackfillSlotsPerEventForTesting,
  setMaxNamesBelowNameForTesting,
} from "../src/ensv2Paths";
import { ENSv2NamePath, ENSv2Namespace, ENSv2NameSlot } from "../src/types/schema";
import {
  assertCanonicalFlagsMatch,
  assertCountsMatchRecount,
  collectPaths,
  declareParent,
  EMPTY,
  ETH_REGISTRY,
  isRegistered,
  link,
  register,
  R1,
  R2,
  R3,
  ROOT_REGISTRY,
  RUN_LABELS,
  RUN_REGISTRIES,
  setUpEth,
  slotId,
  unregister,
} from "./ensv2FuzzHarness";

afterEach(() => {
  setMaxNamesBelowNameForTesting(MAX_NAMES_BELOW_NAME);
  setMaxBackfillSlotsPerEventForTesting(MAX_BACKFILL_SLOTS_PER_EVENT);
  dataSourceMock.resetValues();
  clearStore();
});

// The same run with registries declaring and withdrawing parents, and names
// pointed at the ETHRegistry and the RootRegistry.
function runRandomEventsWithDeclarations(seed: u32, steps: i32, limit: i32): void {
  let eth = setUpEth();
  declareParent(ETH_REGISTRY, ROOT_REGISTRY, "eth");
  setMaxNamesBelowNameForTesting(limit);
  setMaxBackfillSlotsPerEventForTesting(3);
  let registries = RUN_REGISTRIES;
  let labels = RUN_LABELS;
  let state: u32 = seed;
  let mostActive = 0;
  for (let step = 0; step < steps; step++) {
    state = state * 1664525 + 1013904223;
    let op = (state >> 8) % 16;
    state = state * 1664525 + 1013904223;
    let registry = registries[(state >> 8) % 4];
    state = state * 1664525 + 1013904223;
    let label = labels[(state >> 8) % 4];
    state = state * 1664525 + 1013904223;
    let target = registries[1 + ((state >> 8) % 3)];
    state = state * 1664525 + 1013904223;
    let other = registries[(state >> 8) % 4];
    state = state * 1664525 + 1013904223;
    let otherLabel = labels[(state >> 8) % 4];
    state = state * 1664525 + 1013904223;
    let coin = (state >> 8) % 8;

    if (op < 4) {
      register(registry, label);
    } else if (op < 5) {
      if (isRegistered(registry, label)) {
        unregister(registry, label);
      }
    } else if (op < 8) {
      if (isRegistered(registry, label)) {
        link(registry, label, target);
      }
    } else if (op < 9) {
      if (isRegistered(registry, label)) {
        link(registry, label, EMPTY);
      }
    } else if (op < 10) {
      // A name pointed at the ETHRegistry or at the RootRegistry.
      if (isRegistered(registry, label)) {
        link(registry, label, coin % 2 == 0 ? ETH_REGISTRY : ROOT_REGISTRY);
      }
    } else if (op < 14) {
      // The registry a name points at declares that name as its parent, as
      // a registry's owner would: often after the link, sometimes after the
      // registries below it have declared theirs. One time in four the name
      // is taken from a user registry, to build chains.
      let from = op < 13 ? registry : target;
      let slot = ENSv2NameSlot.load(slotId(from, label));
      if (slot != null) {
        let linked = slot.subregistry;
        if (linked) {
          declareParent(Address.fromBytes(linked).toHexString(), from, label);
        }
      }
    } else if (op < 15) {
      // A registry declares a name it may not be linked under.
      declareParent(target, other, otherLabel);
    } else if (coin < 2) {
      declareParent(target, EMPTY, "");
    } else if (coin < 3) {
      // The ETHRegistry withdraws eth, and everything below stops being
      // canonical; then declares it again.
      declareParent(ETH_REGISTRY, EMPTY, "");
    } else if (coin < 5) {
      declareParent(ETH_REGISTRY, ROOT_REGISTRY, "eth");
    }
    let active = assertCountsMatchRecount(eth, step, limit);
    assertCanonicalFlagsMatch(step);
    if (active > mostActive) {
      mostActive = active;
    }
  }
  // Uniformly random declarations seldom line up into a canonical chain;
  // ensv2CanonicalOrder.test.ts builds those on purpose. This run is for
  // everything else declarations and core-registry links can do.
  assert.assertTrue(mostActive > 10);
}

test("canonical flags and counts hold through 400 random events with parent declarations", () => {
  runRandomEventsWithDeclarations(424242, 400, MAX_NAMES_BELOW_NAME);
});

test("the same with another sequence and a limit that keeps refusing", () => {
  runRandomEventsWithDeclarations(1357911, 400, 40);
});
