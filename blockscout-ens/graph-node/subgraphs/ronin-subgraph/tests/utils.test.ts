import { Address, BigInt } from "@graphprotocol/graph-ts";
import { assert, describe, test } from "matchstick-as/assembly/index";

import {
  ADDR_REVERSE_NODE,
  addressOfReverseName,
  EMPTY_ADDRESS,
  EXPIRY_FIELD,
  hasField,
  IMMUTABLE_FIELDS,
  namehash,
  nodeId,
  RESOLVER_FIELD,
  REVERSE_NODE,
  reverseNodeOf,
} from "../src/utils";
import {
  ADDR_REVERSE,
  ALICE,
  ALICE_LABEL,
  ALICE_REVERSE,
  ALL_FIELDS,
  addressString,
  DUKE,
  EXPIRY_BIT,
  node,
  RESERVED,
  REVERSE,
  RON,
  USER_FIELDS,
} from "./helpers";

describe("hashing", () => {
  test("namehash gives the ids RNS uses on chain", () => {
    // LibRNSDomain.RON_ID and ADDR_REVERSE_ID
    assert.stringEquals(RON, namehash("ron"));
    assert.stringEquals(ADDR_REVERSE, namehash("addr.reverse"));
    assert.stringEquals(REVERSE, namehash("reverse"));
    assert.stringEquals(REVERSE_NODE, REVERSE);
    assert.stringEquals(ADDR_REVERSE_NODE, ADDR_REVERSE);
    assert.stringEquals(DUKE, namehash("duke.ron"));
    // RNSUnified.tokenByIndex(5) and an auction-held token on Ronin mainnet,
    // read on 2026-11-19.
    assert.stringEquals(
      "0x4db7d1212a03307ec13676e6b1f8fdde5364c44d3b99dc031a3ae0d312c44d17",
      namehash("rnstesting.ron")
    );
    assert.stringEquals(RESERVED, namehash("99999.ron"));
  });

  test("token ids become 32-byte hex domain ids", () => {
    assert.stringEquals("0x" + "0".repeat(63) + "1", nodeId(BigInt.fromI32(1)));
    assert.stringEquals("0x" + "0".repeat(64), nodeId(BigInt.zero()));
    assert.stringEquals(RON, nodeId(node(RON)));
    assert.stringEquals(DUKE, nodeId(node(DUKE)));
  });

  test("reverse nodes use the lowercase address without 0x", () => {
    assert.stringEquals(ALICE_REVERSE, reverseNodeOf(addressString(ALICE)));
    assert.stringEquals(
      addressString(ALICE),
      addressOfReverseName(ALICE_LABEL + ".addr.reverse")!
    );
    assert.assertNull(addressOfReverseName("duke.ron"));
    assert.assertNull(addressOfReverseName("abc.addr.reverse"));
    assert.assertNull(addressOfReverseName(null));
  });

  test("the zero address constant", () => {
    assert.stringEquals(Address.zero().toHexString(), EMPTY_ADDRESS);
  });
});

describe("ModifyingIndicator", () => {
  test("only mint() sets the immutable fields", () => {
    assert.assertTrue(hasField(ALL_FIELDS, IMMUTABLE_FIELDS));
    assert.assertTrue(!hasField(USER_FIELDS, IMMUTABLE_FIELDS));
    assert.assertTrue(hasField(USER_FIELDS, RESOLVER_FIELD));
    assert.assertTrue(!hasField(USER_FIELDS, EXPIRY_FIELD));
    assert.assertTrue(hasField(EXPIRY_BIT, EXPIRY_FIELD));
  });
});
