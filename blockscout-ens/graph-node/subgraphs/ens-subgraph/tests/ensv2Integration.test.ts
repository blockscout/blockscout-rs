// Cross-file integration scenarios chain handlers from several mappings in
// one matchstick store. Mapping-local behavior remains in the focused suites.
import { Address, BigInt, Bytes, ethereum } from "@graphprotocol/graph-ts";
import {
  afterEach,
  assert,
  clearStore,
  dataSourceMock,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import {
  handleEACRolesChanged,
  handleExpiryUpdated,
  handleLabelRegistered,
  handleSubregistryUpdated,
  handleTokenResource,
} from "../src/ensv2Registry";
import { handleNameRegistered } from "../src/ensv2Registrar";
import { handleLinked } from "../src/ensv2Resolver";
import { nameSlotId, pathNamehash, resourceId, toSlotId } from "../src/ensv2Utils";
import {
  ExpiryUpdated,
  LabelRegistered,
  SubregistryUpdated,
  TokenResource,
  EACRolesChanged,
} from "../src/types/RootRegistry/PermissionedRegistry";
import { NameRegistered } from "../src/types/ETHRegistrar/ETHRegistrar";
import { Linked } from "../src/types/PermissionedResolver/PermissionedResolver";
import { Domain, Registration, WrappedDomain } from "../src/types/schema";

const ROOT_REGISTRY = "0xB458D6a3a77919449d03e7A6903C26827c1eC43f";
const ETH_REGISTRY = "0xD4eBcbBdF463C9c45784603Db0dDD499BC44A8B4";
const ETH_REGISTRAR = "0x8c2E866B439358c41AE05De9cbE8A00BFEFafFcA";
const OWNER = "0x89205A3A3b2A69De6Dbf7f01ED13B2108B2c43e7";
const SENDER = "0x11111111111111111111111111111111111111aa";
const LOCKED_MIGRATION_CONTROLLER = "0x6029a063d69b09D23c52a754a90E4FE43aDac3A8";
const GRAVEYARD = "0x0000000000000000000000000000000000dead01";
const ROOT_NAMEHASH =
  "0x0000000000000000000000000000000000000000000000000000000000000000";
const ETH_LABELHASH =
  "0x4f5b812789fc606be1b3b16908db13fc7a9adf7ca72641f84d75b47069d3d7f0";
const PAYMENT_TOKEN = "0x3DfC8b53dAFa5eBbb071a8B97678Ab534Ed838D9";

const TWO_POW_32 = BigInt.fromI64(4294967296);
function slotToken(n: i32): BigInt {
  return TWO_POW_32.times(BigInt.fromI32(n));
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
    new ethereum.EventParam("tokenId", ethereum.Value.fromUnsignedBigInt(tokenId))
  );
  event.parameters.push(
    new ethereum.EventParam("labelHash", ethereum.Value.fromFixedBytes(labelHash))
  );
  event.parameters.push(new ethereum.EventParam("label", ethereum.Value.fromString(label)));
  event.parameters.push(
    new ethereum.EventParam("owner", ethereum.Value.fromAddress(Address.fromString(owner)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "expiry",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2000000000))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("sender", ethereum.Value.fromAddress(Address.fromString(sender)))
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
    new ethereum.EventParam("tokenId", ethereum.Value.fromUnsignedBigInt(tokenId))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "subregistry",
      ethereum.Value.fromAddress(Address.fromString(subregistry))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("sender", ethereum.Value.fromAddress(Address.fromString(SENDER)))
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
    new ethereum.EventParam("tokenId", ethereum.Value.fromUnsignedBigInt(tokenId))
  );
  event.parameters.push(
    new ethereum.EventParam("resource", ethereum.Value.fromUnsignedBigInt(resource))
  );
  return event;
};

const createNameRegisteredEvent = (
  tokenId: BigInt,
  label: string,
  duration: BigInt
): NameRegistered => {
  let mockEvent = newMockEvent();
  let event = new NameRegistered(
    Address.fromString(ETH_REGISTRAR),
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
    new ethereum.EventParam("tokenId", ethereum.Value.fromUnsignedBigInt(tokenId))
  );
  event.parameters.push(new ethereum.EventParam("label", ethereum.Value.fromString(label)));
  event.parameters.push(
    new ethereum.EventParam("owner", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "subregistry",
      ethereum.Value.fromAddress(Address.fromString(ETH_REGISTRY))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("resolver", ethereum.Value.fromAddress(Address.zero()))
  );
  event.parameters.push(
    new ethereum.EventParam("duration", ethereum.Value.fromUnsignedBigInt(duration))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "paymentToken",
      ethereum.Value.fromAddress(Address.fromString(PAYMENT_TOKEN))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("referrer", ethereum.Value.fromFixedBytes(Bytes.fromI32(1)))
  );
  event.parameters.push(
    new ethereum.EventParam("base", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(500)))
  );
  event.parameters.push(
    new ethereum.EventParam("premium", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(0)))
  );
  return event;
};

const createExpiryUpdatedEvent = (
  tokenId: BigInt,
  newExpiry: BigInt
): ExpiryUpdated => {
  let mockEvent = newMockEvent();
  let event = new ExpiryUpdated(
    Address.fromString(ETH_REGISTRY),
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
    new ethereum.EventParam("tokenId", ethereum.Value.fromUnsignedBigInt(tokenId))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "newExpiry",
      ethereum.Value.fromUnsignedBigInt(newExpiry)
    )
  );
  return event;
};

const createEACRolesChangedEvent = (
  contract: string,
  resource: BigInt,
  account: string,
  oldRoleBitmap: BigInt,
  newRoleBitmap: BigInt
): EACRolesChanged => {
  let mockEvent = newMockEvent();
  let event = new EACRolesChanged(
    Address.fromString(contract),
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
    new ethereum.EventParam("resource", ethereum.Value.fromUnsignedBigInt(resource))
  );
  event.parameters.push(
    new ethereum.EventParam("account", ethereum.Value.fromAddress(Address.fromString(account)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "oldRoleBitmap",
      ethereum.Value.fromUnsignedBigInt(oldRoleBitmap)
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "newRoleBitmap",
      ethereum.Value.fromUnsignedBigInt(newRoleBitmap)
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
    new ethereum.EventParam(
      "node",
      ethereum.Value.fromFixedBytes(node)
    )
  );
  event.parameters.push(
    new ethereum.EventParam("name", ethereum.Value.fromBytes(name))
  );
  return event;
};

function setupEthNamespace(rootTokenN: i32): Bytes {
  let rootLabelHash = Bytes.fromHexString(ETH_LABELHASH);
  handleLabelRegistered(
    createLabelRegisteredEvent(ROOT_REGISTRY, slotToken(rootTokenN), rootLabelHash, "eth")
  );
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(rootTokenN), ETH_REGISTRY)
  );
  return pathNamehash(Bytes.fromHexString(ROOT_NAMEHASH), rootLabelHash);
}

afterEach(() => {
  dataSourceMock.resetValues();
  clearStore();
});

// assert.fieldEquals compares an entity's id as its lowercase-hex string
// form regardless of the underlying GraphQL type.
// Production code now builds composite ids as fixed-width Bytes
// concatenation with no delimiter — this mirrors that exact encoding.
function bigIntHex32(i: BigInt): string {
  return i.toHex().slice(2).padStart(64, "0");
}

test("full chain across registry, paths, domain, registrar, resources, roles, and resolver mapping files stays consistent in one store", () => {
  dataSourceMock.setNetwork("sepolia");

  let ethBaseNamehash = setupEthNamespace(1);

  let labelHash = Bytes.fromI32(80);
  let tokenId = slotToken(2);
  handleLabelRegistered(
    createLabelRegisteredEvent(ETH_REGISTRY, tokenId, labelHash, "chaintest")
  );

  let slotId = nameSlotId(
    Address.fromString(ETH_REGISTRY),
    toSlotId(tokenId)
  ).toHexString();
  let pathId = pathNamehash(ethBaseNamehash, labelHash).toHexString();
  let registrationId = labelHash.toHexString();

  handleNameRegistered(createNameRegisteredEvent(tokenId, "chaintest", BigInt.fromI32(31536000)));

  let resource = BigInt.fromI32(5);
  handleTokenResource(createTokenResourceEvent(ETH_REGISTRY, tokenId, resource));
  let resourceEntityId = resourceId(Address.fromString(ETH_REGISTRY), resource).toHexString();

  let roleAccount = "0x22222222222222222222222222222222222222bb";
  handleEACRolesChanged(
    createEACRolesChangedEvent(
      ETH_REGISTRY,
      resource,
      roleAccount,
      BigInt.fromI32(0),
      BigInt.fromI32(1)
    )
  );
  let assignmentId = Address.fromString(ETH_REGISTRY)
    .toHexString()
    .concat(bigIntHex32(resource))
    .concat(Address.fromString(roleAccount).toHexString().slice(2));

  let resolverAddress = "0x33333333333333333333333333333333333333cc";
  let resolverNode = Bytes.fromI32(211);
  handleLinked(
    createLinkedEvent(
      resolverAddress,
      BigInt.fromI32(1),
      resolverNode,
      encodeLabel("chaintest")
    )
  );
  let linkId = Address.fromString(resolverAddress)
    .toHexString()
    .concat(resolverNode.toHexString().slice(2));

  // Each assertion below reads state written by a different mapping file
  // (ensv2Registry/ensv2Paths/ensv2Domain/ensv2Registrar/ensv2Roles/
  // ensv2Resolver) from the same store, in the order the events fired.
  assert.fieldEquals("ENSv2NameSlot", slotId, "label", "chaintest");
  assert.fieldEquals("Domain", pathId, "name", "chaintest.eth");
  assert.fieldEquals("Registration", registrationId, "domain", pathId);
  assert.fieldEquals("ENSv2Registration", slotId, "duration", "31536000");
  assert.fieldEquals("ENSv2Registration", slotId, "path", pathId);
  assert.fieldEquals("ENSv2Registration", slotId, "resource", resourceEntityId);
  assert.fieldEquals("ENSv2Registration", slotId, "expiryDate", "2000000000");
  assert.fieldEquals("ENSv2RoleAssignment", assignmentId, "resourceEntity", resourceEntityId);
  assert.fieldEquals("ENSv2ResolverLink", linkId, "active", "true");
});

test("direct ETHRegistry renewal synchronizes native registration expiry without resetting registration date", () => {
  dataSourceMock.setNetwork("sepolia");

  setupEthNamespace(10);
  let labelHash = Bytes.fromI32(81);
  let tokenId = slotToken(11);
  handleLabelRegistered(
    createLabelRegisteredEvent(ETH_REGISTRY, tokenId, labelHash, "directrenew")
  );

  let registered = createNameRegisteredEvent(
    tokenId,
    "directrenew",
    BigInt.fromI32(31536000)
  );
  registered.block.timestamp = BigInt.fromI32(1234);
  handleNameRegistered(registered);

  let newExpiry = BigInt.fromI32(2100000000);
  handleExpiryUpdated(createExpiryUpdatedEvent(tokenId, newExpiry));

  let registrationId = nameSlotId(
    Address.fromString(ETH_REGISTRY),
    toSlotId(tokenId)
  ).toHexString();
  assert.fieldEquals(
    "ENSv2Registration",
    registrationId,
    "expiryDate",
    newExpiry.toString()
  );
  assert.fieldEquals(
    "ENSv2Registration",
    registrationId,
    "registrationDate",
    "1234"
  );
});

test("registrar-before-registry and registrar-after-registry orderings produce identical Domain/Registration state", () => {
  dataSourceMock.setNetwork("sepolia");

  let labelHash = Bytes.fromI32(85);
  let tokenId = slotToken(9);

  // Order A: registrar event first, then the registry event that actually
  // materialises the path/Domain projection.
  let ethBaseNamehashA = setupEthNamespace(2);
  handleNameRegistered(createNameRegisteredEvent(tokenId, "orderindep", BigInt.fromI32(31536000)));
  handleLabelRegistered(
    createLabelRegisteredEvent(ETH_REGISTRY, tokenId, labelHash, "orderindep")
  );
  let resource = BigInt.fromI32(27);
  handleTokenResource(createTokenResourceEvent(ETH_REGISTRY, tokenId, resource));
  let pathIdA = pathNamehash(ethBaseNamehashA, labelHash).toHexString();
  let slotIdA = nameSlotId(Address.fromString(ETH_REGISTRY), toSlotId(tokenId)).toHexString();
  let resourceIdA = resourceId(Address.fromString(ETH_REGISTRY), resource).toHexString();
  let registrationIdA = labelHash.toHexString();
  assert.fieldEquals("Domain", pathIdA, "name", "orderindep.eth");
  assert.fieldEquals("Registration", registrationIdA, "domain", pathIdA);
  assert.fieldEquals("Domain", pathIdA, "expiryDate", "2002419200");
  assert.fieldEquals("ENSv2Registration", slotIdA, "path", pathIdA);
  assert.fieldEquals("ENSv2Registration", slotIdA, "resource", resourceIdA);
  assert.fieldEquals("ENSv2Registration", slotIdA, "expiryDate", "2000000000");

  clearStore();

  // Order B: the registry event (and hence Domain projection) fires first,
  // registrar enrichment arrives after.
  let ethBaseNamehashB = setupEthNamespace(2);
  handleLabelRegistered(
    createLabelRegisteredEvent(ETH_REGISTRY, tokenId, labelHash, "orderindep")
  );
  handleTokenResource(createTokenResourceEvent(ETH_REGISTRY, tokenId, resource));
  handleNameRegistered(createNameRegisteredEvent(tokenId, "orderindep", BigInt.fromI32(31536000)));
  let pathIdB = pathNamehash(ethBaseNamehashB, labelHash).toHexString();
  let slotIdB = nameSlotId(Address.fromString(ETH_REGISTRY), toSlotId(tokenId)).toHexString();
  let resourceIdB = resourceId(Address.fromString(ETH_REGISTRY), resource).toHexString();
  let registrationIdB = labelHash.toHexString();

  assert.fieldEquals("Domain", pathIdB, "name", "orderindep.eth");
  assert.fieldEquals("Registration", registrationIdB, "domain", pathIdB);
  assert.fieldEquals("Domain", pathIdB, "expiryDate", "2002419200");
  assert.fieldEquals("ENSv2Registration", slotIdB, "path", pathIdB);
  assert.fieldEquals("ENSv2Registration", slotIdB, "resource", resourceIdB);
  assert.fieldEquals("ENSv2Registration", slotIdB, "expiryDate", "2000000000");
});

test("late-link + Domain projection combined: pre-existing child registration stays unprojected, a fresh post-link registration is fully projected", () => {
  dataSourceMock.setNetwork("sepolia");

  const LATE_CHILD_REGISTRY = "0x777777777777777777777777777777777777777f";

  // Registered in the child registry before it is linked anywhere.
  let preexistingLabelHash = Bytes.fromI32(90);
  handleLabelRegistered(
    createLabelRegisteredEvent(
      LATE_CHILD_REGISTRY,
      slotToken(1),
      preexistingLabelHash,
      "preexisting"
    )
  );
  let preexistingSlotId = nameSlotId(
    Address.fromString(LATE_CHILD_REGISTRY),
    toSlotId(slotToken(1))
  ).toHexString();
  assert.fieldEquals("ENSv2NameSlot", preexistingSlotId, "pathCount", "0");

  // Root's "latebase" slot, then the late-link.
  let baseLabelHash = Bytes.fromI32(91);
  handleLabelRegistered(
    createLabelRegisteredEvent(ROOT_REGISTRY, slotToken(2), baseLabelHash, "latebase")
  );
  handleSubregistryUpdated(
    createSubregistryUpdatedEvent(ROOT_REGISTRY, slotToken(2), LATE_CHILD_REGISTRY)
  );
  let baseNamehash = pathNamehash(Bytes.fromHexString(ROOT_NAMEHASH), baseLabelHash);
  let basePathId = baseNamehash.toHexString();

  // The pre-existing registration is still not backfilled: no Domain row
  // exists at the id it would have gotten had it been materialised now.
  let wouldBePathId = pathNamehash(baseNamehash, preexistingLabelHash).toHexString();
  assert.notInStore("Domain", wouldBePathId);
  assert.notInStore("ENSv2NamePath", wouldBePathId);
  assert.fieldEquals("ENSv2NameSlot", preexistingSlotId, "pathCount", "0");

  // A fresh registration in the same child registry, after the link, is
  // projected into Domain; its RootRegistry parent remains native-only.
  let freshLabelHash = Bytes.fromI32(92);
  handleLabelRegistered(
    createLabelRegisteredEvent(
      LATE_CHILD_REGISTRY,
      slotToken(2),
      freshLabelHash,
      "freshpostlink"
    )
  );
  let freshPathId = pathNamehash(baseNamehash, freshLabelHash).toHexString();
  assert.fieldEquals("Domain", freshPathId, "name", "freshpostlink.latebase");
  assert.notInStore("Domain", basePathId);
  let freshDomain = Domain.load(freshPathId);
  let hasParent = false;
  if (freshDomain != null && freshDomain.parent) {
    hasParent = true;
  }
  assert.assertTrue(!hasParent);
});

test("migration correction survives unrelated cross-file activity in the same store", () => {
  dataSourceMock.setNetwork("sepolia");

  let ethBaseNamehash = setupEthNamespace(3);

  let labelHash = Bytes.fromI32(203);
  let domainId = pathNamehash(ethBaseNamehash, labelHash).toHexString();
  let registrationId = labelHash.toHexString();

  let graveyard = GRAVEYARD;
  let domain = new Domain(domainId);
  domain.storedOffchain = false; // Blockscout/BENS extension
  domain.resolvedWithWildcard = false; // Blockscout/BENS extension
  domain.owner = graveyard;
  domain.registrant = graveyard;
  domain.wrappedOwner = graveyard;
  domain.isMigrated = true;
  domain.subdomainCount = 0;
  domain.createdAt = BigInt.fromI32(0);
  domain.save();

  let registration = new Registration(registrationId);
  registration.domain = domainId;
  registration.registrationDate = BigInt.fromI32(0);
  registration.expiryDate = BigInt.fromI32(1000000000);
  registration.registrant = graveyard;
  registration.save();

  let wrappedDomain = new WrappedDomain(domainId);
  wrappedDomain.domain = domainId;
  wrappedDomain.expiryDate = BigInt.fromI32(1000000000);
  wrappedDomain.fuses = 65536; // PARENT_CANNOT_CONTROL — "locked"
  wrappedDomain.owner = graveyard;
  wrappedDomain.save();

  handleLabelRegistered(
    createLabelRegisteredEvent(
      ETH_REGISTRY,
      slotToken(4),
      labelHash,
      "migratedlocked",
      OWNER,
      LOCKED_MIGRATION_CONTROLLER
    )
  );

  assert.fieldEquals(
    "Domain",
    domainId,
    "wrappedOwner",
    Address.fromString(OWNER).toHexString()
  );
  assert.fieldEquals("Domain", domainId, "registrant", GRAVEYARD);
  assert.fieldEquals("Domain", domainId, "owner", GRAVEYARD);
  assert.fieldEquals("Registration", registrationId, "registrant", GRAVEYARD);

  // Unrelated activity from three other mapping files, fired into the same
  // store after the migration correction landed.
  handleLabelRegistered(
    createLabelRegisteredEvent(ROOT_REGISTRY, slotToken(5), Bytes.fromI32(210), "unrelated")
  );
  handleEACRolesChanged(
    createEACRolesChangedEvent(
      ETH_REGISTRY,
      BigInt.fromI32(999),
      "0x44444444444444444444444444444444444444dd",
      BigInt.fromI32(0),
      BigInt.fromI32(1)
    )
  );
  handleLinked(
    createLinkedEvent(
      "0x55555555555555555555555555555555555555ee",
      BigInt.fromI32(2),
      Bytes.fromI32(212),
      encodeLabel("unrelated")
    )
  );

  // Migration-corrected fields are exactly as before the unrelated activity.
  assert.fieldEquals(
    "Domain",
    domainId,
    "wrappedOwner",
    Address.fromString(OWNER).toHexString()
  );
  assert.fieldEquals("Domain", domainId, "registrant", GRAVEYARD);
  assert.fieldEquals("Domain", domainId, "owner", GRAVEYARD);
  assert.fieldEquals("Registration", registrationId, "registrant", GRAVEYARD);
});
