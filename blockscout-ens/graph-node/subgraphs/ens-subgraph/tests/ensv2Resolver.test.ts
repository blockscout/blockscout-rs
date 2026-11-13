import { Address, BigInt, Bytes, crypto, ethereum } from "@graphprotocol/graph-ts";
import {
  afterEach,
  assert,
  clearStore,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import {
  handleABIUpdated,
  handleAddressUpdated,
  handleContenthashUpdated,
  handleInterfaceUpdated,
  handleLinked,
  handleNameUpdated,
  handleRecordDataUpdated,
  handleResourceArgument,
  handleResolverCreated,
  handleTextUpdated,
} from "../src/ensv2Resolver";
import { createResolverID, handleAddrChanged } from "../src/resolver";
import { handleNewResolver } from "../src/ensRegistry";
import { NewResolver } from "../src/types/ENSRegistry/EnsRegistry";
import { decodeName } from "../src/nameWrapper";
import { resolverLinkId } from "../src/ensv2Utils";
import {
  ABIUpdated,
  AddressUpdated,
  ContenthashUpdated,
  DataUpdated,
  InterfaceUpdated,
  Linked,
  NameUpdated,
  ResolverCreated,
  ResourceArgument,
  TextUpdated,
} from "../src/types/PermissionedResolver/PermissionedResolver";
import { AddrChanged } from "../src/types/Resolver/Resolver";
import {
  Account,
  Domain,
  ENSv2Resolver,
  ENSv2ResolverRecord,
  Resolver,
  ENSv2ResolverLink,
} from "../src/types/schema";

const PERMISSIONED_RESOLVER = "0x11111111111111111111111111111111111111aa";
const DOMAIN_OWNER = "0x22222222222222222222222222222222222222bb";

// DNS-wire-format encode a single-label name, e.g. "alice" ->
// 0x05616c696365 00 (length-prefixed label + zero-length root terminator).
function encodeLabel(label: string): Bytes {
  let labelBytes = Bytes.fromUTF8(label);
  let out = new Uint8Array(labelBytes.length + 2);
  out[0] = labelBytes.length as u8;
  for (let i = 0; i < labelBytes.length; i++) {
    out[i + 1] = labelBytes[i];
  }
  out[labelBytes.length + 1] = 0;
  return Bytes.fromUint8Array(out);
}

function seedProjectedDomain(node: Bytes): string {
  let owner = new Account(DOMAIN_OWNER);
  owner.save();

  let domain = new Domain(node.toHexString());
  domain.storedOffchain = false; // Blockscout/BENS extension
  domain.resolvedWithWildcard = false; // Blockscout/BENS extension
  domain.subdomainCount = 0;
  domain.isMigrated = true;
  domain.createdAt = BigInt.zero();
  domain.owner = owner.id;

  let resolverAddress = Address.fromString(PERMISSIONED_RESOLVER);
  let resolverId = createResolverID(node, resolverAddress);
  let resolver = new Resolver(resolverId);
  resolver.domain = domain.id;
  resolver.address = resolverAddress;
  resolver.save();

  domain.resolver = resolver.id;
  domain.save();
  return resolverId;
}

// --- RecordId-keyed event model mock-event builders ---

const createResolverCreatedEvent = (): ResolverCreated => {
  let mockEvent = newMockEvent();
  let event = new ResolverCreated(
    Address.fromString(PERMISSIONED_RESOLVER),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  return event;
};

const createResourceArgumentEvent = (
  resource: BigInt,
  arg: Bytes
): ResourceArgument => {
  let mockEvent = newMockEvent();
  let event = new ResourceArgument(
    Address.fromString(PERMISSIONED_RESOLVER),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "resource",
      ethereum.Value.fromUnsignedBigInt(resource)
    )
  );
  event.parameters.push(
    new ethereum.EventParam("arg", ethereum.Value.fromBytes(arg))
  );
  return event;
};

const createLinkedEvent = (
  recordId: BigInt,
  node: Bytes,
  name: Bytes
): Linked => {
  let mockEvent = newMockEvent();
  let event = new Linked(
    Address.fromString(PERMISSIONED_RESOLVER),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "recordId",
      ethereum.Value.fromUnsignedBigInt(recordId)
    )
  );
  event.parameters.push(
    new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(node))
  );
  event.parameters.push(
    new ethereum.EventParam("name", ethereum.Value.fromBytes(name))
  );
  return event;
};

const createContenthashUpdatedEvent = (
  recordId: BigInt,
  hash: Bytes
): ContenthashUpdated => {
  let mockEvent = newMockEvent();
  let event = new ContenthashUpdated(
    Address.fromString(PERMISSIONED_RESOLVER),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "recordId",
      ethereum.Value.fromUnsignedBigInt(recordId)
    )
  );
  event.parameters.push(
    new ethereum.EventParam("hash", ethereum.Value.fromBytes(hash))
  );
  return event;
};

const createNameUpdatedEvent = (
  recordId: BigInt,
  primaryName: string
): NameUpdated => {
  let mockEvent = newMockEvent();
  let event = new NameUpdated(
    Address.fromString(PERMISSIONED_RESOLVER),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "recordId",
      ethereum.Value.fromUnsignedBigInt(recordId)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "primaryName",
      ethereum.Value.fromString(primaryName)
    )
  );
  return event;
};

const createAddressUpdatedEvent = (
  recordId: BigInt,
  coinType: BigInt,
  addressBytes: Bytes
): AddressUpdated => {
  let mockEvent = newMockEvent();
  let event = new AddressUpdated(
    Address.fromString(PERMISSIONED_RESOLVER),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "recordId",
      ethereum.Value.fromUnsignedBigInt(recordId)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "coinType",
      ethereum.Value.fromUnsignedBigInt(coinType)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "addressBytes",
      ethereum.Value.fromBytes(addressBytes)
    )
  );
  return event;
};

const createTextUpdatedEvent = (
  recordId: BigInt,
  keyHash: Bytes,
  key: string,
  value: string
): TextUpdated => {
  let mockEvent = newMockEvent();
  let event = new TextUpdated(
    Address.fromString(PERMISSIONED_RESOLVER),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "recordId",
      ethereum.Value.fromUnsignedBigInt(recordId)
    )
  );
  event.parameters.push(
    new ethereum.EventParam("keyHash", ethereum.Value.fromFixedBytes(keyHash))
  );
  event.parameters.push(
    new ethereum.EventParam("key", ethereum.Value.fromString(key))
  );
  event.parameters.push(
    new ethereum.EventParam("value", ethereum.Value.fromString(value))
  );
  return event;
};

const createDataUpdatedEvent = (
  recordId: BigInt,
  keyHash: Bytes,
  key: string,
  value: Bytes
): DataUpdated => {
  let mockEvent = newMockEvent();
  let event = new DataUpdated(
    Address.fromString(PERMISSIONED_RESOLVER),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "recordId",
      ethereum.Value.fromUnsignedBigInt(recordId)
    )
  );
  event.parameters.push(
    new ethereum.EventParam("keyHash", ethereum.Value.fromFixedBytes(keyHash))
  );
  event.parameters.push(
    new ethereum.EventParam("key", ethereum.Value.fromString(key))
  );
  event.parameters.push(
    new ethereum.EventParam("value", ethereum.Value.fromBytes(value))
  );
  return event;
};

const createABIUpdatedEvent = (
  recordId: BigInt,
  contentType: BigInt
): ABIUpdated => {
  let mockEvent = newMockEvent();
  let event = new ABIUpdated(
    Address.fromString(PERMISSIONED_RESOLVER),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "recordId",
      ethereum.Value.fromUnsignedBigInt(recordId)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "contentType",
      ethereum.Value.fromUnsignedBigInt(contentType)
    )
  );
  return event;
};

const createInterfaceUpdatedEvent = (
  recordId: BigInt,
  interfaceId: Bytes,
  implementer: Address
): InterfaceUpdated => {
  let mockEvent = newMockEvent();
  let event = new InterfaceUpdated(
    Address.fromString(PERMISSIONED_RESOLVER),
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "recordId",
      ethereum.Value.fromUnsignedBigInt(recordId)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "interfaceId",
      ethereum.Value.fromFixedBytes(interfaceId)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "implementer",
      ethereum.Value.fromAddress(implementer)
    )
  );
  return event;
};

afterEach(() => {
  clearStore();
});

// assert.fieldEquals compares an entity's id as its lowercase-hex string
// form regardless of the underlying GraphQL type.
// Production code now builds these ids as fixed-width Bytes concatenation
// with no delimiter (a BigInt component is a 32-byte big-endian value,
// src/utils.ts::uint256ToByteArray) — these mirror that exact encoding, and
// hexOf mirrors Bytes.fromUTF8(kindTag).toHexString() for the fixed 4-byte
// "NAME"/"TEXT"/"DATA"/"ADDR" kind tags.
function bigIntHex32(i: BigInt): string {
  return i.toHex().slice(2).padStart(64, "0");
}
function hexOf(b: Bytes): string {
  return b.toHexString().slice(2);
}

test("a standard ENSIP event fired from a PermissionedResolver-style address is still processed by the existing addressless Resolver source", () => {
  let node = Bytes.fromHexString(
    "0x7857c9824139b8a8c3cb04712b41558b4878c55fa9c1e5390e910ee3220c3cce"
  );
  let permissionedResolverAddress = Address.fromString(PERMISSIONED_RESOLVER);

  let mockEvent = newMockEvent();
  let event = new AddrChanged(
    permissionedResolverAddress,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(node))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "a",
      ethereum.Value.fromAddress(
        Address.fromString("0x8e8db5ccef88cca9d624701db544989c996e3211")
      )
    )
  );

  let resolverId = createResolverID(node, permissionedResolverAddress);
  assert.assertNull(Resolver.load(resolverId));

  handleAddrChanged(event);

  assert.assertNotNull(Resolver.load(resolverId));
});

// --- RecordId-keyed PermissionedResolver event model ---

test("ResolverCreated creates only the ENSv2Resolver row, nothing else", () => {
  handleResolverCreated(createResolverCreatedEvent());

  let resolverId = Address.fromString(PERMISSIONED_RESOLVER).toHexString();
  assert.assertNotNull(ENSv2Resolver.load(Bytes.fromHexString(resolverId)));
  assert.entityCount("ENSv2ResolverRecord", 0);
  assert.entityCount("ENSv2ResolverLink", 0);
});

test("Linked with a non-zero recordId sets record/active; re-linking to a different recordId overwrites the same row; linking to recordId 0 clears record and active", () => {
  let node = Bytes.fromI32(101);
  let name = encodeLabel("ivan");
  let resolverId = Address.fromString(PERMISSIONED_RESOLVER).toHexString();
  let linkId = resolverId.concat(hexOf(node));
  let recordId1 = BigInt.fromI32(1);
  let recordId2 = BigInt.fromI32(2);
  let record1Id = resolverId.concat(bigIntHex32(recordId1));
  let record2Id = resolverId.concat(bigIntHex32(recordId2));

  handleLinked(createLinkedEvent(recordId1, node, name));
  assert.fieldEquals("ENSv2ResolverLink", linkId, "record", record1Id);
  assert.fieldEquals("ENSv2ResolverLink", linkId, "recordId", "1");
  assert.fieldEquals("ENSv2ResolverLink", linkId, "active", "true");
  assert.fieldEquals("ENSv2ResolverLink", linkId, "nameDecoded", "ivan");
  assert.assertNotNull(ENSv2ResolverRecord.load(Bytes.fromHexString(record1Id)));

  // Re-link the same node to a different recordId -- same row id, fields overwritten.
  handleLinked(createLinkedEvent(recordId2, node, name));
  assert.fieldEquals("ENSv2ResolverLink", linkId, "record", record2Id);
  assert.fieldEquals("ENSv2ResolverLink", linkId, "recordId", "2");
  assert.entityCount("ENSv2ResolverLink", 1);

  // Explicit unlink (recordId 0) -- record relation cleared, active false,
  // no bogus ENSv2ResolverRecord created for recordId 0.
  let recordId0 = BigInt.zero();
  let record0Id = resolverId.concat(bigIntHex32(recordId0));
  handleLinked(createLinkedEvent(recordId0, node, name));
  assert.fieldEquals("ENSv2ResolverLink", linkId, "recordId", "0");
  assert.fieldEquals("ENSv2ResolverLink", linkId, "active", "false");
  assert.assertTrue(ENSv2ResolverRecord.load(Bytes.fromHexString(record0Id)) == null);
  // Only the two real records (recordId1, recordId2) exist -- no third for 0.
  assert.entityCount("ENSv2ResolverRecord", 2);
});

test("ContenthashUpdated and NameUpdated write directly onto ENSv2ResolverRecord", () => {
  let recordId = BigInt.fromI32(5);
  let resolverId = Address.fromString(PERMISSIONED_RESOLVER).toHexString();
  let recordEntityId = resolverId.concat(bigIntHex32(recordId));
  let hash = Bytes.fromUTF8("ipfs://somehash");

  handleContenthashUpdated(createContenthashUpdatedEvent(recordId, hash));
  assert.fieldEquals(
    "ENSv2ResolverRecord",
    recordEntityId,
    "contenthash",
    hash.toHexString()
  );

  handleNameUpdated(createNameUpdatedEvent(recordId, "julia.eth"));
  assert.fieldEquals(
    "ENSv2ResolverRecord",
    recordEntityId,
    "primaryName",
    "julia.eth"
  );
});

test("AddressUpdated for two coinTypes on the same recordId produces two distinct ENSv2ResolverAddress rows", () => {
  let recordId = BigInt.fromI32(6);
  let resolverId = Address.fromString(PERMISSIONED_RESOLVER).toHexString();
  let recordEntityId = resolverId.concat(bigIntHex32(recordId));
  let ethCoinType = BigInt.fromI32(60);
  let btcCoinType = BigInt.fromI32(0);
  let ethAddr = Bytes.fromI32(111);
  let btcAddr = Bytes.fromI32(222);
  let ethId = recordEntityId.concat(bigIntHex32(ethCoinType));
  let btcId = recordEntityId.concat(bigIntHex32(btcCoinType));

  handleAddressUpdated(createAddressUpdatedEvent(recordId, ethCoinType, ethAddr));
  handleAddressUpdated(createAddressUpdatedEvent(recordId, btcCoinType, btcAddr));

  assert.fieldEquals("ENSv2ResolverAddress", ethId, "addressBytes", ethAddr.toHexString());
  assert.fieldEquals("ENSv2ResolverAddress", btcId, "addressBytes", btcAddr.toHexString());
});

test("TextUpdated for two keys on the same recordId produces two distinct ENSv2ResolverText rows", () => {
  let recordId = BigInt.fromI32(7);
  let resolverId = Address.fromString(PERMISSIONED_RESOLVER).toHexString();
  let recordEntityId = resolverId.concat(bigIntHex32(recordId));
  let avatarKeyHash = Bytes.fromI32(1);
  let urlKeyHash = Bytes.fromI32(2);
  let avatarId = recordEntityId.concat(hexOf(avatarKeyHash));
  let urlId = recordEntityId.concat(hexOf(urlKeyHash));

  handleTextUpdated(
    createTextUpdatedEvent(recordId, avatarKeyHash, "avatar", "ipfs://avatar")
  );
  handleTextUpdated(
    createTextUpdatedEvent(recordId, urlKeyHash, "url", "https://example.com")
  );

  assert.fieldEquals("ENSv2ResolverText", avatarId, "key", "avatar");
  assert.fieldEquals("ENSv2ResolverText", avatarId, "value", "ipfs://avatar");
  assert.fieldEquals("ENSv2ResolverText", urlId, "key", "url");
  assert.fieldEquals("ENSv2ResolverText", urlId, "value", "https://example.com");
});

test("DataUpdated produces ENSv2ResolverRecordData with its recoverable value", () => {
  let recordId = BigInt.fromI32(8);
  let resolverId = Address.fromString(PERMISSIONED_RESOLVER).toHexString();
  let recordEntityId = resolverId.concat(bigIntHex32(recordId));
  let keyHash = Bytes.fromByteArray(crypto.keccak256(Bytes.fromUTF8("pubkey")));
  let id = recordEntityId.concat(hexOf(keyHash));
  let value = Bytes.fromUTF8("real bytes, not just a hash");

  handleRecordDataUpdated(createDataUpdatedEvent(recordId, keyHash, "pubkey", value));

  assert.fieldEquals("ENSv2ResolverRecordData", id, "key", "pubkey");
  assert.fieldEquals("ENSv2ResolverRecordData", id, "value", value.toHexString());
});

test("AddressUpdated on the same recordId+coinType twice overwrites the existing row instead of duplicating", () => {
  let recordId = BigInt.fromI32(10);
  let resolverId = Address.fromString(PERMISSIONED_RESOLVER).toHexString();
  let recordEntityId = resolverId.concat(bigIntHex32(recordId));
  let coinType = BigInt.fromI32(60);
  let firstAddr = Bytes.fromI32(111);
  let secondAddr = Bytes.fromI32(222);
  let id = recordEntityId.concat(bigIntHex32(coinType));

  handleAddressUpdated(createAddressUpdatedEvent(recordId, coinType, firstAddr));
  assert.fieldEquals("ENSv2ResolverAddress", id, "addressBytes", firstAddr.toHexString());

  handleAddressUpdated(createAddressUpdatedEvent(recordId, coinType, secondAddr));
  assert.fieldEquals("ENSv2ResolverAddress", id, "addressBytes", secondAddr.toHexString());
  // Overwritten in place, not duplicated -- one ENSv2ResolverAddress row and
  // one ENSv2ResolverRecord row (getOrCreateRecord's own load-or-new is
  // exercised across both calls too, not just created once).
  assert.entityCount("ENSv2ResolverAddress", 1);
  assert.entityCount("ENSv2ResolverRecord", 1);
});

test("ABIUpdated and InterfaceUpdated produce their own child rows keyed by recordId", () => {
  let recordId = BigInt.fromI32(9);
  let resolverId = Address.fromString(PERMISSIONED_RESOLVER).toHexString();
  let recordEntityId = resolverId.concat(bigIntHex32(recordId));
  let contentType = BigInt.fromI32(1);
  let abiId = recordEntityId.concat(bigIntHex32(contentType));

  handleABIUpdated(createABIUpdatedEvent(recordId, contentType));
  assert.fieldEquals("ENSv2ResolverABI", abiId, "contentType", "1");

  let interfaceId = Bytes.fromI32(0x01ffc9a7);
  let implementer = Address.fromString(
    "0x8e8db5ccef88cca9d624701db544989c996e3211"
  );
  let interfaceEntityId = recordEntityId.concat(hexOf(interfaceId));

  handleInterfaceUpdated(
    createInterfaceUpdatedEvent(recordId, interfaceId, implementer)
  );
  assert.fieldEquals(
    "ENSv2ResolverInterface",
    interfaceEntityId,
    "implementer",
    implementer.toHexString()
  );
});

test("record membership indexes are append-once across repeated links and A to B to A relinks", () => {
  let node = Bytes.fromI32(201);
  let name = encodeLabel("membership");
  let recordA = BigInt.fromI32(20);
  let recordB = BigInt.fromI32(21);
  let resolverId = Address.fromString(PERMISSIONED_RESOLVER).toHexString();
  let recordAId = resolverId.concat(bigIntHex32(recordA));
  let recordBId = resolverId.concat(bigIntHex32(recordB));

  handleLinked(createLinkedEvent(recordA, node, name));
  handleLinked(createLinkedEvent(recordA, node, name));
  assert.fieldEquals("ENSv2ResolverRecord", recordAId, "linkIndexCount", "1");
  assert.entityCount("ENSv2ResolverRecordLinkIndex", 1);

  handleLinked(createLinkedEvent(recordB, node, name));
  let membershipAId = recordAId.concat(hexOf(node));
  let membershipBId = recordBId.concat(hexOf(node));
  assert.fieldEquals(
    "ENSv2ResolverRecordLinkMembership",
    membershipAId,
    "active",
    "false"
  );
  assert.fieldEquals(
    "ENSv2ResolverRecordLinkMembership",
    membershipBId,
    "active",
    "true"
  );

  handleLinked(createLinkedEvent(recordA, node, name));
  assert.fieldEquals("ENSv2ResolverRecord", recordAId, "linkIndexCount", "1");
  assert.fieldEquals("ENSv2ResolverRecord", recordBId, "linkIndexCount", "1");
  assert.entityCount("ENSv2ResolverRecordLinkIndex", 2);
  assert.fieldEquals(
    "ENSv2ResolverRecordLinkMembership",
    membershipAId,
    "active",
    "true"
  );
  assert.fieldEquals(
    "ENSv2ResolverRecordLinkMembership",
    membershipBId,
    "active",
    "false"
  );
});

test("linking a populated record replaces the eligible legacy Resolver snapshot", () => {
  let node = Bytes.fromI32(202);
  let legacyId = seedProjectedDomain(node);
  let recordId = BigInt.fromI32(22);
  let ethAddress = Address.fromString(
    "0x33333333333333333333333333333333333333cc"
  );
  let contenthash = Bytes.fromUTF8("ipfs://option-b");
  let textKeyHash = Bytes.fromByteArray(
    crypto.keccak256(Bytes.fromUTF8("avatar"))
  );

  handleAddressUpdated(
    createAddressUpdatedEvent(recordId, BigInt.fromI32(60), ethAddress)
  );
  handleContenthashUpdated(
    createContenthashUpdatedEvent(recordId, contenthash)
  );
  handleTextUpdated(
    createTextUpdatedEvent(recordId, textKeyHash, "avatar", "ipfs://avatar")
  );

  let beforeLink = Resolver.load(legacyId)!;
  assert.assertTrue(!beforeLink.addr);
  assert.assertTrue(!beforeLink.contentHash);

  handleLinked(createLinkedEvent(recordId, node, encodeLabel("snapshot")));

  let legacy = Resolver.load(legacyId)!;
  assert.stringEquals(ethAddress.toHexString(), legacy.addr!);
  assert.bytesEquals(contenthash, legacy.contentHash!);
  assert.i32Equals(1, legacy.coinTypes!.length);
  assert.bigIntEquals(BigInt.fromI32(60), legacy.coinTypes![0]);
  assert.i32Equals(1, legacy.texts!.length);
  assert.stringEquals("avatar", legacy.texts![0]);
  assert.fieldEquals(
    "Domain",
    node.toHexString(),
    "resolvedAddress",
    ethAddress.toHexString()
  );
  assert.entityCount("AddrChanged", 0);
  assert.entityCount("ContenthashChanged", 0);
  assert.entityCount("TextChanged", 0);
});

test("shared records fan out only to active explicitly linked names", () => {
  let nodeA = Bytes.fromI32(203);
  let nodeB = Bytes.fromI32(204);
  let nodeC = Bytes.fromI32(205);
  let legacyAId = seedProjectedDomain(nodeA);
  let legacyBId = seedProjectedDomain(nodeB);
  let legacyCId = seedProjectedDomain(nodeC);
  let sharedRecord = BigInt.fromI32(23);
  let otherRecord = BigInt.fromI32(24);

  handleLinked(createLinkedEvent(sharedRecord, nodeA, encodeLabel("a")));
  handleLinked(createLinkedEvent(sharedRecord, nodeB, encodeLabel("b")));
  handleLinked(createLinkedEvent(otherRecord, nodeC, encodeLabel("c")));

  let contenthash = Bytes.fromUTF8("shared-content");
  handleContenthashUpdated(
    createContenthashUpdatedEvent(sharedRecord, contenthash)
  );

  assert.bytesEquals(contenthash, Resolver.load(legacyAId)!.contentHash!);
  assert.bytesEquals(contenthash, Resolver.load(legacyBId)!.contentHash!);
  assert.assertTrue(!Resolver.load(legacyCId)!.contentHash);
});

test("relink snapshots replacement state, stale-record updates stop, and unlink clears without root fallback", () => {
  let node = Bytes.fromI32(206);
  let legacyId = seedProjectedDomain(node);
  let recordA = BigInt.fromI32(25);
  let recordB = BigInt.fromI32(26);
  let ethAddress = Address.fromString(
    "0x44444444444444444444444444444444444444dd"
  );
  let contentA = Bytes.fromUTF8("record-a");
  let contentB = Bytes.fromUTF8("record-b");

  handleAddressUpdated(
    createAddressUpdatedEvent(recordA, BigInt.fromI32(60), ethAddress)
  );
  handleContenthashUpdated(createContenthashUpdatedEvent(recordA, contentA));
  handleContenthashUpdated(createContenthashUpdatedEvent(recordB, contentB));
  handleLinked(createLinkedEvent(recordA, node, encodeLabel("relink")));
  assert.stringEquals(ethAddress.toHexString(), Resolver.load(legacyId)!.addr!);

  handleLinked(createLinkedEvent(recordB, node, encodeLabel("relink")));
  let afterRelink = Resolver.load(legacyId)!;
  assert.bytesEquals(contentB, afterRelink.contentHash!);
  assert.assertTrue(!afterRelink.addr);
  assert.assertTrue(!afterRelink.coinTypes);

  handleContenthashUpdated(
    createContenthashUpdatedEvent(recordA, Bytes.fromUTF8("stale-a"))
  );
  assert.bytesEquals(contentB, Resolver.load(legacyId)!.contentHash!);

  handleLinked(createLinkedEvent(BigInt.zero(), node, encodeLabel("relink")));
  let afterUnlink = Resolver.load(legacyId)!;
  assert.assertTrue(!afterUnlink.addr);
  assert.assertTrue(!afterUnlink.contentHash);
  assert.assertTrue(!afterUnlink.texts);
  assert.assertTrue(!afterUnlink.coinTypes);
  assert.assertTrue(!Domain.load(node.toHexString())!.resolvedAddress);
});

test("empty contenthash replaces the projected value with the legacy empty-bytes clearing representation", () => {
  let node = Bytes.fromI32(210);
  let legacyId = seedProjectedDomain(node);
  let recordId = BigInt.fromI32(30);
  let initial = Bytes.fromUTF8("ipfs://before-clear");
  let empty = Bytes.fromUint8Array(new Uint8Array(0));

  handleLinked(createLinkedEvent(recordId, node, encodeLabel("contentclear")));
  handleContenthashUpdated(createContenthashUpdatedEvent(recordId, initial));
  assert.bytesEquals(initial, Resolver.load(legacyId)!.contentHash!);

  handleContenthashUpdated(createContenthashUpdatedEvent(recordId, empty));

  let projected = Resolver.load(legacyId)!;
  assert.i32Equals(0, projected.contentHash!.length);
  let nativeRecord = ENSv2ResolverRecord.load(
    Bytes.fromHexString(
      Address.fromString(PERMISSIONED_RESOLVER)
        .toHexString()
        .concat(bigIntHex32(recordId))
    )
  )!;
  assert.i32Equals(0, nativeRecord.contenthash!.length);
});

test("empty or malformed ETH bytes clear address state while non-ETH updates only extend observed coin types", () => {
  let node = Bytes.fromI32(207);
  let legacyId = seedProjectedDomain(node);
  let recordId = BigInt.fromI32(27);
  let ethAddress = Address.fromString(
    "0x55555555555555555555555555555555555555ee"
  );
  handleLinked(createLinkedEvent(recordId, node, encodeLabel("coins")));
  handleAddressUpdated(
    createAddressUpdatedEvent(recordId, BigInt.fromI32(60), ethAddress)
  );

  let withCoins = Resolver.load(legacyId)!;
  assert.stringEquals(ethAddress.toHexString(), withCoins.addr!);
  assert.i32Equals(1, withCoins.coinTypes!.length);

  // A non-empty value with the wrong byte length is not a valid EVM
  // address. It remains available in the native record but must clear the
  // legacy address projection rather than fabricating an Account id.
  handleAddressUpdated(
    createAddressUpdatedEvent(
      recordId,
      BigInt.fromI32(60),
      Bytes.fromUint8Array(new Uint8Array(19))
    )
  );
  let malformed = Resolver.load(legacyId)!;
  assert.assertTrue(!malformed.addr);
  assert.assertTrue(!Domain.load(node.toHexString())!.resolvedAddress);
  assert.i32Equals(1, malformed.coinTypes!.length);

  handleAddressUpdated(
    createAddressUpdatedEvent(recordId, BigInt.fromI32(60), ethAddress)
  );
  handleAddressUpdated(
    createAddressUpdatedEvent(recordId, BigInt.fromI32(0), Bytes.fromI32(7))
  );
  assert.i32Equals(2, Resolver.load(legacyId)!.coinTypes!.length);

  handleAddressUpdated(
    createAddressUpdatedEvent(
      recordId,
      BigInt.fromI32(60),
      Bytes.fromUint8Array(new Uint8Array(0))
    )
  );
  let cleared = Resolver.load(legacyId)!;
  assert.assertTrue(!cleared.addr);
  assert.i32Equals(2, cleared.coinTypes!.length);
  assert.fieldEquals(
    "ENSv2ResolverRecord",
    Address.fromString(PERMISSIONED_RESOLVER)
      .toHexString()
      .concat(bigIntHex32(recordId)),
    "addressIndexCount",
    "2"
  );
});

test("resolver events remain native-only when no Domain projection exists", () => {
  let node = Bytes.fromI32(208);
  let recordId = BigInt.fromI32(28);
  handleLinked(createLinkedEvent(recordId, node, encodeLabel("nativeonly")));
  handleContenthashUpdated(
    createContenthashUpdatedEvent(recordId, Bytes.fromUTF8("native"))
  );

  assert.entityCount("Domain", 0);
  assert.entityCount("Resolver", 0);
  assert.entityCount("ENSv2ResolverRecord", 1);
  assert.entityCount("ENSv2ResolverRecordLinkMembership", 1);
});

test("a record update reaches its explicit root link but not unlinked fallback consumers", () => {
  let rootNode = Bytes.fromHexString(
    "0x0000000000000000000000000000000000000000000000000000000000000000"
  );
  let fallbackNode = Bytes.fromI32(209);
  let rootLegacyId = seedProjectedDomain(rootNode);
  let fallbackLegacyId = seedProjectedDomain(fallbackNode);
  let defaultRecord = BigInt.fromI32(29);
  handleLinked(
    createLinkedEvent(defaultRecord, rootNode, Bytes.fromHexString("0x00"))
  );

  let contenthash = Bytes.fromUTF8("root-default");
  handleContenthashUpdated(
    createContenthashUpdatedEvent(defaultRecord, contenthash)
  );

  assert.bytesEquals(contenthash, Resolver.load(rootLegacyId)!.contentHash!);
  assert.assertTrue(!Resolver.load(fallbackLegacyId)!.contentHash);
});

test("ResourceArgument stores resolver-scoped role metadata and repeated emissions update in place", () => {
  let resource = BigInt.fromI32(300);
  let firstArg = Bytes.fromUTF8("avatar");
  let secondArg = Bytes.fromUTF8("url");
  let id = Address.fromString(PERMISSIONED_RESOLVER)
    .toHexString()
    .concat(bigIntHex32(resource));

  handleResourceArgument(createResourceArgumentEvent(resource, firstArg));
  assert.fieldEquals(
    "ENSv2ResolverResourceArgument",
    id,
    "arg",
    firstArg.toHexString()
  );

  handleResourceArgument(createResourceArgumentEvent(resource, secondArg));
  assert.entityCount("ENSv2ResolverResourceArgument", 1);
  assert.fieldEquals(
    "ENSv2ResolverResourceArgument",
    id,
    "arg",
    secondArg.toHexString()
  );
});

// --- v2.2.1 -----------------------------------------------------------------

// The PermissionedResolver source has no address: any contract can emit a
// Linked event with arbitrary name bytes. Malformed names must decode to
// null, never abort the mapping (which would halt the whole subgraph).
test("decodeName rejects malformed DNS names instead of aborting", () => {
  assert.assertNull(decodeName(Bytes.fromHexString("0x")));
  assert.assertNull(decodeName(Bytes.fromHexString("0x09666f6f"))); // length past the end
  assert.assertNull(decodeName(Bytes.fromHexString("0x03666f6f03657468"))); // no root terminator
  assert.assertNull(decodeName(Bytes.fromHexString("0x03666f6f")));
  let root = decodeName(Bytes.fromHexString("0x00"))!;
  assert.stringEquals(".", root[1]);
  let fooEth = decodeName(Bytes.fromHexString("0x03666f6f0365746800"))!;
  assert.stringEquals("foo", fooEth[0]);
  assert.stringEquals("foo.eth", fooEth[1]);
});

test("a Linked event with a malformed name is indexed with no decoded name", () => {
  let node = Bytes.fromI32(301);
  let recordId = BigInt.fromI32(31);
  handleLinked(createLinkedEvent(recordId, node, Bytes.fromHexString("0x")));
  handleLinked(createLinkedEvent(recordId, Bytes.fromI32(302), Bytes.fromHexString("0x09666f6f")));
  let link = ENSv2ResolverLink.load(
    resolverLinkId(Address.fromString(PERMISSIONED_RESOLVER), node)
  )!;
  assert.assertTrue(!link.nameDecoded);
  assert.assertTrue(link.active);
  assert.entityCount("ENSv2ResolverLink", 2);
});

// PermissionedResolver.addr(): coin type 60, or the ENSIP-19 default EVM
// address (coin type 1 << 31) when coin type 60 is unset or empty.
test("the ENSIP-19 default EVM address is the fallback for addr()", () => {
  const DEFAULT_EVM = BigInt.fromString("2147483648");
  let node = Bytes.fromI32(303);
  let legacyId = seedProjectedDomain(node);
  let recordId = BigInt.fromI32(33);
  let defaultAddress = Address.fromString("0x66666666666666666666666666666666666666ff");
  let ethAddress = Address.fromString("0x77777777777777777777777777777777777777aa");
  handleLinked(createLinkedEvent(recordId, node, encodeLabel("fallback")));

  // Only the default is set: it is the address.
  handleAddressUpdated(createAddressUpdatedEvent(recordId, DEFAULT_EVM, defaultAddress));
  assert.stringEquals(defaultAddress.toHexString(), Resolver.load(legacyId)!.addr!);
  assert.fieldEquals("Domain", node.toHexString(), "resolvedAddress", defaultAddress.toHexString());

  // Coin type 60 overrides it.
  handleAddressUpdated(createAddressUpdatedEvent(recordId, BigInt.fromI32(60), ethAddress));
  assert.fieldEquals("Domain", node.toHexString(), "resolvedAddress", ethAddress.toHexString());

  // Clearing coin type 60 falls back to the default again.
  handleAddressUpdated(
    createAddressUpdatedEvent(recordId, BigInt.fromI32(60), Bytes.fromUint8Array(new Uint8Array(0)))
  );
  assert.fieldEquals("Domain", node.toHexString(), "resolvedAddress", defaultAddress.toHexString());

  // An explicit zero address on coin type 60 is a value, not unset.
  handleAddressUpdated(
    createAddressUpdatedEvent(recordId, BigInt.fromI32(60), Bytes.fromUint8Array(new Uint8Array(20)))
  );
  assert.fieldEquals(
    "Domain",
    node.toHexString(),
    "resolvedAddress",
    "0x0000000000000000000000000000000000000000"
  );

  // Clearing the default with coin type 60 back to unset leaves no address.
  handleAddressUpdated(
    createAddressUpdatedEvent(recordId, BigInt.fromI32(60), Bytes.fromUint8Array(new Uint8Array(0)))
  );
  handleAddressUpdated(
    createAddressUpdatedEvent(recordId, DEFAULT_EVM, Bytes.fromUint8Array(new Uint8Array(0)))
  );
  assert.assertTrue(!Resolver.load(legacyId)!.addr);
  assert.assertTrue(!Domain.load(node.toHexString())!.resolvedAddress);
});

// --- ENSv1 names that select a PermissionedResolver (v2.3.1) ---

function createV1NewResolverEvent(node: Bytes, resolver: string): NewResolver {
  let mockEvent = newMockEvent();
  let event = new NewResolver(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(node))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "resolver",
      ethereum.Value.fromAddress(Address.fromString(resolver))
    )
  );
  return event;
}

// An ENSv1 Domain (no ENSv2 path, no resolver yet).
function seedV1Domain(node: Bytes): void {
  let owner = new Account(DOMAIN_OWNER);
  owner.save();
  let domain = new Domain(node.toHexString());
  domain.subdomainCount = 0;
  domain.isMigrated = true;
  domain.createdAt = BigInt.zero();
  domain.owner = owner.id;
  domain.storedOffchain = false; // Blockscout/BENS extension
  domain.resolvedWithWildcard = false; // Blockscout/BENS extension
  domain.save();
}

test("an ENSv1 name that selects a PermissionedResolver after its record was linked and set is hydrated from that record", () => {
  let node = Bytes.fromHexString(
    "0xaaaa000000000000000000000000000000000000000000000000000000000001"
  );
  seedV1Domain(node);
  let recordId = BigInt.fromI32(77);
  let eth = Address.fromString("0x33333333333333333333333333333333333333cc");
  let contenthash = Bytes.fromUTF8("ipfs://v1-name");
  handleLinked(createLinkedEvent(recordId, node, encodeLabel("v1name")));
  handleAddressUpdated(
    createAddressUpdatedEvent(recordId, BigInt.fromI32(60), eth)
  );
  handleContenthashUpdated(createContenthashUpdatedEvent(recordId, contenthash));

  handleNewResolver(createV1NewResolverEvent(node, PERMISSIONED_RESOLVER));

  let legacyId = createResolverID(node, Address.fromString(PERMISSIONED_RESOLVER));
  assert.fieldEquals("Domain", node.toHexString(), "resolver", legacyId);
  assert.fieldEquals("Domain", node.toHexString(), "resolvedAddress", eth.toHexString());
  let legacy = Resolver.load(legacyId)!;
  assert.stringEquals(eth.toHexString(), legacy.addr!);
  assert.bytesEquals(contenthash, legacy.contentHash!);
});

test("an ENSv1 name re-selecting a PermissionedResolver picks up records changed while it pointed elsewhere", () => {
  let node = Bytes.fromHexString(
    "0xaaaa000000000000000000000000000000000000000000000000000000000002"
  );
  seedV1Domain(node);
  let recordId = BigInt.fromI32(78);
  let a1 = Address.fromString("0x33333333333333333333333333333333333333c1");
  let a2 = Address.fromString("0x33333333333333333333333333333333333333c2");
  handleNewResolver(createV1NewResolverEvent(node, PERMISSIONED_RESOLVER));
  handleLinked(createLinkedEvent(recordId, node, encodeLabel("v1name")));
  handleAddressUpdated(createAddressUpdatedEvent(recordId, BigInt.fromI32(60), a1));
  assert.fieldEquals("Domain", node.toHexString(), "resolvedAddress", a1.toHexString());

  handleNewResolver(
    createV1NewResolverEvent(node, "0x0000000000000000000000000000000000000000")
  );
  assert.assertTrue(!Domain.load(node.toHexString())!.resolvedAddress);
  handleAddressUpdated(createAddressUpdatedEvent(recordId, BigInt.fromI32(60), a2));

  handleNewResolver(createV1NewResolverEvent(node, PERMISSIONED_RESOLVER));
  assert.fieldEquals("Domain", node.toHexString(), "resolvedAddress", a2.toHexString());
});

test("an ENSv1 name selecting a PermissionedResolver whose link was removed gets no address", () => {
  let node = Bytes.fromHexString(
    "0xaaaa000000000000000000000000000000000000000000000000000000000003"
  );
  seedV1Domain(node);
  let recordId = BigInt.fromI32(79);
  let eth = Address.fromString("0x33333333333333333333333333333333333333c3");
  handleLinked(createLinkedEvent(recordId, node, encodeLabel("v1name")));
  handleAddressUpdated(createAddressUpdatedEvent(recordId, BigInt.fromI32(60), eth));
  handleLinked(createLinkedEvent(BigInt.zero(), node, encodeLabel("v1name")));

  handleNewResolver(createV1NewResolverEvent(node, PERMISSIONED_RESOLVER));
  assert.assertTrue(!Domain.load(node.toHexString())!.resolvedAddress);
});
