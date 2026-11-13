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

function addressEvent(value: Bytes): AddressUpdated {
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
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(60))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("addressBytes", ethereum.Value.fromBytes(value))
  );
  return event;
}

function textEvent(): TextUpdated {
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
      ethereum.Value.fromFixedBytes(Bytes.fromI32(77))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("key", ethereum.Value.fromString("benchmark"))
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

  assert.entityCount("ENSv2ResolverRecordLinkMembership", size);
  assert.entityCount("Resolver", size);
  for (let i = 0; i < resolverIds.length; i++) {
    let resolver = Resolver.load(resolverIds[i]);
    if (resolver == null) {
      assert.assertTrue(false);
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
