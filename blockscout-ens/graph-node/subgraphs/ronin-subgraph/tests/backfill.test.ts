import {
  assert,
  beforeEach,
  clearStore,
  describe,
  test,
} from "matchstick-as/assembly/index";

import { runBackfill } from "../src/backfill";
import {
  A,
  ADDR_REVERSE,
  ALICE,
  ALICE_LABEL,
  ALICE_REVERSE,
  AUCTION,
  B,
  C,
  D,
  DUKE,
  E,
  EXPIRY,
  MAX_EXPIRY,
  REGISTRY,
  RESERVED,
  RESOLVER,
  REVERSE,
  REVERSE_REGISTRAR,
  RON,
  ROOT,
  address,
  addressString,
  block,
  mockRecord,
  mockReverseName,
  mockTokenByIndex,
  mockTopLevelRecords,
  mockTotalSupply,
} from "./helpers";

const STATE = "BackfillState";

function mockTokens(tokens: Array<string>): void {
  mockTotalSupply(tokens.length);
  for (let i = 0; i < tokens.length; i++) {
    mockTokenByIndex(i, tokens[i]);
  }
}

describe("backfill", () => {
  beforeEach(() => {
    clearStore();
    mockTopLevelRecords();
  });

  test("loads every token once, whatever order names and reverse nodes come in", () => {
    mockRecord(DUKE, 2, RON, "duke", RESOLVER, ALICE, EXPIRY);
    mockRecord(
      ALICE_REVERSE,
      3,
      ADDR_REVERSE,
      ALICE_LABEL,
      RESOLVER,
      REVERSE_REGISTRAR,
      MAX_EXPIRY
    );
    mockReverseName(ALICE_REVERSE, "duke.ron");
    mockRecord(RESERVED, 2, RON, "99999", 0, AUCTION, MAX_EXPIRY);
    mockTokens([ROOT, RON, REVERSE, ADDR_REVERSE, DUKE, ALICE_REVERSE, RESERVED]);

    runBackfill(block(55577501), address(REGISTRY), 400);

    assert.fieldEquals(STATE, "rns", "done", "true");
    assert.fieldEquals(STATE, "rns", "total", "7");
    assert.fieldEquals(STATE, "rns", "cursor", "-1");
    // RESERVED, ALICE_REVERSE and DUKE; their parents were loaded with them.
    assert.fieldEquals(STATE, "rns", "loaded", "3");
    assert.entityCount("Domain", 7);

    assert.fieldEquals("Domain", DUKE, "name", "duke.ron");
    assert.fieldEquals("Domain", DUKE, "resolvedAddress", addressString(ALICE));
    assert.fieldEquals("Domain", RESERVED, "expiryDate", MAX_EXPIRY.toString());
    assert.fieldEquals("Domain", ADDR_REVERSE, "name", "addr.reverse");
    // The reverse node (index 5) came before the name it points at (index 4).
    assert.fieldEquals(
      "PrimaryNameRecord",
      addressString(ALICE),
      "domain_name",
      "duke.ron"
    );
  });

  test("works across blocks and clamps the cursor when burns shrink the supply", () => {
    mockRecord(D, 2, RON, "d", RESOLVER, ALICE, EXPIRY);
    mockRecord(E, 2, RON, "e", RESOLVER, ALICE, EXPIRY);
    mockTokens([ROOT, RON, A, B, C, D, E]);

    runBackfill(block(55577501), address(REGISTRY), 2);
    assert.fieldEquals(STATE, "rns", "cursor", "4");
    assert.fieldEquals(STATE, "rns", "done", "false");

    // Burning A, C and B swaps the last token into each freed slot
    // (ERC721Enumerable): [ROOT, RON, E, D]. A, B and C have no getRecord mock,
    // so loading any of them would fail the test.
    mockTokens([ROOT, RON, E, D]);

    runBackfill(block(55577502), address(REGISTRY), 2);
    assert.fieldEquals(STATE, "rns", "cursor", "1");

    runBackfill(block(55577503), address(REGISTRY), 2);
    assert.fieldEquals(STATE, "rns", "cursor", "-1");
    assert.fieldEquals(STATE, "rns", "done", "true");
    assert.fieldEquals(STATE, "rns", "loaded", "2");
    assert.notInStore("Domain", A);
    assert.notInStore("Domain", B);
    assert.notInStore("Domain", C);
  });

  test("a finished backfill does nothing", () => {
    mockTokens([ROOT]);
    runBackfill(block(55577501), address(REGISTRY), 2);
    assert.fieldEquals(STATE, "rns", "done", "true");

    runBackfill(block(55577502), address(REGISTRY), 2);
    assert.fieldEquals(STATE, "rns", "loaded", "0");
    assert.entityCount("Domain", 1);
  });
});
