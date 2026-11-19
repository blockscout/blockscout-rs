import { BigInt } from "@graphprotocol/graph-ts";
import {
  assert,
  beforeEach,
  clearStore,
  describe,
  test,
} from "matchstick-as/assembly/index";

import {
  handleNameRegistered,
  handleNameRenewed,
} from "../src/registrarController";
import { handleRecordUpdated, handleTransfer } from "../src/rnsUnified";
import {
  ALICE,
  BOB,
  DUKE,
  EXPIRY,
  EXPIRY_BIT,
  LATER,
  RESOLVER,
  RON,
  addressString,
  mintName,
  mockRecord,
  mockTopLevelRecords,
  nameRegisteredEvent,
  nameRenewedEvent,
  transferEvent,
  updateEvent,
} from "./helpers";

describe("RONRegistrarController", () => {
  beforeEach(() => {
    clearStore();
    mockTopLevelRecords();
  });

  test("NameRegistered records the registration", () => {
    mintName(DUKE, "duke", ALICE);
    handleNameRegistered(nameRegisteredEvent("duke", DUKE, ALICE, 5, EXPIRY));

    assert.fieldEquals("Registration", DUKE, "domain", DUKE);
    assert.fieldEquals("Registration", DUKE, "registrant", addressString(ALICE));
    assert.fieldEquals("Registration", DUKE, "expiryDate", EXPIRY.toString());
    assert.fieldEquals("Registration", DUKE, "cost", "5");
    assert.fieldEquals("Registration", DUKE, "labelName", "duke");
    assert.entityCount("NameRegistered", 1);
  });

  test("a transfer moves the registration and records NameTransferred", () => {
    mintName(DUKE, "duke", ALICE);
    handleNameRegistered(nameRegisteredEvent("duke", DUKE, ALICE, 5, EXPIRY));

    mockRecord(DUKE, 2, RON, "duke", RESOLVER, BOB, EXPIRY);
    handleTransfer(transferEvent(ALICE, BOB, DUKE));

    assert.fieldEquals("Registration", DUKE, "registrant", addressString(BOB));
    assert.entityCount("NameTransferred", 1);
  });

  test("NameRenewed for a name registered before the L2 genesis creates its registration", () => {
    // RNSUnified.renew emits RecordUpdated first, which loads the name from chain.
    mockRecord(DUKE, 2, RON, "duke", RESOLVER, ALICE, LATER);
    handleRecordUpdated(updateEvent(DUKE, EXPIRY_BIT, 0, 0, LATER));
    handleNameRenewed(nameRenewedEvent("duke", DUKE, 7, LATER));

    assert.fieldEquals("Registration", DUKE, "registrant", addressString(ALICE));
    assert.fieldEquals("Registration", DUKE, "expiryDate", LATER.toString());
    assert.fieldEquals("Registration", DUKE, "cost", "7");
    assert.fieldEquals("Registration", DUKE, "labelName", "duke");
    assert.entityCount("NameRenewed", 1);
  });

  test("events for names that are not indexed are ignored", () => {
    handleNameRegistered(nameRegisteredEvent("duke", DUKE, ALICE, 5, EXPIRY));
    handleNameRenewed(nameRenewedEvent("duke", DUKE, 7, BigInt.fromI32(1)));

    assert.notInStore("Registration", DUKE);
    assert.entityCount("NameRegistered", 0);
  });
});
