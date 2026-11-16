import {
  Address,
  BigInt,
  ByteArray,
  Bytes,
  crypto,
  ethereum,
} from "@graphprotocol/graph-ts";
import {
  afterEach,
  assert,
  clearStore,
  dataSourceMock,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import {
  handleExpiryUpdated,
  handleLabelRegistered,
  handleLabelUnregistered,
  handleResolverUpdated,
  handleSubregistryUpdated,
} from "../src/ensv2Registry";
import {
  handleAddressUpdated,
  handleContenthashUpdated,
  handleLinked,
} from "../src/ensv2Resolver";
import { namespaceId, pathNamehash, registryNamespaceIndexId, resolverLinkId } from "../src/ensv2Utils";
import { getOrCreateRegistry } from "../src/ensv2Discovery";
import { createResolverID, handleAddrChanged } from "../src/resolver";
import { AddrChanged } from "../src/types/Resolver/Resolver";
import {
  Domain,
  ENSv2NamePath,
  ENSv2NameSlot,
  ENSv2Registry,
  Resolver,
} from "../src/types/schema";
import {
  AddressUpdated,
  ContenthashUpdated,
  Linked,
} from "../src/types/PermissionedResolver/PermissionedResolver";
import {
  ExpiryUpdated,
  LabelRegistered,
  LabelUnregistered,
  ResolverUpdated,
  SubregistryUpdated,
} from "../src/types/RootRegistry/PermissionedRegistry";

const ROOT_REGISTRY = "0xB458D6a3a77919449d03e7A6903C26827c1eC43f";
const ETH_REGISTRY = "0xD4eBcbBdF463C9c45784603Db0dDD499BC44A8B4";
const OWNER = "0x89205A3A3b2A69De6Dbf7f01ED13B2108B2c43e7";
const OWNER_2 = "0xF0205A3A3b2A69De6Dbf7f01ED13B2108B2c4321";
const SENDER = "0x11111111111111111111111111111111111111aa";
const ROOT_NAMEHASH =
  "0x0000000000000000000000000000000000000000000000000000000000000000";

// TWO_POW_32: toSlotId zeroes the low 32 bits of a tokenId (src/ensv2Utils.ts),
// so an exact multiple of 2^32 survives toSlotId unchanged and distinct
// multiples give distinct slots within one registry — used below instead of
// small sequential tokenIds (which would all collapse to slot 0).
const TWO_POW_32 = BigInt.fromI64(4294967296);

function slotToken(n: i32): BigInt {
  return TWO_POW_32.times(BigInt.fromI32(n));
}

// assert.fieldEquals compares an entity's id as its lowercase-hex string
// form regardless of the underlying GraphQL type.
// Production code now builds composite ids as fixed-width Bytes
// concatenation with no delimiter (a BigInt component is a 32-byte
// big-endian value, src/utils.ts::uint256ToByteArray) — this mirrors that
// exact encoding to reproduce the same hex string.
function bigIntHex32(i: BigInt): string {
  return i.toHex().slice(2).padStart(64, "0");
}

function slotIdFor(registryAddress: string, n: i32): string {
  return Address.fromString(registryAddress)
    .toHexString()
    .concat(bigIntHex32(slotToken(n)));
}

function slotExists(id: string): boolean {
  let slot = ENSv2NameSlot.load(Bytes.fromHexString(id));
  let exists = slot != null;
  return exists;
}

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

const createLabelRegisteredEvent = (
  registryAddress: string,
  tokenId: BigInt,
  labelHash: Bytes,
  label: string,
  owner: string = OWNER,
  expiry: BigInt = BigInt.fromI32(2000000000)
): LabelRegistered => {
  let mockEvent = newMockEvent();
  let event = new LabelRegistered(
    Address.fromString(registryAddress),
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
      "tokenId",
      ethereum.Value.fromUnsignedBigInt(tokenId)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "labelHash",
      ethereum.Value.fromFixedBytes(labelHash)
    )
  );
  event.parameters.push(
    new ethereum.EventParam("label", ethereum.Value.fromString(label))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "owner",
      ethereum.Value.fromAddress(Address.fromString(owner))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "expiry",
      ethereum.Value.fromUnsignedBigInt(expiry)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "sender",
      ethereum.Value.fromAddress(Address.fromString(SENDER))
    )
  );
  return event;
};

const createExpiryUpdatedEvent = (
  registryAddress: string,
  tokenId: BigInt,
  newExpiry: BigInt
): ExpiryUpdated => {
  let mockEvent = newMockEvent();
  let event = new ExpiryUpdated(
    Address.fromString(registryAddress),
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
      "tokenId",
      ethereum.Value.fromUnsignedBigInt(tokenId)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "newExpiry",
      ethereum.Value.fromUnsignedBigInt(newExpiry)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "sender",
      ethereum.Value.fromAddress(Address.fromString(SENDER))
    )
  );
  return event;
};

const createLabelUnregisteredEvent = (
  registryAddress: string,
  tokenId: BigInt
): LabelUnregistered => {
  let mockEvent = newMockEvent();
  let event = new LabelUnregistered(
    Address.fromString(registryAddress),
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
      "tokenId",
      ethereum.Value.fromUnsignedBigInt(tokenId)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "sender",
      ethereum.Value.fromAddress(Address.fromString(SENDER))
    )
  );
  return event;
};

const createSubregistryUpdatedEvent = (
  registryAddress: string,
  tokenId: BigInt,
  subregistry: string
): SubregistryUpdated => {
  let mockEvent = newMockEvent();
  let event = new SubregistryUpdated(
    Address.fromString(registryAddress),
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
      "tokenId",
      ethereum.Value.fromUnsignedBigInt(tokenId)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "subregistry",
      ethereum.Value.fromAddress(Address.fromString(subregistry))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "sender",
      ethereum.Value.fromAddress(Address.fromString(SENDER))
    )
  );
  return event;
};

const createResolverUpdatedEvent = (
  registryAddress: string,
  tokenId: BigInt,
  resolver: string
): ResolverUpdated => {
  let mockEvent = newMockEvent();
  let event = new ResolverUpdated(
    Address.fromString(registryAddress),
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
      "tokenId",
      ethereum.Value.fromUnsignedBigInt(tokenId)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "resolver",
      ethereum.Value.fromAddress(Address.fromString(resolver))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "sender",
      ethereum.Value.fromAddress(Address.fromString(SENDER))
    )
  );
  return event;
};

const createLinkedEvent = (
  resolverAddress: string,
  recordId: BigInt,
  node: Bytes,
  name: Bytes
): Linked => {
  let mockEvent = newMockEvent();
  let event = new Linked(
    Address.fromString(resolverAddress),
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
  resolverAddress: string,
  recordId: BigInt,
  hash: Bytes
): ContenthashUpdated => {
  let mockEvent = newMockEvent();
  let event = new ContenthashUpdated(
    Address.fromString(resolverAddress),
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

afterEach(() => {
  dataSourceMock.resetValues();
  clearStore();
});

test("RootRegistry paths do not create or mutate legacy eth/reverse Domain rows", () => {
  dataSourceMock.setNetwork("sepolia");

  let ethLabelHash = Bytes.fromByteArray(
    crypto.keccak256(ByteArray.fromUTF8("eth"))
  );
  let reverseLabelHash = Bytes.fromByteArray(
    crypto.keccak256(ByteArray.fromUTF8("reverse"))
  );
  let ethDomainId = pathNamehash(
    Bytes.fromHexString(ROOT_NAMEHASH),
    ethLabelHash
  ).toHexString();
  let reverseDomainId = pathNamehash(
    Bytes.fromHexString(ROOT_NAMEHASH),
    reverseLabelHash
  ).toHexString();
  let legacyOwner = Address.fromString(OWNER_2).toHexString();
  let legacyRegistrant = Address.fromString(OWNER).toHexString();
  let resolverAddress = Address.fromString(
    "0x2222222222222222222222222222222222222222"
  );
  let resolverId = createResolverID(
    Bytes.fromHexString(ethDomainId),
    resolverAddress
  );

  let ethDomain = new Domain(ethDomainId);
  ethDomain.storedOffchain = false; // Blockscout/BENS extension
  ethDomain.resolvedWithWildcard = false; // Blockscout/BENS extension
  ethDomain.name = "eth";
  ethDomain.owner = legacyOwner;
  ethDomain.registrant = legacyRegistrant;
  ethDomain.resolver = resolverId;
  ethDomain.expiryDate = BigInt.fromI32(1234567890);
  ethDomain.isMigrated = false;
  ethDomain.subdomainCount = 0;
  ethDomain.createdAt = BigInt.fromI32(1);
  ethDomain.save();

  let resolver = new Resolver(resolverId);
  resolver.domain = ethDomainId;
  resolver.address = resolverAddress;
  resolver.save();

  let reverseDomain = new Domain(reverseDomainId);
  reverseDomain.storedOffchain = false; // Blockscout/BENS extension
  reverseDomain.resolvedWithWildcard = false; // Blockscout/BENS extension
  reverseDomain.name = "reverse";
  reverseDomain.owner = legacyOwner;
  reverseDomain.registrant = legacyRegistrant;
  reverseDomain.expiryDate = BigInt.fromI32(1234567891);
  reverseDomain.isMigrated = false;
  reverseDomain.subdomainCount = 0;
  reverseDomain.createdAt = BigInt.fromI32(1);
  reverseDomain.save();

  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(1),
      ethLabelHash,
      "eth",
      OWNER
    )
  );
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(2),
      reverseLabelHash,
      "reverse",
      OWNER
    )
  );

  assert.fieldEquals("Domain", ethDomainId, "owner", legacyOwner);
  assert.fieldEquals("Domain", ethDomainId, "registrant", legacyRegistrant);
  assert.fieldEquals("Domain", ethDomainId, "expiryDate", "1234567890");
  assert.fieldEquals("Domain", ethDomainId, "isMigrated", "false");
  assert.fieldEquals("Domain", ethDomainId, "resolver", resolverId);
  assert.fieldEquals("Domain", reverseDomainId, "owner", legacyOwner);
  assert.fieldEquals(
    "Domain",
    reverseDomainId,
    "registrant",
    legacyRegistrant
  );
  assert.fieldEquals("Domain", reverseDomainId, "expiryDate", "1234567891");
  assert.fieldEquals("Domain", reverseDomainId, "isMigrated", "false");

  let ethPath = ENSv2NamePath.load(Bytes.fromHexString(ethDomainId));
  let reversePath = ENSv2NamePath.load(Bytes.fromHexString(reverseDomainId));
  let ethPathHasDomain = false;
  let reversePathHasDomain = false;
  if (ethPath != null && ethPath.domain) {
    ethPathHasDomain = true;
  }
  if (reversePath != null && reversePath.domain) {
    reversePathHasDomain = true;
  }
  assert.assertTrue(!ethPathHasDomain);
  assert.assertTrue(!reversePathHasDomain);

  // A root-slot re-registration must not clear the legacy resolver either.
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(1),
      ethLabelHash,
      "eth",
      OWNER
    )
  );
  assert.fieldEquals("Domain", ethDomainId, "resolver", resolverId);
});

test("3-level nested registration produces correct name/namehash/depth chain", () => {
  dataSourceMock.setNetwork("sepolia");

  const USER_REGISTRY_A = "0x121234567890123456789012345678901234abcd";

  // eth (RootRegistry, depth 0)
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(1),
      Bytes.fromI32(1),
      "eth"
    )
  );
  let ethPathId = pathNamehash(
    Bytes.fromHexString(ROOT_NAMEHASH),
    Bytes.fromI32(1)
  ).toHexString();
  assert.fieldEquals("ENSv2NamePath", ethPathId, "name", "eth");
  assert.fieldEquals("ENSv2NamePath", ethPathId, "depth", "0");

  // link RootRegistry's "eth" slot -> ETHRegistry
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), ETH_REGISTRY)
  );

  // base (ETHRegistry, depth 1)
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ETH_REGISTRY,
      slotToken(1),
      Bytes.fromI32(2),
      "base"
    )
  );
  let basePathId = pathNamehash(
    Bytes.fromHexString(ethPathId),
    Bytes.fromI32(2)
  ).toHexString();
  assert.fieldEquals("ENSv2NamePath", basePathId, "name", "base.eth");
  assert.fieldEquals("ENSv2NamePath", basePathId, "depth", "1");
  assert.fieldEquals("ENSv2NamePath", basePathId, "parent", ethPathId);

  // link ETHRegistry's "base" slot -> USER_REGISTRY_A
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ETH_REGISTRY, slotToken(1), USER_REGISTRY_A)
  );

  // buildersdao (USER_REGISTRY_A, depth 2)
  handleLabelRegistered(
    createLabelRegisteredEvent(
      USER_REGISTRY_A,
      slotToken(1),
      Bytes.fromI32(3),
      "buildersdao"
    )
  );
  let buildersdaoPathId = pathNamehash(
    Bytes.fromHexString(basePathId),
    Bytes.fromI32(3)
  ).toHexString();
  assert.fieldEquals(
    "ENSv2NamePath",
    buildersdaoPathId,
    "name",
    "buildersdao.base.eth"
  );
  assert.fieldEquals("ENSv2NamePath", buildersdaoPathId, "depth", "2");
  assert.fieldEquals("ENSv2NamePath", buildersdaoPathId, "parent", basePathId);

  // ETHRegistry should have been classified ETH even though it was first
  // referenced via SubregistryUpdated (as a child), not its own event.
  let ethRegistryId = Address.fromString(ETH_REGISTRY).toHexString();
  assert.fieldEquals("ENSv2Registry", ethRegistryId, "kind", "ETH");
});

test("resolver set, change, clear, and re-registration reset reach every path of a shared slot without creating paths", () => {
  dataSourceMock.setNetwork("sepolia");

  const CHILD_REGISTRY = "0x222222222222222222222222222222222222222b";

  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(1),
      Bytes.fromI32(11),
      "one"
    )
  );
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(2),
      Bytes.fromI32(12),
      "two"
    )
  );

  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), CHILD_REGISTRY)
  );
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(2), CHILD_REGISTRY)
  );

  let childRegistryId = Address.fromString(CHILD_REGISTRY).toHexString();
  assert.fieldEquals("ENSv2Registry", childRegistryId, "namespaceCount", "2");

  handleLabelRegistered(
    createLabelRegisteredEvent(
      CHILD_REGISTRY,
      slotToken(1),
      Bytes.fromI32(20),
      "wallet"
    )
  );

  let onePathId = pathNamehash(
    Bytes.fromHexString(ROOT_NAMEHASH),
    Bytes.fromI32(11)
  ).toHexString();
  let twoPathId = pathNamehash(
    Bytes.fromHexString(ROOT_NAMEHASH),
    Bytes.fromI32(12)
  ).toHexString();
  let walletUnderOne = pathNamehash(
    Bytes.fromHexString(onePathId),
    Bytes.fromI32(20)
  ).toHexString();
  let walletUnderTwo = pathNamehash(
    Bytes.fromHexString(twoPathId),
    Bytes.fromI32(20)
  ).toHexString();

  assert.fieldEquals("ENSv2NamePath", walletUnderOne, "name", "wallet.one");
  assert.fieldEquals("ENSv2NamePath", walletUnderTwo, "name", "wallet.two");

  let walletSlotId = slotIdFor(CHILD_REGISTRY, 1);
  assert.fieldEquals("ENSv2NameSlot", walletSlotId, "pathCount", "2");

  const RESOLVER_A = "0x2323232323232323232323232323232323232323";
  const RESOLVER_B = "0x2424242424242424242424242424242424242424";
  handleResolverUpdated(
    createResolverUpdatedEvent(
      CHILD_REGISTRY,
      slotToken(1),
      RESOLVER_A
    )
  );
  let resolverA = Address.fromString(RESOLVER_A);
  assert.fieldEquals(
    "Domain",
    walletUnderOne,
    "resolver",
    createResolverID(Bytes.fromHexString(walletUnderOne), resolverA)
  );
  assert.fieldEquals(
    "Domain",
    walletUnderTwo,
    "resolver",
    createResolverID(Bytes.fromHexString(walletUnderTwo), resolverA)
  );
  assert.entityCount("ENSv2NamePath", 4);

  handleResolverUpdated(
    createResolverUpdatedEvent(CHILD_REGISTRY, slotToken(1), RESOLVER_B)
  );
  let resolverB = Address.fromString(RESOLVER_B);
  assert.fieldEquals(
    "Domain",
    walletUnderOne,
    "resolver",
    createResolverID(Bytes.fromHexString(walletUnderOne), resolverB)
  );
  assert.fieldEquals(
    "Domain",
    walletUnderTwo,
    "resolver",
    createResolverID(Bytes.fromHexString(walletUnderTwo), resolverB)
  );
  assert.entityCount("ENSv2NamePath", 4);

  handleResolverUpdated(
    createResolverUpdatedEvent(
      CHILD_REGISTRY,
      slotToken(1),
      "0x0000000000000000000000000000000000000000"
    )
  );
  assert.assertTrue(!Domain.load(walletUnderOne)!.resolver);
  assert.assertTrue(!Domain.load(walletUnderTwo)!.resolver);
  assert.entityCount("ENSv2NamePath", 4);

  // Reattach, then prove LabelUnregistered deletes both compatibility rows
  // (ENSv1 deletion convention) and the later LabelRegistered performs the
  // protocol's silent resolver reset across both materialized paths.
  handleResolverUpdated(
    createResolverUpdatedEvent(CHILD_REGISTRY, slotToken(1), RESOLVER_A)
  );
  handleLabelUnregistered(
    createLabelUnregisteredEvent(CHILD_REGISTRY, slotToken(1))
  );
  assert.assertTrue(!Domain.load(walletUnderOne)!.resolver);
  assert.assertTrue(!Domain.load(walletUnderTwo)!.resolver);
  assert.fieldEquals(
    "Domain",
    walletUnderOne,
    "owner",
    "0x0000000000000000000000000000000000000000"
  );
  assert.fieldEquals(
    "Domain",
    walletUnderTwo,
    "owner",
    "0x0000000000000000000000000000000000000000"
  );

  handleLabelRegistered(
    createLabelRegisteredEvent(
      CHILD_REGISTRY,
      slotToken(1),
      Bytes.fromI32(20),
      "wallet",
      OWNER_2
    )
  );
  assert.assertTrue(!Domain.load(walletUnderOne)!.resolver);
  assert.assertTrue(!Domain.load(walletUnderTwo)!.resolver);
  assert.fieldEquals("ENSv2NameSlot", walletSlotId, "pathCount", "2");
  assert.entityCount("ENSv2NamePath", 4);
});

test("late-link: each pre-existing child registration gets exactly one path", () => {
  dataSourceMock.setNetwork("sepolia");

  const LATE_CHILD_REGISTRY = "0x333333333333333333333333333333333333333c";

  // "wallet" registered in LATE_CHILD_REGISTRY before it is linked anywhere.
  handleLabelRegistered(
    createLabelRegisteredEvent(
      LATE_CHILD_REGISTRY,
      slotToken(1),
      Bytes.fromI32(30),
      "wallet"
    )
  );
  let walletSlotId = slotIdFor(LATE_CHILD_REGISTRY, 1);
  assert.fieldEquals("ENSv2NameSlot", walletSlotId, "pathCount", "0");

  // RootRegistry's own "late" registration is the only path materialised so
  // far (root always has an active root namespace) — snapshot that count.
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(1),
      Bytes.fromI32(31),
      "late"
    )
  );
  assert.entityCount("ENSv2SlotPathIndex", 1); // just "late" under root

  // Now late-link LATE_CHILD_REGISTRY under root's "late" slot.
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(
      ROOT_REGISTRY,
      slotToken(1),
      LATE_CHILD_REGISTRY
    )
  );

  let lateChildRegistryId = Address.fromString(
    LATE_CHILD_REGISTRY
  ).toHexString();
  assert.fieldEquals(
    "ENSv2Registry",
    lateChildRegistryId,
    "namespaceCount",
    "1"
  );

  // The link backfills one path for the pre-existing registration.
  assert.fieldEquals("ENSv2NameSlot", walletSlotId, "pathCount", "1");
  assert.entityCount("ENSv2SlotPathIndex", 2);

  // Re-emitting the same link does not backfill again.
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(
      ROOT_REGISTRY,
      slotToken(1),
      LATE_CHILD_REGISTRY
    )
  );
  assert.fieldEquals("ENSv2NameSlot", walletSlotId, "pathCount", "1");
  assert.entityCount("ENSv2SlotPathIndex", 2);
});

test("unregister -> re-register does not inflate pathCount", () => {
  dataSourceMock.setNetwork("sepolia");

  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(1),
      Bytes.fromI32(40),
      "reuse"
    )
  );
  let slotId = slotIdFor(ROOT_REGISTRY, 1);
  assert.fieldEquals("ENSv2NameSlot", slotId, "pathCount", "1");

  handleLabelUnregistered(
    createLabelUnregisteredEvent(ROOT_REGISTRY, slotToken(1))
  );
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(1),
      Bytes.fromI32(40),
      "reuse"
    )
  );

  assert.fieldEquals("ENSv2NameSlot", slotId, "pathCount", "1");
});

test("handleResolverUpdated creates ENSv2Resolver and links/clears slot.resolver", () => {
  dataSourceMock.setNetwork("sepolia");

  const CHILD_REGISTRY = "0x11111111111111111111111111111111111111bb";
  const RESOLVER_ADDRESS = "0x444444444444444444444444444444444444444d";
  const ZERO_ADDRESS = "0x0000000000000000000000000000000000000000";

  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(1),
      Bytes.fromI32(49),
      "parent"
    )
  );
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), CHILD_REGISTRY)
  );
  handleLabelRegistered(
    createLabelRegisteredEvent(
      CHILD_REGISTRY,
      slotToken(1),
      Bytes.fromI32(50),
      "res"
    )
  );
  let slotId = slotIdFor(CHILD_REGISTRY, 1);

  handleResolverUpdated(
    createResolverUpdatedEvent(CHILD_REGISTRY, slotToken(1), RESOLVER_ADDRESS)
  );

  let resolverId = Address.fromString(RESOLVER_ADDRESS).toHexString();
  let pathNode = pathNamehash(
    pathNamehash(Bytes.fromHexString(ROOT_NAMEHASH), Bytes.fromI32(49)),
    Bytes.fromI32(50)
  );
  let legacyResolverId = createResolverID(
    pathNode,
    Address.fromString(RESOLVER_ADDRESS)
  );
  assert.fieldEquals("ENSv2Resolver", resolverId, "address", resolverId);
  assert.fieldEquals("ENSv2NameSlot", slotId, "resolver", resolverId);
  assert.fieldEquals("ENSv2NameSlot", slotId, "resolverAddress", resolverId);
  assert.fieldEquals(
    "Domain",
    pathNode.toHexString(),
    "resolver",
    legacyResolverId
  );
  assert.fieldEquals(
    "Resolver",
    legacyResolverId,
    "domain",
    pathNode.toHexString()
  );

  // Clearing — slot is never deleted, only the resolver fields go null.
  handleResolverUpdated(
    createResolverUpdatedEvent(CHILD_REGISTRY, slotToken(1), ZERO_ADDRESS)
  );
  assert.assertTrue(slotExists(slotId));
  assert.assertTrue(!Domain.load(pathNode.toHexString())!.resolver);
  assert.assertTrue(!Domain.load(pathNode.toHexString())!.resolvedAddress);
});

test("link and record update before registry association snapshot correctly, and stale resolver fanout is ignored after a change", () => {
  dataSourceMock.setNetwork("sepolia");
  const CHILD_REGISTRY = "0x33333333333333333333333333333333333333bb";
  const RESOLVER_A = "0x4646464646464646464646464646464646464646";
  const RESOLVER_B = "0x4747474747474747474747474747474747474747";
  let tokenId = slotToken(1);
  let labelHash = Bytes.fromI32(52);
  let recordId = BigInt.fromI32(52);
  let initialContent = Bytes.fromUTF8("linked-before-association");

  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      tokenId,
      Bytes.fromI32(51),
      "parent"
    )
  );
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, tokenId, CHILD_REGISTRY)
  );
  handleLabelRegistered(
    createLabelRegisteredEvent(
      CHILD_REGISTRY,
      tokenId,
      labelHash,
      "ordered"
    )
  );
  let node = pathNamehash(
    pathNamehash(Bytes.fromHexString(ROOT_NAMEHASH), Bytes.fromI32(51)),
    labelHash
  );
  let resolverAId = createResolverID(node, Address.fromString(RESOLVER_A));
  let resolverBId = createResolverID(node, Address.fromString(RESOLVER_B));

  handleLinked(
    createLinkedEvent(
      RESOLVER_A,
      recordId,
      node,
      encodeLabel("ordered")
    )
  );
  handleContenthashUpdated(
    createContenthashUpdatedEvent(RESOLVER_A, recordId, initialContent)
  );
  assert.assertTrue(!Domain.load(node.toHexString())!.resolver);
  assert.notInStore("Resolver", resolverAId);

  // The registry association arrives last and must snapshot the already
  // populated native record immediately.
  handleResolverUpdated(
    createResolverUpdatedEvent(CHILD_REGISTRY, tokenId, RESOLVER_A)
  );
  assert.fieldEquals("Domain", node.toHexString(), "resolver", resolverAId);
  assert.bytesEquals(initialContent, Resolver.load(resolverAId)!.contentHash!);

  // Once the registry switches to B, later updates from A remain native-only
  // and cannot mutate either Domain.resolver or B's legacy snapshot.
  handleResolverUpdated(
    createResolverUpdatedEvent(CHILD_REGISTRY, tokenId, RESOLVER_B)
  );
  let staleContent = Bytes.fromUTF8("stale-resolver-update");
  handleContenthashUpdated(
    createContenthashUpdatedEvent(RESOLVER_A, recordId, staleContent)
  );
  let nativeRecordId = Address.fromString(RESOLVER_A)
    .toHexString()
    .concat(bigIntHex32(recordId));
  assert.fieldEquals(
    "ENSv2ResolverRecord",
    nativeRecordId,
    "contenthash",
    staleContent.toHexString()
  );
  assert.fieldEquals("Domain", node.toHexString(), "resolver", resolverBId);
  assert.bytesEquals(initialContent, Resolver.load(resolverAId)!.contentHash!);
  assert.assertTrue(!Resolver.load(resolverBId)!.contentHash);
});

test("a classic PublicResolverV2 event updates the Resolver row attached by ENSv2 ResolverUpdated", () => {
  dataSourceMock.setNetwork("sepolia");
  const CHILD_REGISTRY = "0x55555555555555555555555555555555555555bb";
  const RESOLVER_ADDRESS = "0x4545454545454545454545454545454545454545";
  const RESOLVED_ADDRESS = "0x5656565656565656565656565656565656565656";

  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(1),
      Bytes.fromI32(50),
      "parent"
    )
  );
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), CHILD_REGISTRY)
  );
  handleLabelRegistered(
    createLabelRegisteredEvent(
      CHILD_REGISTRY,
      slotToken(1),
      Bytes.fromI32(51),
      "classic"
    )
  );
  let node = pathNamehash(
    pathNamehash(Bytes.fromHexString(ROOT_NAMEHASH), Bytes.fromI32(50)),
    Bytes.fromI32(51)
  );
  handleResolverUpdated(
    createResolverUpdatedEvent(CHILD_REGISTRY, slotToken(1), RESOLVER_ADDRESS)
  );

  let mockEvent = newMockEvent();
  let event = new AddrChanged(
    Address.fromString(RESOLVER_ADDRESS),
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
      ethereum.Value.fromAddress(Address.fromString(RESOLVED_ADDRESS))
    )
  );
  handleAddrChanged(event);

  assert.fieldEquals(
    "Domain",
    node.toHexString(),
    "resolvedAddress",
    Address.fromString(RESOLVED_ADDRESS).toHexString()
  );
});

test("SubregistryUpdated(..., address(0)) clears the link and deactivates (not deletes) the namespace it produced", () => {
  dataSourceMock.setNetwork("sepolia");

  const CLEAR_CHILD_REGISTRY = "0x555555555555555555555555555555555555555e";
  const ZERO_ADDRESS = "0x0000000000000000000000000000000000000000";

  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(1),
      Bytes.fromI32(60),
      "clearme"
    )
  );
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(
      ROOT_REGISTRY,
      slotToken(1),
      CLEAR_CHILD_REGISTRY
    )
  );

  let clearPathId = pathNamehash(
    Bytes.fromHexString(ROOT_NAMEHASH),
    Bytes.fromI32(60)
  ).toHexString();
  let namespaceEntityId = namespaceId(
    Address.fromString(CLEAR_CHILD_REGISTRY),
    Bytes.fromHexString(clearPathId)
  ).toHexString();
  assert.fieldEquals("ENSv2Namespace", namespaceEntityId, "active", "true");

  let rootSlotId = slotIdFor(ROOT_REGISTRY, 1);

  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), ZERO_ADDRESS)
  );

  assert.assertTrue(slotExists(rootSlotId));
  assert.fieldEquals("ENSv2Namespace", namespaceEntityId, "active", "false");
});

test("deactivating a namespace also deactivates the paths materialised under it", () => {
  dataSourceMock.setNetwork("sepolia");

  const CHILD_REGISTRY = "0x666666666666666666666666666666666666666f";
  const ZERO_ADDRESS = "0x0000000000000000000000000000000000000000";

  // Link CHILD_REGISTRY under the root at "linked", creating a namespace
  // for it, then register a name directly IN that child registry -- this
  // is the path that must go inactive once the namespace does.
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(1),
      Bytes.fromI32(70),
      "linked"
    )
  );
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), CHILD_REGISTRY)
  );
  handleLabelRegistered(
    createLabelRegisteredEvent(
      CHILD_REGISTRY,
      slotToken(1),
      Bytes.fromI32(80),
      "leaf"
    )
  );

  let linkedPathId = pathNamehash(
    Bytes.fromHexString(ROOT_NAMEHASH),
    Bytes.fromI32(70)
  ).toHexString();
  let leafPathId = pathNamehash(
    Bytes.fromHexString(linkedPathId),
    Bytes.fromI32(80)
  ).toHexString();
  assert.fieldEquals("ENSv2NamePath", leafPathId, "active", "true");

  // Clear the link -- deactivates the namespace, which must now cascade to
  // "leaf"'s path (materialised inside CHILD_REGISTRY under that namespace).
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), ZERO_ADDRESS)
  );

  assert.fieldEquals("ENSv2NamePath", leafPathId, "active", "false");
});

test("deactivating a namespace deactivates every materialised path, not just the first", () => {
  dataSourceMock.setNetwork("sepolia");

  const CHILD_REGISTRY = "0x777777777777777777777777777777777777778a";
  const ZERO_ADDRESS = "0x0000000000000000000000000000000000000000";

  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(1),
      Bytes.fromI32(90),
      "multilinked"
    )
  );
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), CHILD_REGISTRY)
  );
  // Two separate names registered in the same child registry -- two
  // distinct paths materialised under the SAME namespace
  // (namespace.pathCount == 2), unlike the single-path test above.
  handleLabelRegistered(
    createLabelRegisteredEvent(
      CHILD_REGISTRY,
      slotToken(1),
      Bytes.fromI32(91),
      "leafone"
    )
  );
  handleLabelRegistered(
    createLabelRegisteredEvent(
      CHILD_REGISTRY,
      slotToken(2),
      Bytes.fromI32(92),
      "leaftwo"
    )
  );

  let linkedPathId = pathNamehash(
    Bytes.fromHexString(ROOT_NAMEHASH),
    Bytes.fromI32(90)
  ).toHexString();
  let leafOnePathId = pathNamehash(
    Bytes.fromHexString(linkedPathId),
    Bytes.fromI32(91)
  ).toHexString();
  let leafTwoPathId = pathNamehash(
    Bytes.fromHexString(linkedPathId),
    Bytes.fromI32(92)
  ).toHexString();
  assert.fieldEquals("ENSv2NamePath", leafOnePathId, "active", "true");
  assert.fieldEquals("ENSv2NamePath", leafTwoPathId, "active", "true");

  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), ZERO_ADDRESS)
  );

  // The loop over namespace.pathCount must reach BOTH indexed paths, not
  // just index 0.
  assert.fieldEquals("ENSv2NamePath", leafOnePathId, "active", "false");
  assert.fieldEquals("ENSv2NamePath", leafTwoPathId, "active", "false");
});

test("fresh non-.eth registration projects a Domain while its root parent stays native-only", () => {
  dataSourceMock.setNetwork("sepolia");

  const CHILD_REGISTRY = "0x666666666666666666666666666666666666666f";

  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(1),
      Bytes.fromI32(70),
      "alpha"
    )
  );
  let alphaPathId = pathNamehash(
    Bytes.fromHexString(ROOT_NAMEHASH),
    Bytes.fromI32(70)
  ).toHexString();
  assert.fieldEquals("ENSv2NamePath", alphaPathId, "name", "alpha");
  assert.notInStore("Domain", alphaPathId);

  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), CHILD_REGISTRY)
  );
  handleLabelRegistered(
    createLabelRegisteredEvent(
      CHILD_REGISTRY,
      slotToken(1),
      Bytes.fromI32(71),
      "beta"
    )
  );
  let betaPathId = pathNamehash(
    Bytes.fromHexString(alphaPathId),
    Bytes.fromI32(71)
  ).toHexString();

  assert.fieldEquals("Domain", betaPathId, "name", "beta.alpha");
  // Non-.eth: no legacy Registration row, and no grace-period addition —
  // Domain.expiryDate is the slot's raw expiry.
  assert.fieldEquals("Domain", betaPathId, "expiryDate", "2000000000");
  let betaDomain = Domain.load(betaPathId);
  let hasParent = false;
  if (betaDomain != null && betaDomain.parent) {
    hasParent = true;
  }
  assert.assertTrue(!hasParent);
  assert.notInStore("Registration", Bytes.fromI32(71).toHexString());
});

test("fresh .eth registration produces Domain + Registration sharing the legacy labelhash ID, with the grace-period split applied only to Domain", () => {
  dataSourceMock.setNetwork("sepolia");

  // ETHRegistry must first be linked under root's "eth" slot — its own
  // namespaceCount is 0 (and materializePathsForSlot a no-op) until then.
  let ethLabelHash = Bytes.fromByteArray(
    crypto.keccak256(ByteArray.fromUTF8("eth"))
  );
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(1),
      ethLabelHash,
      "eth"
    )
  );
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), ETH_REGISTRY)
  );
  let ethPathId = pathNamehash(
    Bytes.fromHexString(ROOT_NAMEHASH),
    ethLabelHash
  ).toHexString();

  // RootRegistry's native eth path must remain separate from the existing
  // ENSv1 Domain row, but projected .eth children should still use that row
  // as their legacy parent.
  let legacyEthOwner = Address.fromString(OWNER_2).toHexString();
  let legacyEthRegistrant = Address.fromString(OWNER).toHexString();
  let legacyEth = new Domain(ethPathId);
  legacyEth.storedOffchain = false; // Blockscout/BENS extension
  legacyEth.resolvedWithWildcard = false; // Blockscout/BENS extension
  legacyEth.name = "eth";
  legacyEth.owner = legacyEthOwner;
  legacyEth.registrant = legacyEthRegistrant;
  legacyEth.expiryDate = BigInt.fromI32(1234567890);
  legacyEth.isMigrated = false;
  legacyEth.subdomainCount = 7;
  legacyEth.createdAt = BigInt.fromI32(1);
  legacyEth.save();

  let labelHash = Bytes.fromI32(80);
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ETH_REGISTRY,
      slotToken(1),
      labelHash,
      "vitalik"
    )
  );

  let pathId = pathNamehash(Bytes.fromHexString(ethPathId), labelHash)
    .toHexString();
  let registrationId = labelHash.toHexString();

  assert.fieldEquals("Domain", pathId, "name", "vitalik.eth");
  assert.fieldEquals("Registration", registrationId, "domain", pathId);
  assert.fieldEquals(
    "Registration",
    registrationId,
    "expiryDate",
    "2000000000"
  );
  assert.fieldEquals("Domain", pathId, "parent", ethPathId);
  assert.fieldEquals("Domain", ethPathId, "subdomainCount", "8");
  assert.fieldEquals("Domain", ethPathId, "owner", legacyEthOwner);
  assert.fieldEquals("Domain", ethPathId, "registrant", legacyEthRegistrant);
  assert.fieldEquals("Domain", ethPathId, "expiryDate", "1234567890");
  assert.fieldEquals("Domain", ethPathId, "isMigrated", "false");

  // Re-registration reuses the projected Domain row and must not count the
  // same direct child again.
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ETH_REGISTRY,
      slotToken(1),
      labelHash,
      "vitalik",
      OWNER_2
    )
  );
  assert.fieldEquals("Domain", ethPathId, "subdomainCount", "8");

  // v2GracePeriod is 2,419,200s (28 days) per src/ensv2Constants.ts.
  assert.fieldEquals("Domain", pathId, "expiryDate", "2002419200");
});

test("an ETHRegistry slot with several paths keeps legacy Registration.domain on the canonical .eth Domain", () => {
  dataSourceMock.setNetwork("sepolia");

  let ethLabelHash = Bytes.fromByteArray(
    crypto.keccak256(ByteArray.fromUTF8("eth"))
  );
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(1),
      ethLabelHash,
      "eth"
    )
  );
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), ETH_REGISTRY)
  );

  // Link the same ETHRegistry under two additional namespaces before the
  // label is registered. All three paths should receive compatibility
  // Domains, but only <label>.eth is the legacy Registration's domain.
  let aliasOneHash = Bytes.fromI32(81);
  let aliasTwoHash = Bytes.fromI32(82);
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(2),
      aliasOneHash,
      "alias-one"
    )
  );
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(2), ETH_REGISTRY)
  );
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(3),
      aliasTwoHash,
      "alias-two"
    )
  );
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(3), ETH_REGISTRY)
  );

  let labelHash = Bytes.fromI32(83);
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ETH_REGISTRY,
      slotToken(4),
      labelHash,
      "multipath"
    )
  );

  let canonicalPathId = pathNamehash(
    pathNamehash(Bytes.fromHexString(ROOT_NAMEHASH), ethLabelHash),
    labelHash
  ).toHexString();
  let aliasOnePathId = pathNamehash(
    pathNamehash(Bytes.fromHexString(ROOT_NAMEHASH), aliasOneHash),
    labelHash
  ).toHexString();
  let aliasTwoPathId = pathNamehash(
    pathNamehash(Bytes.fromHexString(ROOT_NAMEHASH), aliasTwoHash),
    labelHash
  ).toHexString();

  assert.fieldEquals("Domain", canonicalPathId, "name", "multipath.eth");
  assert.fieldEquals(
    "Domain",
    aliasOnePathId,
    "name",
    "multipath.alias-one"
  );
  assert.fieldEquals(
    "Domain",
    aliasTwoPathId,
    "name",
    "multipath.alias-two"
  );
  assert.fieldEquals(
    "Registration",
    labelHash.toHexString(),
    "domain",
    canonicalPathId
  );
});

test("a late-linked registration is projected into Domain, with its resolver", () => {
  dataSourceMock.setNetwork("sepolia");

  const LATE_CHILD_REGISTRY = "0x777777777777777777777777777777777777777a";
  const RESOLVER_ADDRESS = "0x999999999999999999999999999999999999999c";

  handleLabelRegistered(
    createLabelRegisteredEvent(
      LATE_CHILD_REGISTRY,
      slotToken(1),
      Bytes.fromI32(90),
      "orphan"
    )
  );
  // Resolver set while the registry is not linked anywhere yet.
  handleResolverUpdated(
    createResolverUpdatedEvent(
      LATE_CHILD_REGISTRY,
      slotToken(1),
      RESOLVER_ADDRESS
    )
  );
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(1),
      Bytes.fromI32(91),
      "late2"
    )
  );
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(
      ROOT_REGISTRY,
      slotToken(1),
      LATE_CHILD_REGISTRY
    )
  );

  let late2PathId = pathNamehash(
    Bytes.fromHexString(ROOT_NAMEHASH),
    Bytes.fromI32(91)
  ).toHexString();
  let orphanPathId = pathNamehash(
    Bytes.fromHexString(late2PathId),
    Bytes.fromI32(90)
  ).toHexString();

  assert.fieldEquals("Domain", orphanPathId, "name", "orphan.late2");
  assert.fieldEquals("Domain", orphanPathId, "owner", OWNER.toLowerCase());
  let resolverId = createResolverID(
    Bytes.fromHexString(orphanPathId),
    Address.fromString(RESOLVER_ADDRESS)
  );
  assert.fieldEquals("Domain", orphanPathId, "resolver", resolverId);
  assert.fieldEquals("Resolver", resolverId, "domain", orphanPathId);
  // The RootRegistry parent stays native-only.
  assert.notInStore("Domain", late2PathId);
});

test("RootRegistry re-registration reactivates the native path without a legacy Domain", () => {
  dataSourceMock.setNetwork("sepolia");

  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(1),
      Bytes.fromI32(100),
      "changeling",
      OWNER
    )
  );
  let pathId = pathNamehash(
    Bytes.fromHexString(ROOT_NAMEHASH),
    Bytes.fromI32(100)
  ).toHexString();
  assert.notInStore("Domain", pathId);

  const RESOLVER_ADDRESS = "0x888888888888888888888888888888888888888b";
  handleResolverUpdated(
    createResolverUpdatedEvent(ROOT_REGISTRY, slotToken(1), RESOLVER_ADDRESS)
  );
  assert.notInStore("Domain", pathId);

  handleLabelUnregistered(
    createLabelUnregisteredEvent(ROOT_REGISTRY, slotToken(1))
  );
  assert.notInStore("Domain", pathId);
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(1),
      Bytes.fromI32(100),
      "changeling",
      OWNER_2
    )
  );

  assert.notInStore("Domain", pathId);
  assert.fieldEquals("ENSv2NamePath", pathId, "active", "true");
  assert.entityCount("ENSv2NamePath", 1);
});

test("unregister decrements and re-registration restores the legacy parent's subdomainCount, without double counting", () => {
  dataSourceMock.setNetwork("sepolia");

  let ethLabelHash = Bytes.fromByteArray(
    crypto.keccak256(ByteArray.fromUTF8("eth"))
  );
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(1),
      ethLabelHash,
      "eth"
    )
  );
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), ETH_REGISTRY)
  );
  let ethPathId = pathNamehash(
    Bytes.fromHexString(ROOT_NAMEHASH),
    ethLabelHash
  ).toHexString();

  let legacyEth = new Domain(ethPathId);
  legacyEth.storedOffchain = false; // Blockscout/BENS extension
  legacyEth.resolvedWithWildcard = false; // Blockscout/BENS extension
  legacyEth.name = "eth";
  legacyEth.owner = Address.fromString(OWNER_2).toHexString();
  legacyEth.isMigrated = false;
  legacyEth.subdomainCount = 3;
  legacyEth.createdAt = BigInt.fromI32(1);
  legacyEth.save();

  let labelHash = Bytes.fromI32(81);
  let pathId = pathNamehash(Bytes.fromHexString(ethPathId), labelHash).toHexString();

  // Fresh registration: new projected Domain counts once.
  handleLabelRegistered(
    createLabelRegisteredEvent(ETH_REGISTRY, slotToken(1), labelHash, "cycle")
  );
  assert.fieldEquals("Domain", pathId, "parent", ethPathId);
  assert.fieldEquals("Domain", ethPathId, "subdomainCount", "4");

  // Unregister: the child is no longer live, so it stops counting.
  handleLabelUnregistered(
    createLabelUnregisteredEvent(ETH_REGISTRY, slotToken(1))
  );
  assert.fieldEquals("ENSv2NamePath", pathId, "active", "false");
  assert.fieldEquals("Domain", ethPathId, "subdomainCount", "3");

  // A second unregister of the same (already inactive) slot is a no-op.
  handleLabelUnregistered(
    createLabelUnregisteredEvent(ETH_REGISTRY, slotToken(1))
  );
  assert.fieldEquals("Domain", ethPathId, "subdomainCount", "3");

  // Re-registration reuses the Domain row and restores the count exactly once.
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ETH_REGISTRY,
      slotToken(1),
      labelHash,
      "cycle",
      OWNER_2
    )
  );
  assert.fieldEquals("ENSv2NamePath", pathId, "active", "true");
  assert.fieldEquals("Domain", ethPathId, "subdomainCount", "4");

  // Unregistering "eth" itself in the RootRegistry makes every name below
  // it unreachable (getSubregistry returns zero for an unregistered name):
  // the projected child is deactivated and stops counting. The root-level
  // "eth" path has no projected Domain of its own.
  handleLabelUnregistered(
    createLabelUnregisteredEvent(ROOT_REGISTRY, slotToken(1))
  );
  assert.fieldEquals("ENSv2NamePath", pathId, "active", "false");
  assert.fieldEquals("Domain", ethPathId, "subdomainCount", "3");
});

test("never-expiring ENSv2 slots (uint64 max) clamp legacy Domain/Registration expiry to the max supported timestamp, native slot keeps the raw value", () => {
  dataSourceMock.setNetwork("sepolia");

  const UINT64_MAX = BigInt.fromString("18446744073709551615");
  const MAX_LEGACY = "253402300799";

  let ethLabelHash = Bytes.fromByteArray(
    crypto.keccak256(ByteArray.fromUTF8("eth"))
  );
  handleLabelRegistered(
    createLabelRegisteredEvent(ROOT_REGISTRY, slotToken(1), ethLabelHash, "eth")
  );
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), ETH_REGISTRY)
  );
  let ethPathId = pathNamehash(
    Bytes.fromHexString(ROOT_NAMEHASH),
    ethLabelHash
  ).toHexString();

  // Registration path: uint64 max + grace must not overflow or leak through.
  let foreverHash = Bytes.fromI32(82);
  let foreverPathId = pathNamehash(
    Bytes.fromHexString(ethPathId),
    foreverHash
  ).toHexString();
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ETH_REGISTRY,
      slotToken(1),
      foreverHash,
      "forever",
      OWNER,
      UINT64_MAX
    )
  );
  assert.fieldEquals("Domain", foreverPathId, "expiryDate", MAX_LEGACY);
  assert.fieldEquals("Registration", foreverHash.toHexString(), "expiryDate", MAX_LEGACY);
  assert.fieldEquals(
    "ENSv2NameSlot",
    slotIdFor(ETH_REGISTRY, 1),
    "expiryDate",
    UINT64_MAX.toString()
  );

  // Renewal path: an ordinary name extended to "never".
  let laterHash = Bytes.fromI32(83);
  let laterPathId = pathNamehash(
    Bytes.fromHexString(ethPathId),
    laterHash
  ).toHexString();
  handleLabelRegistered(
    createLabelRegisteredEvent(ETH_REGISTRY, slotToken(2), laterHash, "later")
  );
  assert.fieldEquals("Domain", laterPathId, "expiryDate", "2002419200");
  handleExpiryUpdated(
    createExpiryUpdatedEvent(ETH_REGISTRY, slotToken(2), UINT64_MAX)
  );
  assert.fieldEquals("Domain", laterPathId, "expiryDate", MAX_LEGACY);
  assert.fieldEquals("Registration", laterHash.toHexString(), "expiryDate", MAX_LEGACY);
  assert.fieldEquals(
    "ENSv2NameSlot",
    slotIdFor(ETH_REGISTRY, 2),
    "expiryDate",
    UINT64_MAX.toString()
  );

  // Values below the cap are untouched.
  handleExpiryUpdated(
    createExpiryUpdatedEvent(ETH_REGISTRY, slotToken(2), BigInt.fromI32(2100000000))
  );
  assert.fieldEquals("Domain", laterPathId, "expiryDate", "2102419200");
});

const createAddressUpdatedEvent = (
  resolverAddress: string,
  recordId: BigInt,
  coinType: BigInt,
  addressBytes: Bytes
): AddressUpdated => {
  let mockEvent = newMockEvent();
  let event = new AddressUpdated(
    Address.fromString(resolverAddress),
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

const createAddrChangedEvent = (
  resolverAddress: string,
  node: Bytes,
  a: string
): AddrChanged => {
  let mockEvent = newMockEvent();
  let event = new AddrChanged(
    Address.fromString(resolverAddress),
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
      ethereum.Value.fromAddress(Address.fromString(a))
    )
  );
  return event;
};

// Registers parent/<label> under a fresh child registry and returns the
// namehash of the child name. Shared by the attachDomainResolver tests below.
function registerChildName(
  childRegistry: string,
  parentLabelHash: Bytes,
  childLabelHash: Bytes
): Bytes {
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(1),
      parentLabelHash,
      "parent"
    )
  );
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), childRegistry)
  );
  handleLabelRegistered(
    createLabelRegisteredEvent(childRegistry, slotToken(1), childLabelHash, "child")
  );
  return pathNamehash(
    pathNamehash(Bytes.fromHexString(ROOT_NAMEHASH), parentLabelHash),
    childLabelHash
  );
}

// A classic node-keyed resolver (PublicResolverV2 etc.) has no
// ENSv2ResolverLink: its records live on the legacy Resolver row, written by
// the ENSv1 resolver handlers, and can arrive before the registry points the
// name at it (as AddrChanged before NewResolver does under ENSv1).
// attachDomainResolver must then behave like handleNewResolver: keep the
// snapshot and copy addr to Domain.resolvedAddress, not wipe it.
test("ENSv2 ResolverUpdated to a classic resolver with pre-existing records keeps them and sets resolvedAddress", () => {
  dataSourceMock.setNetwork("sepolia");
  const CHILD_REGISTRY = "0x66666666666666666666666666666666666666bb";
  const CLASSIC_RESOLVER = "0x4848484848484848484848484848484848484848";
  const RESOLVED = "0x5959595959595959595959595959595959595959";

  let node = registerChildName(CHILD_REGISTRY, Bytes.fromI32(60), Bytes.fromI32(61));
  let legacyId = createResolverID(node, Address.fromString(CLASSIC_RESOLVER));

  // Records first: the Resolver row exists with addr, but the Domain is not
  // yet pointed at it so resolvedAddress stays unset.
  handleAddrChanged(createAddrChangedEvent(CLASSIC_RESOLVER, node, RESOLVED));
  assert.fieldEquals(
    "Resolver",
    legacyId,
    "addr",
    Address.fromString(RESOLVED).toHexString()
  );
  assert.assertTrue(!Domain.load(node.toHexString())!.resolver);
  assert.assertTrue(!Domain.load(node.toHexString())!.resolvedAddress);

  // Then the ENSv2 registry association.
  handleResolverUpdated(
    createResolverUpdatedEvent(CHILD_REGISTRY, slotToken(1), CLASSIC_RESOLVER)
  );
  assert.fieldEquals("Domain", node.toHexString(), "resolver", legacyId);
  assert.fieldEquals(
    "Resolver",
    legacyId,
    "addr",
    Address.fromString(RESOLVED).toHexString()
  );
  assert.fieldEquals(
    "Domain",
    node.toHexString(),
    "resolvedAddress",
    Address.fromString(RESOLVED).toHexString()
  );
});

// linkToRecord(name, 0) leaves an explicitly inactive ENSv2ResolverLink. If
// the registry then (re)points the name at that resolver, the native side
// says there is no record, so a stale legacy snapshot must be cleared. This
// is the one case attachDomainResolver still clears.
test("ENSv2 ResolverUpdated to a PermissionedResolver whose link is inactive clears the legacy snapshot", () => {
  dataSourceMock.setNetwork("sepolia");
  const CHILD_REGISTRY = "0x67676767676767676767676767676767676767bb";
  const RESOLVER_ADDRESS = "0x4949494949494949494949494949494949494949";
  const STALE = "0x5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a";

  let node = registerChildName(CHILD_REGISTRY, Bytes.fromI32(62), Bytes.fromI32(63));
  let legacyId = createResolverID(node, Address.fromString(RESOLVER_ADDRESS));

  // Explicit unlink: recordId 0.
  handleLinked(
    createLinkedEvent(RESOLVER_ADDRESS, BigInt.zero(), node, encodeLabel("child"))
  );
  assert.fieldEquals(
    "ENSv2ResolverLink",
    resolverLinkId(Address.fromString(RESOLVER_ADDRESS), node).toHexString(),
    "active",
    "false"
  );

  // A stale legacy snapshot that must not survive the association.
  let stale = new Resolver(legacyId);
  stale.domain = node.toHexString();
  stale.address = Address.fromString(RESOLVER_ADDRESS);
  stale.addr = Address.fromString(STALE).toHexString();
  stale.texts = ["avatar"];
  stale.save();

  handleResolverUpdated(
    createResolverUpdatedEvent(CHILD_REGISTRY, slotToken(1), RESOLVER_ADDRESS)
  );
  assert.fieldEquals("Domain", node.toHexString(), "resolver", legacyId);
  let cleared = Resolver.load(legacyId)!;
  assert.assertTrue(!cleared.addr);
  assert.assertTrue(!cleared.texts);
  assert.assertTrue(!Domain.load(node.toHexString())!.resolvedAddress);
});

// An active link projects the native record (ETH address, coinTypes) onto
// the legacy Resolver and Domain.resolvedAddress at association time.
test("ENSv2 ResolverUpdated to a PermissionedResolver whose link is active projects the native record", () => {
  dataSourceMock.setNetwork("sepolia");
  const CHILD_REGISTRY = "0x68686868686868686868686868686868686868bb";
  const RESOLVER_ADDRESS = "0x4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a";
  const RESOLVED = "0x5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b";
  let recordId = BigInt.fromI32(64);

  let node = registerChildName(CHILD_REGISTRY, Bytes.fromI32(65), Bytes.fromI32(66));
  let legacyId = createResolverID(node, Address.fromString(RESOLVER_ADDRESS));

  handleLinked(
    createLinkedEvent(RESOLVER_ADDRESS, recordId, node, encodeLabel("child"))
  );
  handleAddressUpdated(
    createAddressUpdatedEvent(
      RESOLVER_ADDRESS,
      recordId,
      BigInt.fromI32(60),
      Bytes.fromHexString(RESOLVED)
    )
  );
  // Not associated yet: nothing projected.
  assert.notInStore("Resolver", legacyId);

  handleResolverUpdated(
    createResolverUpdatedEvent(CHILD_REGISTRY, slotToken(1), RESOLVER_ADDRESS)
  );
  assert.fieldEquals("Domain", node.toHexString(), "resolver", legacyId);
  assert.fieldEquals(
    "Resolver",
    legacyId,
    "addr",
    Address.fromString(RESOLVED).toHexString()
  );
  assert.fieldEquals("Resolver", legacyId, "coinTypes", "[60]");
  assert.fieldEquals(
    "Domain",
    node.toHexString(),
    "resolvedAddress",
    Address.fromString(RESOLVED).toHexString()
  );
});

// --- v2.3.2: late-link backfill (subname copy) and unregister clearing ---

const EMPTY = "0x0000000000000000000000000000000000000000";

function rootPath(labelHash: Bytes): Bytes {
  return pathNamehash(Bytes.fromHexString(ROOT_NAMEHASH), labelHash);
}

test("subname copy: names registered in a new registry before the parent switches to it take over their paths", () => {
  dataSourceMock.setNetwork("sepolia");
  const OLD_REGISTRY = "0xa1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
  const NEW_REGISTRY = "0xb1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1";
  const RESOLVER_ADDRESS = "0x999999999999999999999999999999999999999d";

  let baseHash = Bytes.fromI32(500);
  let supportHash = Bytes.fromI32(501);
  let legacyHash = Bytes.fromI32(502);
  handleLabelRegistered(createLabelRegisteredEvent(ROOT_REGISTRY, slotToken(1), baseHash, "base"));
  handleSubregistryUpdated(createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), OLD_REGISTRY));
  handleLabelRegistered(createLabelRegisteredEvent(OLD_REGISTRY, slotToken(1), supportHash, "support"));
  handleLabelRegistered(createLabelRegisteredEvent(OLD_REGISTRY, slotToken(2), legacyHash, "legacy"));

  let supportId = pathNamehash(rootPath(baseHash), supportHash).toHexString();
  let legacyId = pathNamehash(rootPath(baseHash), legacyHash).toHexString();
  assert.fieldEquals("Domain", supportId, "owner", OWNER.toLowerCase());
  assert.fieldEquals("Domain", legacyId, "owner", OWNER.toLowerCase());

  // The copy: deploy NEW, register the name there (with its resolver), and
  // only then point the parent at NEW. "legacy" is not copied.
  handleLabelRegistered(
    createLabelRegisteredEvent(NEW_REGISTRY, slotToken(1), supportHash, "support", OWNER_2)
  );
  handleResolverUpdated(createResolverUpdatedEvent(NEW_REGISTRY, slotToken(1), RESOLVER_ADDRESS));
  handleSubregistryUpdated(createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), NEW_REGISTRY));

  assert.fieldEquals("Domain", supportId, "name", "support.base");
  assert.fieldEquals("Domain", supportId, "owner", OWNER_2.toLowerCase());
  assert.fieldEquals(
    "Domain",
    supportId,
    "resolver",
    createResolverID(Bytes.fromHexString(supportId), Address.fromString(RESOLVER_ADDRESS))
  );
  assert.fieldEquals("ENSv2NamePath", supportId, "slot", slotIdFor(NEW_REGISTRY, 1));
  assert.fieldEquals("ENSv2NamePath", supportId, "active", "true");
  // The name left behind in OLD is no longer reachable: deleted.
  assert.fieldEquals("Domain", legacyId, "owner", EMPTY);
  assert.fieldEquals("ENSv2NamePath", legacyId, "active", "false");
});

test("subname copy: nested registries linked before the top-level switch are backfilled too", () => {
  dataSourceMock.setNetwork("sepolia");
  const NEW_REGISTRY = "0xb2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2";
  const NEW_CHILD_REGISTRY = "0xc2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2";

  let baseHash = Bytes.fromI32(510);
  let aHash = Bytes.fromI32(511);
  let bHash = Bytes.fromI32(512);
  // a.<NEW> -> NEW_CHILD, and b registered in NEW_CHILD, all before any link.
  handleLabelRegistered(createLabelRegisteredEvent(NEW_REGISTRY, slotToken(1), aHash, "a"));
  handleSubregistryUpdated(createSubregistryUpdatedEvent(NEW_REGISTRY, slotToken(1), NEW_CHILD_REGISTRY));
  handleLabelRegistered(createLabelRegisteredEvent(NEW_CHILD_REGISTRY, slotToken(1), bHash, "b"));

  handleLabelRegistered(createLabelRegisteredEvent(ROOT_REGISTRY, slotToken(1), baseHash, "base"));
  handleSubregistryUpdated(createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), NEW_REGISTRY));

  let aId = pathNamehash(rootPath(baseHash), aHash);
  let bId = pathNamehash(aId, bHash).toHexString();
  assert.fieldEquals("Domain", aId.toHexString(), "name", "a.base");
  assert.fieldEquals("Domain", bId, "name", "b.a.base");
  assert.fieldEquals("Domain", bId, "parent", aId.toHexString());
  assert.fieldEquals("Domain", aId.toHexString(), "subdomainCount", "1");

  // A registration in NEW_CHILD after the link is projected as before.
  let cHash = Bytes.fromI32(513);
  handleLabelRegistered(createLabelRegisteredEvent(NEW_CHILD_REGISTRY, slotToken(2), cHash, "c"));
  assert.fieldEquals("Domain", pathNamehash(aId, cHash).toHexString(), "name", "c.a.base");
  assert.fieldEquals("Domain", aId.toHexString(), "subdomainCount", "2");
});

function setupUnregisterScenario(parentRegistry: string, childRegistry: string): Bytes {
  let baseHash = Bytes.fromI32(520);
  let xHash = Bytes.fromI32(521);
  let yHash = Bytes.fromI32(522);
  handleLabelRegistered(createLabelRegisteredEvent(ROOT_REGISTRY, slotToken(1), baseHash, "base"));
  handleSubregistryUpdated(createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), parentRegistry));
  handleLabelRegistered(createLabelRegisteredEvent(parentRegistry, slotToken(1), xHash, "x"));
  handleResolverUpdated(
    createResolverUpdatedEvent(parentRegistry, slotToken(1), "0x999999999999999999999999999999999999999e")
  );
  handleSubregistryUpdated(createSubregistryUpdatedEvent(parentRegistry, slotToken(1), childRegistry));
  handleLabelRegistered(createLabelRegisteredEvent(childRegistry, slotToken(1), yHash, "y"));
  let xPath = pathNamehash(rootPath(baseHash), xHash);
  assert.fieldEquals("Domain", pathNamehash(xPath, yHash).toHexString(), "owner", OWNER.toLowerCase());
  handleLabelUnregistered(createLabelUnregisteredEvent(parentRegistry, slotToken(1)));
  return xPath;
}

test("unregister on a registry with the fixed UserRegistry implementation clears resolver and subregistry and unlinks the child registry", () => {
  dataSourceMock.setNetwork("sepolia");
  const PARENT = "0xd1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1";
  const CHILD = "0xd2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2";
  let parent = Address.fromString(PARENT);
  getOrCreateRegistry(
    parent,
    parent,
    newMockEvent().block,
    Address.fromString("0x0822d199699f0a9D378617817a1439663234a126")
  );

  let xPath = setupUnregisterScenario(PARENT, CHILD);
  let xSlot = slotIdFor(PARENT, 1);
  assert.fieldEquals("ENSv2NameSlot", xSlot, "subregistry", "null");
  assert.fieldEquals("ENSv2NameSlot", xSlot, "resolver", "null");
  let ns = namespaceId(Address.fromString(CHILD), xPath).toHexString();
  assert.fieldEquals("ENSv2Namespace", ns, "active", "false");
  let yId = pathNamehash(xPath, Bytes.fromI32(522)).toHexString();
  assert.fieldEquals("ENSv2NamePath", yId, "active", "false");
  assert.fieldEquals("Domain", yId, "owner", EMPTY);
});

test("unregister on an older registry implementation keeps resolver and subregistry, but the names below stop resolving", () => {
  dataSourceMock.setNetwork("sepolia");
  const PARENT = "0xe1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1";
  const CHILD = "0xe2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2";

  let xPath = setupUnregisterScenario(PARENT, CHILD);
  let xSlot = slotIdFor(PARENT, 1);
  // Stored on chain (and restored by a revive) ...
  assert.fieldEquals("ENSv2NameSlot", xSlot, "subregistry", Address.fromString(CHILD).toHexString());
  // ... but getSubregistry() returns zero for the unregistered name.
  let ns = namespaceId(Address.fromString(CHILD), xPath).toHexString();
  assert.fieldEquals("ENSv2Namespace", ns, "active", "false");
  let yId = pathNamehash(xPath, Bytes.fromI32(522)).toHexString();
  assert.fieldEquals("ENSv2NamePath", yId, "active", "false");
  assert.fieldEquals("Domain", yId, "owner", EMPTY);
});

// --- v2.3.3: index allocation under recursion, recursive deactivation ---

test("a self-linking registry reached through a parent name with two paths indexes every namespace it gets", () => {
  dataSourceMock.setNetwork("sepolia");
  const SHARED = "0xf9f9f9f9f9f9f9f9f9f9f9f9f9f9f9f9f9f9f9f9";
  const SELF = "0xfafafafafafafafafafafafafafafafafafafafa";
  handleLabelRegistered(createLabelRegisteredEvent(SELF, slotToken(1), Bytes.fromI32(631), "a"));
  handleSubregistryUpdated(createSubregistryUpdatedEvent(SELF, slotToken(1), SELF));
  // SHARED serves two root names, so its slot "x" has two paths.
  handleLabelRegistered(createLabelRegisteredEvent(ROOT_REGISTRY, slotToken(1), Bytes.fromI32(632), "b1"));
  handleSubregistryUpdated(createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), SHARED));
  handleLabelRegistered(createLabelRegisteredEvent(ROOT_REGISTRY, slotToken(2), Bytes.fromI32(633), "b2"));
  handleSubregistryUpdated(createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(2), SHARED));
  handleLabelRegistered(createLabelRegisteredEvent(SHARED, slotToken(1), Bytes.fromI32(634), "x"));
  assert.fieldEquals("ENSv2NameSlot", slotIdFor(SHARED, 1), "pathCount", "2");

  handleSubregistryUpdated(createSubregistryUpdatedEvent(SHARED, slotToken(1), SELF));

  // Every namespace row is indexed exactly once: the root namespace and
  // SHARED's two, plus all of SELF's.
  let self = ENSv2Registry.load(Address.fromString(SELF))!;
  assert.entityCount("ENSv2Namespace", 3 + self.namespaceCount);
  assert.entityCount("ENSv2RegistryNamespaceIndex", 3 + self.namespaceCount);
  for (let i = 0; i < self.namespaceCount; i++) {
    assert.fieldEquals(
      "ENSv2RegistryNamespaceIndex",
      registryNamespaceIndexId(self.id, i).toHexString(),
      "index",
      i.toString()
    );
  }
});

test("switching a subregistry retires the old registry's names at every depth, and switching back restores them", () => {
  dataSourceMock.setNetwork("sepolia");
  const OLD = "0xf6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6";
  const OLD_CHILD = "0xf7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7";
  const EMPTY_REGISTRY = "0xf8f8f8f8f8f8f8f8f8f8f8f8f8f8f8f8f8f8f8f8";
  let baseHash = Bytes.fromI32(620);
  let xHash = Bytes.fromI32(621);
  let yHash = Bytes.fromI32(622);
  handleLabelRegistered(createLabelRegisteredEvent(ROOT_REGISTRY, slotToken(1), baseHash, "base"));
  handleSubregistryUpdated(createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), OLD));
  handleLabelRegistered(createLabelRegisteredEvent(OLD, slotToken(1), xHash, "x"));
  handleSubregistryUpdated(createSubregistryUpdatedEvent(OLD, slotToken(1), OLD_CHILD));
  handleLabelRegistered(createLabelRegisteredEvent(OLD_CHILD, slotToken(1), yHash, "y"));

  let xPath = pathNamehash(rootPath(baseHash), xHash);
  let yId = pathNamehash(xPath, yHash).toHexString();
  assert.fieldEquals("Domain", yId, "owner", OWNER.toLowerCase());

  handleSubregistryUpdated(createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), EMPTY_REGISTRY));
  assert.fieldEquals("ENSv2NamePath", xPath.toHexString(), "active", "false");
  assert.fieldEquals("ENSv2NamePath", yId, "active", "false");
  assert.fieldEquals("Domain", yId, "owner", EMPTY);
  assert.fieldEquals(
    "ENSv2Namespace",
    namespaceId(Address.fromString(OLD_CHILD), xPath).toHexString(),
    "active",
    "false"
  );

  handleSubregistryUpdated(createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), OLD));
  assert.fieldEquals("ENSv2NamePath", yId, "active", "true");
  assert.fieldEquals("Domain", yId, "owner", OWNER.toLowerCase());
  assert.fieldEquals("Domain", xPath.toHexString(), "subdomainCount", "1");
});

test("unregistering a name retires every name below it, and re-registering it with its subregistry restores them", () => {
  dataSourceMock.setNetwork("sepolia");
  const PARENT = "0xc5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5";
  const CHILD = "0xc6c6c6c6c6c6c6c6c6c6c6c6c6c6c6c6c6c6c6c6";
  const GRANDCHILD = "0xc7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7";
  let baseHash = Bytes.fromI32(640);
  let xHash = Bytes.fromI32(641);
  let yHash = Bytes.fromI32(642);
  let zHash = Bytes.fromI32(643);
  handleLabelRegistered(createLabelRegisteredEvent(ROOT_REGISTRY, slotToken(1), baseHash, "base"));
  handleSubregistryUpdated(createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(1), PARENT));
  handleLabelRegistered(createLabelRegisteredEvent(PARENT, slotToken(1), xHash, "x"));
  handleSubregistryUpdated(createSubregistryUpdatedEvent(PARENT, slotToken(1), CHILD));
  handleLabelRegistered(createLabelRegisteredEvent(CHILD, slotToken(1), yHash, "y"));
  handleSubregistryUpdated(createSubregistryUpdatedEvent(CHILD, slotToken(1), GRANDCHILD));
  handleLabelRegistered(createLabelRegisteredEvent(GRANDCHILD, slotToken(1), zHash, "z"));
  let xPath = pathNamehash(rootPath(baseHash), xHash);
  let yPath = pathNamehash(xPath, yHash);
  let zId = pathNamehash(yPath, zHash).toHexString();
  assert.fieldEquals("Domain", zId, "name", "z.y.x.base");

  handleLabelUnregistered(createLabelUnregisteredEvent(PARENT, slotToken(1)));
  assert.fieldEquals("ENSv2NamePath", yPath.toHexString(), "active", "false");
  assert.fieldEquals("ENSv2NamePath", zId, "active", "false");
  assert.fieldEquals("Domain", zId, "owner", EMPTY);

  // Re-registration sets the subregistry again (SubregistryUpdated follows
  // LabelRegistered), which brings back everything below.
  handleLabelRegistered(createLabelRegisteredEvent(PARENT, slotToken(1), xHash, "x"));
  handleSubregistryUpdated(createSubregistryUpdatedEvent(PARENT, slotToken(1), CHILD));
  assert.fieldEquals("ENSv2NamePath", zId, "active", "true");
  assert.fieldEquals("Domain", zId, "owner", OWNER.toLowerCase());
  assert.fieldEquals("Domain", yPath.toHexString(), "subdomainCount", "1");
});
