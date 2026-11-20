// v2.3.5: a registry is canonical through the registries above it whatever
// order they declare their parents in (ensv2Paths.ts:
// reconcileCanonicalBelow). Before, a registry that had declared its parent
// before the one above declared its own went on counting as an alias.
// Randomized scenarios, with a recount after every event. In a file of its
// own: a matchstick run never gives memory back.
import { afterEach, assert, clearStore, dataSourceMock, test } from "matchstick-as/assembly/index";
import { log } from "@graphprotocol/graph-ts";
import {
  MAX_BACKFILL_SLOTS_PER_EVENT,
  MAX_NAMES_BELOW_NAME,
  setMaxBackfillSlotsPerEventForTesting,
  setMaxNamesBelowNameForTesting,
} from "../src/ensv2Paths";
import { ENSv2NameSlot } from "../src/types/schema";
import {
  assertCanonicalFlagsMatch,
  assertCountsMatchRecount,
  declareParent,
  EMPTY,
  ETH_REGISTRY,
  isRegistered,
  link,
  R1,
  R2,
  R3,
  register,
  ROOT_REGISTRY,
  RUN_REGISTRIES,
  setUpEth,
  slotId,
} from "./ensv2FuzzHarness";

afterEach(() => {
  setMaxNamesBelowNameForTesting(MAX_NAMES_BELOW_NAME);
  setMaxBackfillSlotsPerEventForTesting(MAX_BACKFILL_SLOTS_PER_EVENT);
  dataSourceMock.resetValues();
  clearStore();
});

// A chain eth -> a.eth -> a.a.eth -> a.a.a.eth, each name pointing at its own
// registry, with other names pointing at the same registries as aliases.
// The four registries (the ETHRegistry included) then declare their parents
// in a random order, which is the late-declaration case when the one at the
// top comes last, and after that declarations are withdrawn, made again or
// pointed elsewhere, and links removed and remade, at random. The flags and
// the counts are recounted after every event.
function runDeclarationOrders(seed: u32, rounds: i32, steps: i32, limit: i32): void {
  let state: u32 = seed;
  let users = [R1, R2, R3];
  let chainParents = [ETH_REGISTRY, R1, R2];
  let spare = ["b", "c", "d"];
  let step = 0;
  let stepsWithChain = 0;
  let fullChains = 0;
  for (let round = 0; round < rounds; round++) {
    clearStore();
    let eth = setUpEth();
    setMaxNamesBelowNameForTesting(limit);
    setMaxBackfillSlotsPerEventForTesting(3);
    for (let i = 0; i < 3; i++) {
      register(chainParents[i], "a");
      link(chainParents[i], "a", users[i]);
    }
    // Aliases and extra names, on labels the chain does not use.
    for (let k = 0; k < 6; k++) {
      state = state * 1664525 + 1013904223;
      let registry = RUN_REGISTRIES[(state >> 8) % 4];
      state = state * 1664525 + 1013904223;
      let label = spare[(state >> 8) % 3];
      register(registry, label);
      state = state * 1664525 + 1013904223;
      if ((state >> 8) % 2 == 0) {
        state = state * 1664525 + 1013904223;
        link(registry, label, users[(state >> 8) % 3]);
      }
    }

    // Each registry declares its true parent, in a random order.
    let order = [0, 1, 2, 3];
    for (let i = 3; i > 0; i--) {
      state = state * 1664525 + 1013904223;
      let j = ((state >> 8) % (i + 1)) as i32;
      let swap = order[i];
      order[i] = order[j];
      order[j] = swap;
    }
    for (let i = 0; i < 4; i++) {
      if (order[i] == 0) {
        declareParent(ETH_REGISTRY, ROOT_REGISTRY, "eth");
      } else {
        declareParent(users[order[i] - 1], chainParents[order[i] - 1], "a");
      }
      assertCountsMatchRecount(eth, step, limit);
      assertCanonicalFlagsMatch(step);
      step++;
    }
    // All declared: the registries two and three names down are canonical,
    // whichever order it happened in (unless the limit kept one out).
    let deep = assertCanonicalFlagsMatch(step);
    if (deep >= 2) {
      fullChains++;
    }

    for (let t = 0; t < steps; t++) {
      state = state * 1664525 + 1013904223;
      let op = (state >> 8) % 10;
      state = state * 1664525 + 1013904223;
      let who = ((state >> 8) % 4) as i32;
      state = state * 1664525 + 1013904223;
      let label = spare[(state >> 8) % 3];
      state = state * 1664525 + 1013904223;
      let registry = RUN_REGISTRIES[(state >> 8) % 4];
      if (op < 3) {
        // Withdraw a declaration.
        declareParent(who == 0 ? ETH_REGISTRY : users[who - 1], EMPTY, "");
      } else if (op < 6) {
        // Make the true one (again).
        if (who == 0) {
          declareParent(ETH_REGISTRY, ROOT_REGISTRY, "eth");
        } else {
          declareParent(users[who - 1], chainParents[who - 1], "a");
        }
      } else if (op < 7) {
        // Declare a name the registry may only be an alias under.
        if (who > 0) {
          declareParent(users[who - 1], registry, label);
        }
      } else if (op < 8) {
        // Take a link of the chain away, or put it back.
        if (who > 0) {
          let slot = ENSv2NameSlot.load(slotId(chainParents[who - 1], "a"));
          if (slot != null && slot.status == "REGISTERED") {
            link(chainParents[who - 1], "a", slot.subregistry ? EMPTY : users[who - 1]);
          }
        }
      } else if (op < 9) {
        register(registry, label);
      } else {
        if (isRegistered(registry, label)) {
          link(registry, label, users[who % 3]);
        }
      }
      assertCountsMatchRecount(eth, step, limit);
      if (assertCanonicalFlagsMatch(step) > 0) {
        stepsWithChain++;
      }
      step++;
    }
  }
  log.info("declaration orders: {} events, {} with a canonical registry two names down, {} of {} rounds fully canonical after the declarations", [
    step.toString(),
    stepsWithChain.toString(),
    fullChains.toString(),
    rounds.toString(),
  ]);
  // The runs spent a good part of their time in the case the check is about.
  assert.assertTrue(stepsWithChain > step / 10);
  assert.assertTrue(fullChains > 0);
}

test("a registry is canonical through the ones above it whatever order they declare in", () => {
  runDeclarationOrders(20261118, 12, 30, MAX_NAMES_BELOW_NAME);
});

test("the same when the limit below a name keeps registries and names out", () => {
  runDeclarationOrders(8675309, 12, 30, 14);
});
