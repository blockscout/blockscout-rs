import { Address, BigInt, Bytes, ethereum } from "@graphprotocol/graph-ts";
import {
  afterEach,
  assert,
  clearStore,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import {
  handleAddressUpdated,
  handleContenthashUpdated,
  handleLinked,
  handleTextUpdated,
} from "../src/ensv2Resolver";
import {
  MAX_LEGACY_FANOUT,
  MAX_LEGACY_SNAPSHOT_KEYS,
  setMaxLegacySnapshotKeysForTesting,
} from "../src/ensv2ResolverProjection";
import { createResolverID } from "../src/resolver";
import {
  AddressUpdated,
  ContenthashUpdated,
  Linked,
  TextUpdated,
} from "../src/types/PermissionedResolver/PermissionedResolver";
import { Account, Domain, Resolver } from "../src/types/schema";

const RESOLVER_ADDRESS = "0xa1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
const OWNER = "0xb2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2";
const RECORD_ID = BigInt.fromI32(1);

function linkedEvent(node: Bytes): Linked {
  let mock = newMockEvent();
  let event = new Linked(
    Address.fromString(RESOLVER_ADDRESS),
    mock.logIndex,
    mock.transactionLogIndex,
    mock.logType,
    mock.block,
    mock.transaction,
    mock.parameters,
    mock.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "recordId",
      ethereum.Value.fromUnsignedBigInt(RECORD_ID)
    )
  );
  event.parameters.push(
    new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(node))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "name",
      ethereum.Value.fromBytes(Bytes.fromHexString("0x00"))
    )
  );
  return event;
}

function contenthashEvent(value: Bytes): ContenthashUpdated {
  let mock = newMockEvent();
  let event = new ContenthashUpdated(
    Address.fromString(RESOLVER_ADDRESS),
    mock.logIndex,
    mock.transactionLogIndex,
    mock.logType,
    mock.block,
    mock.transaction,
    mock.parameters,
    mock.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "recordId",
      ethereum.Value.fromUnsignedBigInt(RECORD_ID)
    )
  );
  event.parameters.push(
    new ethereum.EventParam("hash", ethereum.Value.fromBytes(value))
  );
  return event;
}

function addressEvent(value: Bytes, coinType: i32 = 60): AddressUpdated {
  let mock = newMockEvent();
  let event = new AddressUpdated(
    Address.fromString(RESOLVER_ADDRESS),
    mock.logIndex,
    mock.transactionLogIndex,
    mock.logType,
    mock.block,
    mock.transaction,
    mock.parameters,
    mock.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "recordId",
      ethereum.Value.fromUnsignedBigInt(RECORD_ID)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "coinType",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(coinType))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("addressBytes", ethereum.Value.fromBytes(value))
  );
  return event;
}

function textEvent(key: string = "benchmark", keyHash: i32 = 77): TextUpdated {
  let mock = newMockEvent();
  let event = new TextUpdated(
    Address.fromString(RESOLVER_ADDRESS),
    mock.logIndex,
    mock.transactionLogIndex,
    mock.logType,
    mock.block,
    mock.transaction,
    mock.parameters,
    mock.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "recordId",
      ethereum.Value.fromUnsignedBigInt(RECORD_ID)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "keyHash",
      ethereum.Value.fromFixedBytes(Bytes.fromI32(keyHash))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("key", ethereum.Value.fromString(key))
  );
  event.parameters.push(
    new ethereum.EventParam("value", ethereum.Value.fromString("complete"))
  );
  return event;
}

function seedDomain(node: Bytes, owner: Account): string {
  let resolverAddress = Address.fromString(RESOLVER_ADDRESS);
  let resolverId = createResolverID(node, resolverAddress);
  let domain = new Domain(node.toHexString());
  domain.storedOffchain = false; // Blockscout/BENS extension
  domain.resolvedWithWildcard = false; // Blockscout/BENS extension
  domain.subdomainCount = 0;
  domain.isMigrated = true;
  domain.createdAt = BigInt.zero();
  domain.owner = owner.id;
  domain.resolver = resolverId;
  domain.save();

  let resolver = new Resolver(resolverId);
  resolver.domain = domain.id;
  resolver.address = resolverAddress;
  resolver.save();
  return resolverId;
}

function runFanout(size: i32): void {
  let owner = new Account(OWNER);
  owner.save();
  let resolverIds = new Array<string>();
  for (let i = 0; i < size; i++) {
    let node = Bytes.fromI32(i + 1);
    resolverIds.push(seedDomain(node, owner));
    handleLinked(linkedEvent(node));
  }

  let value = Bytes.fromUTF8("fanout-benchmark");
  handleContenthashUpdated(contenthashEvent(value));
  handleAddressUpdated(addressEvent(Bytes.fromHexString(OWNER)));
  handleTextUpdated(textEvent());

  // The native side is complete whatever the size.
  assert.entityCount("ENSv2ResolverRecordLinkMembership", size);
  assert.entityCount("Resolver", size);
  // One event refreshes the first MAX_LEGACY_FANOUT linked names. The rest
  // keep the snapshot from their own Linked event, taken before the record
  // had anything in it, and the resolver says so.
  assert.fieldEquals(
    "ENSv2Resolver",
    RESOLVER_ADDRESS,
    "legacyProjectionTruncated",
    size > MAX_LEGACY_FANOUT ? "true" : "false"
  );
  for (let i = 0; i < resolverIds.length; i++) {
    let resolver = Resolver.load(resolverIds[i]);
    if (resolver == null) {
      assert.assertTrue(false);
      continue;
    }
    if (i >= MAX_LEGACY_FANOUT) {
      assert.assertTrue(resolver.contentHash === null);
      assert.assertTrue(resolver.addr === null);
      assert.assertTrue(resolver.texts === null);
      assert.assertTrue(resolver.coinTypes === null);
      continue;
    }
    let contentHash = resolver.contentHash;
    let addr = resolver.addr;
    let texts = resolver.texts;
    let coinTypes = resolver.coinTypes;
    if (!contentHash) {
      assert.assertTrue(false);
      continue;
    }
    if (!addr) {
      assert.assertTrue(false);
      continue;
    }
    if (!texts) {
      assert.assertTrue(false);
      continue;
    }
    if (!coinTypes) {
      assert.assertTrue(false);
      continue;
    }
    assert.bytesEquals(value, contentHash!);
    assert.stringEquals(OWNER, addr!);
    assert.assertTrue(texts!.includes("benchmark"));
    let hasEthCoinType = false;
    for (let j = 0; j < coinTypes!.length; j++) {
      if (coinTypes![j].equals(BigInt.fromI32(60))) {
        hasEthCoinType = true;
      }
    }
    assert.assertTrue(hasEthCoinType);
  }
}

afterEach(() => {
  setMaxLegacySnapshotKeysForTesting(MAX_LEGACY_SNAPSHOT_KEYS);
  clearStore();
});

test("fanout benchmark: 1 active explicit membership", () => {
  runFanout(1);
});

test("fanout benchmark: 10 active explicit memberships", () => {
  runFanout(10);
});

test("fanout benchmark: 100 active explicit memberships", () => {
  runFanout(100);
});

test("fanout benchmark: 500 active explicit memberships", () => {
  runFanout(500);
});

test("fanout benchmark: 1000 active explicit memberships", () => {
  runFanout(1000);
});

// A name linked beyond the limit must not be handed the record as it is at
// that moment: no later update would reach it, and it would go on showing an
// address the record no longer has.
test("a name linked to a populated record beyond the limit gets no snapshot, before or after an update", () => {
  let owner = new Account(OWNER);
  owner.save();
  handleAddressUpdated(addressEvent(Bytes.fromHexString(OWNER)));
  let resolverIds = new Array<string>();
  for (let i = 0; i < MAX_LEGACY_FANOUT + 1; i++) {
    let node = Bytes.fromI32(i + 1);
    resolverIds.push(seedDomain(node, owner));
    handleLinked(linkedEvent(node));
  }
  let lastNode = Bytes.fromI32(MAX_LEGACY_FANOUT + 1).toHexString();
  let lastTracked = Bytes.fromI32(MAX_LEGACY_FANOUT).toHexString();
  assert.fieldEquals("Domain", lastTracked, "resolvedAddress", OWNER);
  assert.fieldEquals("Domain", lastNode, "resolvedAddress", "null");
  assert.fieldEquals("Resolver", resolverIds[MAX_LEGACY_FANOUT], "addr", "null");
  assert.fieldEquals("ENSv2Resolver", RESOLVER_ADDRESS, "legacyProjectionTruncated", "true");

  const OTHER = "0xc3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3";
  handleAddressUpdated(addressEvent(Bytes.fromHexString(OTHER)));
  assert.fieldEquals("Domain", lastTracked, "resolvedAddress", OTHER);
  assert.fieldEquals("Domain", lastNode, "resolvedAddress", "null");
  // The native record is complete either way.
  assert.entityCount("ENSv2ResolverRecordLinkMembership", MAX_LEGACY_FANOUT + 1);
});

// The PermissionedResolver source has no address filter, so these events can
// come from any contract, for nodes that are no names at all.
test("links to nodes without a Domain cost no legacy writes and stop at the limit", () => {
  for (let i = 0; i < MAX_LEGACY_FANOUT + 50; i++) {
    handleLinked(linkedEvent(Bytes.fromI32(i + 1)));
  }
  assert.fieldEquals("ENSv2Resolver", RESOLVER_ADDRESS, "legacyProjectionTruncated", "false");

  handleAddressUpdated(addressEvent(Bytes.fromHexString(OWNER)));

  assert.entityCount("ENSv2ResolverRecordLinkMembership", MAX_LEGACY_FANOUT + 50);
  assert.entityCount("Resolver", 0);
  assert.entityCount("Domain", 0);
  // No name was eligible, so the record's snapshot was never built: the
  // address would otherwise have created its Account.
  assert.notInStore("Account", OWNER);
  assert.fieldEquals("ENSv2Resolver", RESOLVER_ADDRESS, "legacyProjectionTruncated", "true");
});

// A record keeps an index row for every coin type it was ever given, and
// any contract can emit AddressUpdated. The snapshot lists a bounded number
// of them; the address itself is read by coin type, so it does not depend on
// where coin type 60 sits in that list.
test("a record with more coin types than its snapshot lists still resolves to its coin type 60 address", () => {
  let owner = new Account(OWNER);
  owner.save();
  let node = Bytes.fromI32(1);
  let resolverId = seedDomain(node, owner);
  handleLinked(linkedEvent(node));
  setMaxLegacySnapshotKeysForTesting(5);
  for (let i = 0; i < 8; i++) {
    handleAddressUpdated(addressEvent(Bytes.fromHexString("0x01"), 1000 + i));
  }
  assert.fieldEquals("Domain", node.toHexString(), "resolvedAddress", "null");

  // Coin type 60 arrives last, past the end of the list.
  handleAddressUpdated(addressEvent(Bytes.fromHexString(OWNER)));
  assert.fieldEquals("Domain", node.toHexString(), "resolvedAddress", OWNER);
  assert.stringEquals(OWNER, Resolver.load(resolverId)!.addr!);
  assert.i32Equals(5, Resolver.load(resolverId)!.coinTypes!.length);
  assert.fieldEquals("ENSv2Resolver", RESOLVER_ADDRESS, "legacyProjectionTruncated", "true");

  // And it follows a change.
  const OTHER = "0xc3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3";
  handleAddressUpdated(addressEvent(Bytes.fromHexString(OTHER)));
  assert.fieldEquals("Domain", node.toHexString(), "resolvedAddress", OTHER);
});

// A text key has no length limit on chain, and the snapshot's key list is
// copied onto the legacy Resolver row of every name that follows the record.
test("a text key too long for the snapshot is left out of it", () => {
  let owner = new Account(OWNER);
  owner.save();
  let node = Bytes.fromI32(1);
  let resolverId = seedDomain(node, owner);
  handleLinked(linkedEvent(node));

  handleTextUpdated(textEvent("avatar", 1));
  handleTextUpdated(textEvent("k".repeat(300), 2));
  handleTextUpdated(textEvent("url", 3));

  let texts = Resolver.load(resolverId)!.texts!;
  assert.i32Equals(2, texts.length);
  assert.assertTrue(texts.includes("avatar"));
  assert.assertTrue(texts.includes("url"));
  assert.fieldEquals("ENSv2Resolver", RESOLVER_ADDRESS, "legacyProjectionTruncated", "true");
  // The native record has all three.
  assert.entityCount("ENSv2ResolverText", 3);
});
