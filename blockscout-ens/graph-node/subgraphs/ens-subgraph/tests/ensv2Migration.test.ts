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
  handleLabelReserved,
  handleSubregistryUpdated,
  handleTokenResource,
  handleTransferSingle,
} from "../src/ensv2Registry";
import { nameSlotId, pathNamehash, toSlotId } from "../src/ensv2Utils";
import { Domain, Registration, WrappedDomain } from "../src/types/schema";
import {
  ExpiryUpdated,
  LabelRegistered,
  LabelReserved,
  SubregistryUpdated,
  TokenResource,
  TransferSingle,
} from "../src/types/RootRegistry/PermissionedRegistry";

const ROOT_REGISTRY = "0xB458D6a3a77919449d03e7A6903C26827c1eC43f";
const ETH_REGISTRY = "0xD4eBcbBdF463C9c45784603Db0dDD499BC44A8B4";
const OWNER = "0x89205A3A3b2A69De6Dbf7f01ED13B2108B2c43e7";
const OWNER_2 = "0xF0205A3A3b2A69De6Dbf7f01ED13B2108B2c4321";
const SENDER = "0x11111111111111111111111111111111111111aa";
const GRAVEYARD = "0x0000000000000000000000000000000000dead01";
const LOCKED_MIGRATION_CONTROLLER = "0x6029a063d69b09D23c52a754a90E4FE43aDac3A8";
const UNLOCKED_MIGRATION_CONTROLLER = "0x2a35B94DF22cc7354570be2284655E2CDC0e64A2";
const ROOT_NAMEHASH =
  "0x0000000000000000000000000000000000000000000000000000000000000000";

const TWO_POW_32 = BigInt.fromI64(4294967296);
function slotToken(n: i32): BigInt {
  return TWO_POW_32.times(BigInt.fromI32(n));
}

const createLabelRegisteredEvent = (
  registryAddress: string,
  tokenId: BigInt,
  labelHash: Bytes,
  label: string,
  owner: string = OWNER,
  sender: string = SENDER
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
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2000000000))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "sender",
      ethereum.Value.fromAddress(Address.fromString(sender))
    )
  );
  return event;
};

const createLabelReservedEvent = (
  registryAddress: string,
  tokenId: BigInt,
  labelHash: Bytes,
  label: string
): LabelReserved => {
  let mockEvent = newMockEvent();
  let event = new LabelReserved(
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
      "expiry",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2000000000))
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

const createTokenResourceEvent = (
  registryAddress: string,
  tokenId: BigInt,
  resource: BigInt
): TokenResource => {
  let mockEvent = newMockEvent();
  let event = new TokenResource(
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
      "resource",
      ethereum.Value.fromUnsignedBigInt(resource)
    )
  );
  return event;
};

const createTransferSingleEvent = (
  registryAddress: string,
  tokenId: BigInt,
  from: string,
  to: string
): TransferSingle => {
  let mockEvent = newMockEvent();
  let event = new TransferSingle(
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
      "operator",
      ethereum.Value.fromAddress(Address.fromString(from))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "from",
      ethereum.Value.fromAddress(Address.fromString(from))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "to",
      ethereum.Value.fromAddress(Address.fromString(to))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "id",
      ethereum.Value.fromUnsignedBigInt(tokenId)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "value",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(1))
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

// Links ETHRegistry under root's "eth" slot so ETHRegistry has an active
// namespace and materializePathsForSlot actually produces paths/domains for
// registrations inside it — mirrors tests/ensv2Paths.test.ts's .eth setup.
// Returns the "eth" ENSv2NamePath's own namehash (ETHRegistry's namespace
// baseNamehash), needed to compute domain/registration ids for a given
// migrated label.
//
// Uses the REAL keccak256("eth") labelhash (not an arbitrary placeholder)
// so the resulting namespace.baseNamehash is genuinely ETH_NODE — required
// since getEthDomainId filters on exactly that. A placeholder labelhash would
// produce a namespace getEthDomainId correctly refuses to treat as "eth".
function setupEthNamespace(): Bytes {
  let ethLabelHash = Bytes.fromByteArray(
    crypto.keccak256(ByteArray.fromUTF8("eth"))
  );
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      slotToken(999),
      ethLabelHash,
      "eth"
    )
  );
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(999), ETH_REGISTRY)
  );
  return pathNamehash(Bytes.fromHexString(ROOT_NAMEHASH), ethLabelHash);
}

function addEthRegistryAliasNamespace(
  tokenId: BigInt,
  labelHash: Bytes,
  label: string
): Bytes {
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ROOT_REGISTRY,
      tokenId,
      labelHash,
      label
    )
  );
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, tokenId, ETH_REGISTRY)
  );
  return pathNamehash(Bytes.fromHexString(ROOT_NAMEHASH), labelHash);
}

afterEach(() => {
  dataSourceMock.resetValues();
  clearStore();
});

test("unwrapped migration corrects registrant only, never owner, and a post-migration transfer keeps tracking it", () => {
  dataSourceMock.setNetwork("sepolia");
  let ethBaseNamehash = setupEthNamespace();
  let aliasBaseNamehash = addEthRegistryAliasNamespace(
    slotToken(998),
    Bytes.fromI32(998),
    "migration-alias"
  );

  let labelHash = Bytes.fromI32(200);
  let domainId = pathNamehash(ethBaseNamehash, labelHash).toHexString();
  let registrationId = labelHash.toHexString();

  // Pre-seed legacy state as if this were a real pre-migration ENSv1 name.
  let domain = new Domain(domainId);
  domain.storedOffchain = false; // Blockscout/BENS extension
  domain.resolvedWithWildcard = false; // Blockscout/BENS extension
  domain.owner = GRAVEYARD;
  domain.registrant = GRAVEYARD;
  domain.isMigrated = true;
  domain.subdomainCount = 0;
  domain.createdAt = BigInt.fromI32(0);
  domain.save();

  let registration = new Registration(registrationId);
  registration.domain = domainId;
  registration.registrationDate = BigInt.fromI32(0);
  registration.expiryDate = BigInt.fromI32(1000000000);
  registration.registrant = GRAVEYARD;
  registration.save();

  let tokenId = slotToken(1);
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ETH_REGISTRY,
      tokenId,
      labelHash,
      "unwrapped",
      OWNER,
      LOCKED_MIGRATION_CONTROLLER
    )
  );

  let slotId = nameSlotId(
    Address.fromString(ETH_REGISTRY),
    toSlotId(tokenId)
  ).toHexString();
  assert.fieldEquals("ENSv2NameSlot", slotId, "migratedFromV1", "true");

  assert.fieldEquals(
    "Domain",
    domainId,
    "registrant",
    Address.fromString(OWNER).toHexString()
  );
  assert.fieldEquals(
    "Registration",
    registrationId,
    "registrant",
    Address.fromString(OWNER).toHexString()
  );
  // domain.owner is never touched by migration correction.
  assert.fieldEquals("Domain", domainId, "owner", GRAVEYARD);
  assert.fieldEquals("Registration", registrationId, "domain", domainId);
  assert.fieldEquals(
    "Domain",
    pathNamehash(aliasBaseNamehash, labelHash).toHexString(),
    "name",
    "unwrapped.migration-alias"
  );

  // Post-migration transfer keeps the same legacy field tracking ownership.
  handleTokenResource(
    createTokenResourceEvent(ETH_REGISTRY, tokenId, BigInt.fromI32(1))
  );
  handleTransferSingle(
    createTransferSingleEvent(ETH_REGISTRY, tokenId, OWNER, OWNER_2)
  );

  assert.fieldEquals(
    "Domain",
    domainId,
    "registrant",
    Address.fromString(OWNER_2).toHexString()
  );
  assert.fieldEquals(
    "Registration",
    registrationId,
    "registrant",
    Address.fromString(OWNER_2).toHexString()
  );
  assert.fieldEquals("Domain", domainId, "owner", GRAVEYARD);
});

test("migration-flagged registration with no pre-existing v1 Domain row sets owner instead of crashing", () => {
  // Regression test for the Sepolia indexing failure at block #11480885:
  // Domain#save on "missing value for non-nullable field `owner`" when a
  // migration-controller-sent LabelRegistered materialised into a namespace
  // with no pre-existing ENSv1 Domain row to inherit a graveyard owner from.
  dataSourceMock.setNetwork("sepolia");
  let ethBaseNamehash = setupEthNamespace();

  let labelHash = Bytes.fromI32(555);
  let domainId = pathNamehash(ethBaseNamehash, labelHash).toHexString();

  // Deliberately no pre-seeded Domain/Registration row — unlike the other
  // migration tests, there is no ENSv1 legacy state to inherit from.
  assert.notInStore("Domain", domainId);

  let tokenId = slotToken(2);
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ETH_REGISTRY,
      tokenId,
      labelHash,
      "freshmigrated",
      OWNER,
      LOCKED_MIGRATION_CONTROLLER
    )
  );

  assert.fieldEquals(
    "Domain",
    domainId,
    "owner",
    Address.fromString(OWNER).toHexString()
  );
});

test("wrapped-unlocked migration (no WrappedDomain) behaves the same as unwrapped", () => {
  dataSourceMock.setNetwork("sepolia");
  let ethBaseNamehash = setupEthNamespace();

  let labelHash = Bytes.fromI32(201);
  let domainId = pathNamehash(ethBaseNamehash, labelHash).toHexString();
  let registrationId = labelHash.toHexString();

  let domain = new Domain(domainId);
  domain.storedOffchain = false; // Blockscout/BENS extension
  domain.resolvedWithWildcard = false; // Blockscout/BENS extension
  domain.owner = GRAVEYARD;
  domain.registrant = GRAVEYARD;
  domain.isMigrated = true;
  domain.subdomainCount = 0;
  domain.createdAt = BigInt.fromI32(0);
  domain.save();

  let registration = new Registration(registrationId);
  registration.domain = domainId;
  registration.registrationDate = BigInt.fromI32(0);
  registration.expiryDate = BigInt.fromI32(1000000000);
  registration.registrant = GRAVEYARD;
  registration.save();
  // No WrappedDomain row — already unwrapped earlier in the same tx.

  handleLabelRegistered(
    createLabelRegisteredEvent(
      ETH_REGISTRY,
      slotToken(2),
      labelHash,
      "wrappedunlocked",
      OWNER,
      UNLOCKED_MIGRATION_CONTROLLER
    )
  );

  assert.fieldEquals(
    "Domain",
    domainId,
    "registrant",
    Address.fromString(OWNER).toHexString()
  );
  assert.fieldEquals(
    "Registration",
    registrationId,
    "registrant",
    Address.fromString(OWNER).toHexString()
  );
  assert.fieldEquals("Domain", domainId, "owner", GRAVEYARD);
  assert.notInStore("WrappedDomain", domainId);
});

test("wrapped-locked migration corrects wrappedOwner only, leaves registrant/owner untouched, and a post-migration transfer keeps tracking it", () => {
  dataSourceMock.setNetwork("sepolia");
  let ethBaseNamehash = setupEthNamespace();
  let aliasBaseNamehash = addEthRegistryAliasNamespace(
    slotToken(997),
    Bytes.fromI32(997),
    "locked-alias"
  );

  let labelHash = Bytes.fromI32(202);
  let domainId = pathNamehash(ethBaseNamehash, labelHash).toHexString();
  let registrationId = labelHash.toHexString();

  let domain = new Domain(domainId);
  domain.storedOffchain = false; // Blockscout/BENS extension
  domain.resolvedWithWildcard = false; // Blockscout/BENS extension
  domain.owner = GRAVEYARD;
  domain.registrant = GRAVEYARD;
  domain.wrappedOwner = GRAVEYARD;
  domain.isMigrated = true;
  domain.subdomainCount = 0;
  domain.createdAt = BigInt.fromI32(0);
  domain.save();

  let registration = new Registration(registrationId);
  registration.domain = domainId;
  registration.registrationDate = BigInt.fromI32(0);
  registration.expiryDate = BigInt.fromI32(1000000000);
  registration.registrant = GRAVEYARD;
  registration.save();

  let wrappedDomain = new WrappedDomain(domainId);
  wrappedDomain.domain = domainId;
  wrappedDomain.expiryDate = BigInt.fromI32(1000000000);
  wrappedDomain.fuses = 65536; // PARENT_CANNOT_CONTROL — "locked"
  wrappedDomain.owner = GRAVEYARD;
  wrappedDomain.save();

  let tokenId = slotToken(3);
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ETH_REGISTRY,
      tokenId,
      labelHash,
      "wrappedlocked",
      OWNER,
      LOCKED_MIGRATION_CONTROLLER
    )
  );

  assert.fieldEquals(
    "WrappedDomain",
    domainId,
    "owner",
    Address.fromString(OWNER).toHexString()
  );
  assert.fieldEquals(
    "Domain",
    domainId,
    "wrappedOwner",
    Address.fromString(OWNER).toHexString()
  );
  // registrant/owner are untouched by the wrapped branch.
  assert.fieldEquals("Domain", domainId, "registrant", GRAVEYARD);
  assert.fieldEquals("Registration", registrationId, "registrant", GRAVEYARD);
  assert.fieldEquals("Domain", domainId, "owner", GRAVEYARD);
  assert.fieldEquals("Registration", registrationId, "domain", domainId);
  assert.fieldEquals(
    "Domain",
    pathNamehash(aliasBaseNamehash, labelHash).toHexString(),
    "name",
    "wrappedlocked.locked-alias"
  );
  assert.fieldEquals(
    "Domain",
    pathNamehash(aliasBaseNamehash, labelHash).toHexString(),
    "registrant",
    Address.fromString(OWNER).toHexString()
  );

  handleTokenResource(
    createTokenResourceEvent(ETH_REGISTRY, tokenId, BigInt.fromI32(2))
  );
  handleTransferSingle(
    createTransferSingleEvent(ETH_REGISTRY, tokenId, OWNER, OWNER_2)
  );

  assert.fieldEquals(
    "WrappedDomain",
    domainId,
    "owner",
    Address.fromString(OWNER_2).toHexString()
  );
  assert.fieldEquals(
    "Domain",
    domainId,
    "wrappedOwner",
    Address.fromString(OWNER_2).toHexString()
  );
  assert.fieldEquals("Domain", domainId, "registrant", GRAVEYARD);
});

test("a native ENSv2 .eth registration clears stale ENSv1 wrapped state", () => {
  dataSourceMock.setNetwork("sepolia");
  let ethBaseNamehash = setupEthNamespace();

  let labelHash = Bytes.fromI32(303);
  let domainId = pathNamehash(ethBaseNamehash, labelHash).toHexString();

  // ENSv1 state of a name that expired while wrapped: registry owner is
  // still the NameWrapper and the token's last holder is still recorded,
  // because expiry emits no event on either contract.
  let domain = new Domain(domainId);
  domain.storedOffchain = false; // Blockscout/BENS extension
  domain.resolvedWithWildcard = false; // Blockscout/BENS extension
  domain.owner = SENDER;
  domain.wrappedOwner = OWNER_2;
  domain.isMigrated = true;
  domain.subdomainCount = 0;
  domain.createdAt = BigInt.fromI32(0);
  domain.save();

  let wrappedDomain = new WrappedDomain(domainId);
  wrappedDomain.domain = domainId;
  wrappedDomain.expiryDate = BigInt.fromI32(1000000000);
  wrappedDomain.fuses = 0;
  wrappedDomain.owner = OWNER_2;
  wrappedDomain.save();

  handleLabelRegistered(
    createLabelRegisteredEvent(
      ETH_REGISTRY,
      slotToken(6),
      labelHash,
      "freshv2",
      OWNER,
      SENDER
    )
  );

  assert.fieldEquals(
    "Domain",
    domainId,
    "owner",
    Address.fromString(OWNER).toHexString()
  );
  assert.fieldEquals("Domain", domainId, "wrappedOwner", "null");
  assert.notInStore("WrappedDomain", domainId);
});

test("a non-migration registration is not flagged", () => {
  dataSourceMock.setNetwork("sepolia");
  setupEthNamespace();

  let tokenId = slotToken(4);
  handleLabelRegistered(
    createLabelRegisteredEvent(
      ETH_REGISTRY,
      tokenId,
      Bytes.fromI32(203),
      "ordinary",
      OWNER,
      SENDER
    )
  );

  let slotId = nameSlotId(
    Address.fromString(ETH_REGISTRY),
    toSlotId(tokenId)
  ).toHexString();
  assert.fieldEquals("ENSv2NameSlot", slotId, "migratedFromV1", "false");
});

test("handleExpiryUpdated on a REGISTERED .eth slot syncs raw Registration.expiryDate and grace-inclusive Domain.expiryDate", () => {
  dataSourceMock.setNetwork("sepolia");
  let ethBaseNamehash = setupEthNamespace();

  let labelHash = Bytes.fromI32(210);
  let domainId = pathNamehash(ethBaseNamehash, labelHash).toHexString();
  let registrationId = labelHash.toHexString();
  let tokenId = slotToken(5);

  handleLabelRegistered(
    createLabelRegisteredEvent(ETH_REGISTRY, tokenId, labelHash, "renewme")
  );

  let newExpiry = BigInt.fromI32(2100000000);
  handleExpiryUpdated(createExpiryUpdatedEvent(ETH_REGISTRY, tokenId, newExpiry));

  assert.fieldEquals("Registration", registrationId, "expiryDate", "2100000000");
  // v2GracePeriod is 2,419,200s (28 days) per src/ensv2Constants.ts.
  assert.fieldEquals("Domain", domainId, "expiryDate", "2102419200");
});

test("handleExpiryUpdated on a RESERVED slot leaves legacy fields unchanged", () => {
  dataSourceMock.setNetwork("sepolia");
  let ethBaseNamehash = setupEthNamespace();

  let labelHash = Bytes.fromI32(211);
  let domainId = pathNamehash(ethBaseNamehash, labelHash).toHexString();
  let registrationId = labelHash.toHexString();
  let tokenId = slotToken(6);

  // Pre-seed Domain/Registration with a known expiry, as ETHRenewerV1's own
  // v1-authoritative path would have already set correctly — this must
  // survive untouched.
  let domain = new Domain(domainId);
  domain.storedOffchain = false; // Blockscout/BENS extension
  domain.resolvedWithWildcard = false; // Blockscout/BENS extension
  domain.owner = OWNER;
  domain.isMigrated = true;
  domain.subdomainCount = 0;
  domain.createdAt = BigInt.fromI32(0);
  domain.expiryDate = BigInt.fromI32(1500000000);
  domain.save();

  let registration = new Registration(registrationId);
  registration.domain = domainId;
  registration.registrationDate = BigInt.fromI32(0);
  registration.expiryDate = BigInt.fromI32(1400000000);
  registration.registrant = OWNER;
  registration.save();

  handleLabelReserved(
    createLabelReservedEvent(ETH_REGISTRY, tokenId, labelHash, "reserveme")
  );
  handleExpiryUpdated(
    createExpiryUpdatedEvent(ETH_REGISTRY, tokenId, BigInt.fromI32(2100000000))
  );

  assert.fieldEquals("Registration", registrationId, "expiryDate", "1400000000");
  assert.fieldEquals("Domain", domainId, "expiryDate", "1500000000");
});
