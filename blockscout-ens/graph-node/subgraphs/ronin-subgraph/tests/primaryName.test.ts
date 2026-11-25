import {
  assert,
  beforeEach,
  clearStore,
  describe,
  test,
} from "matchstick-as/assembly/index";

import { handleNameChanged, handleVersionChanged } from "../src/resolver";
import { handleTransfer } from "../src/rnsUnified";
import { Domain } from "../src/types/schema";
import {
  ADDR_REVERSE,
  ALICE,
  ALICE_LABEL,
  ALICE_REVERSE,
  BOB,
  DUKE,
  EXPIRY,
  MAX_EXPIRY,
  OTHER,
  RESOLVER,
  REVERSE_REGISTRAR,
  RON,
  addressString,
  claimReverse,
  mintName,
  mockGetRNSUnified,
  mockRecord,
  mockReverseName,
  mockTopLevelRecords,
  nameChangedEvent,
  transferEvent,
  versionChangedEvent,
} from "./helpers";

const RECORD = "PrimaryNameRecord";

describe("primary names", () => {
  beforeEach(() => {
    clearStore();
    mockTopLevelRecords();
  });

  test("a reverse name the address owns becomes its primary name", () => {
    mintName(DUKE, "duke", ALICE);
    claimReverse(ALICE_REVERSE, ALICE_LABEL);
    handleNameChanged(nameChangedEvent(ALICE_REVERSE, "duke.ron"));

    let alice = addressString(ALICE);
    assert.fieldEquals(RECORD, alice, "resolved_address", alice);
    assert.fieldEquals(RECORD, alice, "domain_id", DUKE);
    assert.fieldEquals(RECORD, alice, "domain_name", "duke.ron");

    // The reverse node itself stays out of BENS lookups and resolves to nothing.
    assert.fieldEquals("Domain", ALICE_REVERSE, "name", ALICE_LABEL + ".addr.reverse");
    assert.fieldEquals("Domain", ALICE_REVERSE, "owner", addressString(REVERSE_REGISTRAR));
    let reverse = Domain.load(ALICE_REVERSE)!;
    assert.assertNull(reverse.labelName);
    assert.assertNull(reverse.resolvedAddress);
  });

  test("the primary name follows ownership of the named domain", () => {
    mintName(DUKE, "duke", ALICE);
    claimReverse(ALICE_REVERSE, ALICE_LABEL);
    handleNameChanged(nameChangedEvent(ALICE_REVERSE, "duke.ron"));

    mockRecord(DUKE, 2, RON, "duke", RESOLVER, BOB, EXPIRY);
    handleTransfer(transferEvent(ALICE, BOB, DUKE));
    assert.notInStore(RECORD, addressString(ALICE));

    mockRecord(DUKE, 2, RON, "duke", RESOLVER, ALICE, EXPIRY);
    handleTransfer(transferEvent(BOB, ALICE, DUKE));
    assert.fieldEquals(RECORD, addressString(ALICE), "domain_name", "duke.ron");
  });

  test("a reverse name for someone else's domain is not recorded", () => {
    mintName(OTHER, "other", BOB);
    claimReverse(ALICE_REVERSE, ALICE_LABEL);
    handleNameChanged(nameChangedEvent(ALICE_REVERSE, "other.ron"));

    assert.notInStore(RECORD, addressString(ALICE));
  });

  test("a reverse name set before the domain exists counts once it is minted", () => {
    claimReverse(ALICE_REVERSE, ALICE_LABEL);
    handleNameChanged(nameChangedEvent(ALICE_REVERSE, "duke.ron"));
    assert.notInStore(RECORD, addressString(ALICE));

    mintName(DUKE, "duke", ALICE);
    assert.fieldEquals(RECORD, addressString(ALICE), "domain_id", DUKE);
  });

  test("clearRecords on the reverse node drops the primary name", () => {
    mintName(DUKE, "duke", ALICE);
    claimReverse(ALICE_REVERSE, ALICE_LABEL);
    handleNameChanged(nameChangedEvent(ALICE_REVERSE, "duke.ron"));

    // On the forward name it clears records, but addr() is ownerOf() in RNS.
    handleVersionChanged(versionChangedEvent(DUKE, 1));
    assert.fieldEquals("Domain", DUKE, "resolvedAddress", addressString(ALICE));
    assert.fieldEquals(RECORD, addressString(ALICE), "domain_name", "duke.ron");

    handleVersionChanged(versionChangedEvent(ALICE_REVERSE, 1));
    assert.notInStore(RECORD, addressString(ALICE));
  });

  test("a reverse node claimed before the L2 genesis is loaded by its NameChanged", () => {
    mintName(DUKE, "duke", ALICE);
    mockGetRNSUnified();
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

    handleNameChanged(nameChangedEvent(ALICE_REVERSE, "duke.ron"));

    assert.fieldEquals("Domain", ALICE_REVERSE, "parent", ADDR_REVERSE);
    assert.fieldEquals(RECORD, addressString(ALICE), "domain_name", "duke.ron");
  });

  test("a name carrying a NUL byte is ignored", () => {
    mintName(DUKE, "duke", ALICE);
    claimReverse(ALICE_REVERSE, ALICE_LABEL);
    handleNameChanged(nameChangedEvent(ALICE_REVERSE, "duke.ron\u0000"));

    assert.entityCount("NameChanged", 0);
    assert.notInStore(RECORD, addressString(ALICE));
  });
});
