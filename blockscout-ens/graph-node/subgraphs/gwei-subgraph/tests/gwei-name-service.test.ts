import { Address, BigInt } from "@graphprotocol/graph-ts";
import {
  afterEach,
  assert,
  clearStore,
  describe,
  test,
} from "matchstick-as/assembly/index";

import { Domain, PrimaryNameRecord } from "../generated/schema";
import {
  handleAddrChanged,
  handleNameRegistered,
  handlePrimaryNameSet,
  handleSubdomainRegistered,
  handleTransfer,
} from "../src/gwei-name-service";
import { GWEI_NODE, tokenIdToDomainID } from "../src/utils";
import {
  addrChangedEvent,
  nameRegisteredEvent,
  primaryNameSetEvent,
  subdomainRegisteredEvent,
  transferEvent,
} from "./gwei-name-service-utils";

const OWNER = "0x0000000000000000000000000000000000000001";
const RECIPIENT = "0x0000000000000000000000000000000000000002";
const EXPLICIT = "0x0000000000000000000000000000000000000003";

describe("Gwei Name Service mapping", () => {
  afterEach(() => {
    clearStore();
  });

  test("indexes a registration and invalidates an owner-fallback primary name after transfer", () => {
    let tokenId = BigInt.fromI32(1);
    let domainID = tokenIdToDomainID(tokenId);
    handleNameRegistered(
      nameRegisteredEvent(
        tokenId,
        "alice",
        Address.fromString(OWNER),
        BigInt.fromI32(1000)
      )
    );

    assert.fieldEquals("Domain", domainID, "name", "alice.gwei");
    assert.fieldEquals("Domain", domainID, "parent", GWEI_NODE);
    assert.fieldEquals("Domain", domainID, "owner", OWNER);
    assert.fieldEquals("Domain", domainID, "resolvedAddress", OWNER);
    assert.fieldEquals("Domain", domainID, "expiryDate", "1000");

    handlePrimaryNameSet(
      primaryNameSetEvent(Address.fromString(OWNER), tokenId)
    );
    assert.fieldEquals(
      "PrimaryNameRecord",
      OWNER,
      "domain_name",
      "alice.gwei"
    );

    handleTransfer(
      transferEvent(
        Address.fromString(OWNER),
        Address.fromString(RECIPIENT),
        tokenId
      )
    );
    assert.fieldEquals("Domain", domainID, "owner", RECIPIENT);
    assert.fieldEquals("Domain", domainID, "resolvedAddress", RECIPIENT);
    let oldPrimary = PrimaryNameRecord.load(OWNER)!;
    assert.assertNull(oldPrimary.domain_name);
  });

  test("keeps an explicit resolved address across ERC-721 transfers", () => {
    let tokenId = BigInt.fromI32(2);
    let domainID = tokenIdToDomainID(tokenId);
    handleNameRegistered(
      nameRegisteredEvent(
        tokenId,
        "explicit",
        Address.fromString(OWNER),
        BigInt.fromI32(1000)
      )
    );
    handleAddrChanged(
      addrChangedEvent(tokenId, Address.fromString(EXPLICIT))
    );
    handlePrimaryNameSet(
      primaryNameSetEvent(Address.fromString(EXPLICIT), tokenId)
    );
    handleTransfer(
      transferEvent(
        Address.fromString(OWNER),
        Address.fromString(RECIPIENT),
        tokenId
      )
    );

    assert.fieldEquals("Domain", domainID, "owner", RECIPIENT);
    assert.fieldEquals("Domain", domainID, "resolvedAddress", EXPLICIT);
    assert.fieldEquals(
      "PrimaryNameRecord",
      EXPLICIT,
      "domain_name",
      "explicit.gwei"
    );
  });

  test("constructs subdomain names and snapshots inherited expiry", () => {
    let parentId = BigInt.fromI32(3);
    let childId = BigInt.fromI32(4);
    let parentDomainID = tokenIdToDomainID(parentId);
    let childDomainID = tokenIdToDomainID(childId);
    handleNameRegistered(
      nameRegisteredEvent(
        parentId,
        "alice",
        Address.fromString(OWNER),
        BigInt.fromI32(1000)
      )
    );
    handleNameRegistered(
      nameRegisteredEvent(
        childId,
        "pay",
        Address.fromString(OWNER),
        BigInt.fromI32(0)
      )
    );
    handleSubdomainRegistered(
      subdomainRegisteredEvent(childId, parentId, "pay")
    );

    assert.fieldEquals("Domain", childDomainID, "name", "pay.alice.gwei");
    assert.fieldEquals("Domain", childDomainID, "parent", parentDomainID);
    assert.fieldEquals("Domain", childDomainID, "expiryDate", "1000");
    assert.fieldEquals("Domain", parentDomainID, "subdomainCount", "1");
    let child = Domain.load(childDomainID)!;
    assert.assertNotNull(child.resolver);
  });
});
