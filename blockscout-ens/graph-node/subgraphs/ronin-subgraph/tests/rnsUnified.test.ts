import { BigInt } from "@graphprotocol/graph-ts";
import {
  assert,
  beforeEach,
  clearStore,
  describe,
  test,
} from "matchstick-as/assembly/index";

import { runBackfill } from "../src/backfill";
import { handleRecordUpdated, handleTransfer } from "../src/rnsUnified";
import { Domain } from "../src/types/schema";
import {
  ALICE,
  AUCTION,
  BOB,
  DUKE,
  EMPTY,
  EXPIRY,
  EXPIRY_BIT,
  LATER,
  MAX_EXPIRY,
  OTHER_RESOLVER,
  OWNER_BIT,
  REGISTRY,
  RESERVED,
  RESOLVER,
  RON,
  USER_FIELDS,
  address,
  addressString,
  block,
  mintEvent,
  mintName,
  mockRecord,
  mockRecordReverts,
  mockTokenByIndex,
  mockTopLevelRecords,
  mockTotalSupply,
  node,
  resolverId,
  transferEvent,
  updateEvent,
} from "./helpers";

describe("RNSUnified", () => {
  beforeEach(() => {
    clearStore();
    mockTopLevelRecords();
  });

  test("mint creates the name and loads its parent from chain", () => {
    mintName(DUKE, "duke", ALICE);

    assert.fieldEquals("Domain", DUKE, "name", "duke.ron");
    assert.fieldEquals("Domain", DUKE, "labelName", "duke");
    assert.fieldEquals(
      "Domain",
      DUKE,
      "labelhash",
      "0x60b4ccaaa1304900b972ca53a93187c32ce5d990740c56d973102e9b162d6979"
    );
    assert.fieldEquals("Domain", DUKE, "parent", RON);
    assert.fieldEquals("Domain", DUKE, "owner", addressString(ALICE));
    assert.fieldEquals("Domain", DUKE, "registrant", addressString(ALICE));
    assert.fieldEquals("Domain", DUKE, "resolver", resolverId(DUKE));
    assert.fieldEquals("Domain", DUKE, "resolvedAddress", addressString(ALICE));
    assert.fieldEquals("Domain", DUKE, "expiryDate", EXPIRY.toString());
    assert.fieldEquals("Domain", DUKE, "tokenId", node(DUKE).toString());

    // The top-level node keeps its name for its children but no label, so
    // BENS lookups skip it.
    assert.fieldEquals("Domain", RON, "name", "ron");
    assert.fieldEquals("Domain", RON, "subdomainCount", "1");
    assert.assertNull(Domain.load(RON)!.labelName);

    assert.entityCount("NewOwner", 1);
    assert.entityCount("NewResolver", 1);
    // The mint's own ERC-721 Transfer comes before the record and is skipped.
    assert.entityCount("Transfer", 0);
  });

  test("a transfer moves the owner, the resolved address and the registrant", () => {
    mintName(DUKE, "duke", ALICE);

    mockRecord(DUKE, 2, RON, "duke", RESOLVER, BOB, EXPIRY);
    handleTransfer(transferEvent(ALICE, BOB, DUKE));
    handleRecordUpdated(updateEvent(DUKE, OWNER_BIT, 0, BOB, BigInt.zero()));

    assert.fieldEquals("Domain", DUKE, "owner", addressString(BOB));
    assert.fieldEquals("Domain", DUKE, "resolvedAddress", addressString(BOB));
    assert.fieldEquals("Domain", DUKE, "registrant", addressString(BOB));
    assert.entityCount("Transfer", 1);
  });

  test("setRecord's placeholder zero owner is ignored", () => {
    mintName(DUKE, "duke", ALICE);

    // setRecord(id, resolver|owner, {resolver, owner: BOB}) first emits a
    // RecordUpdated whose owner is zero, then transfers.
    handleRecordUpdated(
      updateEvent(DUKE, USER_FIELDS, OTHER_RESOLVER, 0, BigInt.zero())
    );
    assert.fieldEquals("Domain", DUKE, "owner", addressString(ALICE));

    mockRecord(DUKE, 2, RON, "duke", OTHER_RESOLVER, BOB, EXPIRY);
    handleTransfer(transferEvent(ALICE, BOB, DUKE));
    handleRecordUpdated(updateEvent(DUKE, OWNER_BIT, 0, BOB, BigInt.zero()));

    assert.fieldEquals("Domain", DUKE, "owner", addressString(BOB));
    assert.fieldEquals("Domain", DUKE, "resolver", resolverId(DUKE, OTHER_RESOLVER));
    assert.fieldEquals("Domain", DUKE, "resolvedAddress", addressString(BOB));
    assert.fieldEquals("Domain", DUKE, "expiryDate", EXPIRY.toString());
  });

  test("a burn clears the record and a re-mint restores it", () => {
    mintName(DUKE, "duke", ALICE);

    // _burn: Transfer to zero, then RecordUpdated with the user fields zeroed.
    handleTransfer(transferEvent(ALICE, 0, DUKE));
    handleRecordUpdated(updateEvent(DUKE, USER_FIELDS, 0, 0, BigInt.zero()));
    assert.fieldEquals("Domain", DUKE, "owner", EMPTY);
    assert.fieldEquals("Domain", DUKE, "expiryDate", "0");
    let burned = Domain.load(DUKE)!;
    assert.assertNull(burned.resolver);
    assert.assertNull(burned.resolvedAddress);
    assert.assertNull(burned.registrant);

    mockRecord(DUKE, 2, RON, "duke", RESOLVER, BOB, LATER);
    handleTransfer(transferEvent(0, BOB, DUKE));
    handleRecordUpdated(mintEvent(DUKE, 2, RON, "duke", RESOLVER, BOB, LATER));

    assert.fieldEquals("Domain", DUKE, "owner", addressString(BOB));
    assert.fieldEquals("Domain", DUKE, "resolvedAddress", addressString(BOB));
    assert.fieldEquals("Domain", DUKE, "expiryDate", LATER.toString());
    assert.fieldEquals("Domain", RON, "subdomainCount", "1");
  });

  test("reserved names held by the auction resolve to nothing and never expire", () => {
    // mint() stored some expiry; getRecord applies RESERVATION_ROLE.
    mockRecord(RESERVED, 2, RON, "99999", 0, AUCTION, MAX_EXPIRY);
    handleRecordUpdated(
      mintEvent(RESERVED, 2, RON, "99999", 0, AUCTION, BigInt.fromI32(100))
    );

    assert.fieldEquals("Domain", RESERVED, "owner", addressString(AUCTION));
    assert.fieldEquals("Domain", RESERVED, "labelName", "99999");
    assert.fieldEquals("Domain", RESERVED, "expiryDate", MAX_EXPIRY.toString());
    assert.assertNull(Domain.load(RESERVED)!.resolvedAddress);
    assert.entityCount("NewResolver", 0);
  });

  test("an event for a name minted before the L2 genesis loads it from chain", () => {
    mockRecord(DUKE, 2, RON, "duke", RESOLVER, BOB, EXPIRY);
    handleTransfer(transferEvent(ALICE, BOB, DUKE));

    assert.fieldEquals("Domain", DUKE, "name", "duke.ron");
    assert.fieldEquals("Domain", DUKE, "owner", addressString(BOB));
    assert.fieldEquals("Domain", DUKE, "resolvedAddress", addressString(BOB));
    assert.fieldEquals("Domain", RON, "subdomainCount", "1");
    assert.entityCount("Transfer", 1);
  });

  test("a token burned in the block and never loaded is skipped", () => {
    mockRecordReverts(DUKE);
    handleTransfer(transferEvent(ALICE, 0, DUKE));
    handleRecordUpdated(updateEvent(DUKE, USER_FIELDS, 0, 0, BigInt.zero()));

    assert.notInStore("Domain", DUKE);
  });

  test("renewing a name loaded while expired restores its owner", () => {
    // The backfill saw the name expired: getRecord reports owner 0.
    mockTotalSupply(1);
    mockTokenByIndex(0, DUKE);
    mockRecord(DUKE, 2, RON, "duke", RESOLVER, 0, BigInt.fromI32(10));
    runBackfill(block(55577501), address(REGISTRY), 10);
    assert.fieldEquals("Domain", DUKE, "owner", EMPTY);

    mockRecord(DUKE, 2, RON, "duke", RESOLVER, ALICE, LATER);
    handleRecordUpdated(updateEvent(DUKE, EXPIRY_BIT, 0, 0, LATER));

    assert.fieldEquals("Domain", DUKE, "owner", addressString(ALICE));
    assert.fieldEquals("Domain", DUKE, "resolvedAddress", addressString(ALICE));
    assert.fieldEquals("Domain", DUKE, "expiryDate", LATER.toString());
    assert.entityCount("ExpiryExtended", 1);
  });
});
