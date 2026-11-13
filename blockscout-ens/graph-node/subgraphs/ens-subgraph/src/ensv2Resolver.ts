// PermissionedResolver's recordId-keyed native event model. PublicResolverV2
// emits the classic node-keyed profile events and is handled by the existing
// addressless Resolver data source instead of being wired here a second time.
import { Address, BigInt, Bytes, ethereum } from "@graphprotocol/graph-ts";
import { concat, uint256ToByteArray } from "./utils";
import {
  resolverAddressId,
  resolverLinkId,
  resolverRecordId,
  resolverTextId,
  resourceId,
} from "./ensv2Utils";
import { decodeName } from "./nameWrapper";
import { processEACRolesChanged } from "./ensv2Roles";
import {
  activateRecordMembership,
  activeRecordMembershipAt,
  appendAddressIndex,
  appendTextIndex,
  clearLegacyResolverSnapshot,
  deactivateRecordMembership,
  replaceLegacyResolverSnapshot,
} from "./ensv2ResolverProjection";
import {
  ENSv2Resolver,
  ENSv2ResolverABI,
  ENSv2ResolverAddress,
  ENSv2ResolverInterface,
  ENSv2ResolverLink,
  ENSv2ResolverRecord,
  ENSv2ResolverRecordData,
  ENSv2ResolverResourceArgument,
  ENSv2ResolverText,
} from "./types/schema";
import {
  ABIUpdated,
  AddressUpdated,
  ContenthashUpdated,
  DataUpdated,
  EACRolesChanged,
  InterfaceUpdated,
  Linked,
  NameUpdated,
  ResolverCreated,
  ResourceArgument,
  TextUpdated,
} from "./types/PermissionedResolver/PermissionedResolver";

// Shared with ensv2Paths.ts::handleResolverUpdated.
export function getOrCreateResolver(address: Address): ENSv2Resolver {
  let id: Bytes = address;
  let resolver = ENSv2Resolver.load(id);
  if (resolver == null) {
    resolver = new ENSv2Resolver(id);
    resolver.address = address;
    resolver.save();
  }
  return resolver;
}

function decodedNameOf(buf: Bytes): string | null {
  let decoded = decodeName(buf);
  if (decoded == null) {
    return null;
  }
  return decoded[1];
}

export function handleEACRolesChanged(event: EACRolesChanged): void {
  processEACRolesChanged(
    event.address,
    event.params.resource,
    event.params.account,
    event.params.oldRoleBitmap,
    event.params.newRoleBitmap,
    event.block,
    event.transaction.hash,
    event.logIndex,
  );
}

// See schema.graphql's comment above ENSv2ResolverRecord for what a recordId
// is. This is the event model in the deployed Sepolia implementation.

// Caller must never invoke with recordId.isZero() — 0 is the "no record"
// sentinel (see handleLinked), not a real record to get-or-create.
function getOrCreateRecord(
  resolver: ENSv2Resolver,
  recordId: BigInt,
  event: ethereum.Event,
): ENSv2ResolverRecord {
  let id = resolverRecordId(resolver.id, recordId);
  let record = ENSv2ResolverRecord.load(id);
  if (record == null) {
    record = new ENSv2ResolverRecord(id);
    record.resolver = resolver.id;
    record.recordId = recordId;
    record.linkIndexCount = 0;
    record.addressIndexCount = 0;
    record.textIndexCount = 0;
    record.createdAtBlock = event.block.number;
  }
  record.updatedAtBlock = event.block.number;
  record.save();
  return record;
}

function projectRecordToActiveMemberships(
  record: ENSv2ResolverRecord,
): void {
  for (let i = 0; i < record.linkIndexCount; i++) {
    let membership = activeRecordMembershipAt(record, i);
    if (membership != null) {
      replaceLegacyResolverSnapshot(record, membership.node);
    }
  }
}

// Also fires from the implementation constructor, creating an otherwise
// unused resolver row for the implementation address.
export function handleResolverCreated(event: ResolverCreated): void {
  getOrCreateResolver(event.address);
}

export function handleResourceArgument(event: ResourceArgument): void {
  let resolver = getOrCreateResolver(event.address);
  let id = resourceId(resolver.id, event.params.resource);
  let argument = ENSv2ResolverResourceArgument.load(id);
  if (argument == null) {
    argument = new ENSv2ResolverResourceArgument(id);
    argument.resolver = resolver.id;
    argument.resource = event.params.resource;
  }
  argument.arg = event.params.arg;
  argument.blockNumber = event.block.number;
  argument.transactionID = event.transaction.hash;
  argument.logIndex = event.logIndex;
  argument.save();
}

export function handleLinked(event: Linked): void {
  let resolver = getOrCreateResolver(event.address);
  let recordId = event.params.recordId;
  let id = resolverLinkId(resolver.id, event.params.node);

  let link = ENSv2ResolverLink.load(id);
  if (link != null) {
    let previousRecordId = link.record;
    if (previousRecordId) {
      let previousRecord = ENSv2ResolverRecord.load(previousRecordId);
      if (previousRecord != null) {
        deactivateRecordMembership(previousRecord, event.params.node);
      }
    }
  }
  if (link == null) {
    link = new ENSv2ResolverLink(id);
    link.resolver = resolver.id;
    link.node = event.params.node;
  }
  link.name = event.params.name;
  link.nameDecoded = decodedNameOf(event.params.name);
  link.recordId = recordId;
  // recordId 0 is the explicit-unlink sentinel (linkToRecord(name, 0)) —
  // not a real record, so no getOrCreateRecord call and no relation.
  if (recordId.isZero()) {
    link.record = null;
    link.active = false;
    link.blockNumber = event.block.number;
    link.transactionID = event.transaction.hash;
    link.logIndex = event.logIndex;
    link.save();
    clearLegacyResolverSnapshot(resolver.address, event.params.node);
  } else {
    let record = getOrCreateRecord(resolver, recordId, event);
    link.record = record.id;
    link.active = true;
    link.blockNumber = event.block.number;
    link.transactionID = event.transaction.hash;
    link.logIndex = event.logIndex;
    link.save();
    activateRecordMembership(record, link);
    replaceLegacyResolverSnapshot(record, event.params.node);
  }
}

export function handleContenthashUpdated(event: ContenthashUpdated): void {
  let resolver = getOrCreateResolver(event.address);
  let record = getOrCreateRecord(resolver, event.params.recordId, event);
  record.contenthash = event.params.hash;
  record.save();
  projectRecordToActiveMemberships(record);
}

export function handleNameUpdated(event: NameUpdated): void {
  let resolver = getOrCreateResolver(event.address);
  let record = getOrCreateRecord(resolver, event.params.recordId, event);
  record.primaryName = event.params.primaryName;
  record.save();
}

export function handleAddressUpdated(event: AddressUpdated): void {
  let resolver = getOrCreateResolver(event.address);
  let record = getOrCreateRecord(resolver, event.params.recordId, event);
  let id = resolverAddressId(record.id, event.params.coinType);
  let entity = ENSv2ResolverAddress.load(id);
  if (entity == null) {
    entity = new ENSv2ResolverAddress(id);
    entity.record = record.id;
    entity.coinType = event.params.coinType;
    appendAddressIndex(record, entity);
  }
  entity.addressBytes = event.params.addressBytes;
  entity.blockNumber = event.block.number;
  entity.transactionID = event.transaction.hash;
  entity.logIndex = event.logIndex;
  entity.save();
  projectRecordToActiveMemberships(record);
}

export function handleTextUpdated(event: TextUpdated): void {
  let resolver = getOrCreateResolver(event.address);
  let record = getOrCreateRecord(resolver, event.params.recordId, event);
  let id = resolverTextId(record.id, event.params.keyHash);
  let entity = ENSv2ResolverText.load(id);
  if (entity == null) {
    entity = new ENSv2ResolverText(id);
    entity.record = record.id;
    entity.keyHash = event.params.keyHash;
    appendTextIndex(record, entity);
  }
  entity.key = event.params.key;
  entity.value = event.params.value;
  entity.blockNumber = event.block.number;
  entity.transactionID = event.transaction.hash;
  entity.logIndex = event.logIndex;
  entity.save();
  projectRecordToActiveMemberships(record);
}

export function handleRecordDataUpdated(event: DataUpdated): void {
  let resolver = getOrCreateResolver(event.address);
  let record = getOrCreateRecord(resolver, event.params.recordId, event);
  let id = Bytes.fromByteArray(concat(record.id, event.params.keyHash));
  let entity = ENSv2ResolverRecordData.load(id);
  if (entity == null) {
    entity = new ENSv2ResolverRecordData(id);
    entity.record = record.id;
    entity.keyHash = event.params.keyHash;
  }
  entity.key = event.params.key;
  entity.value = event.params.value;
  entity.blockNumber = event.block.number;
  entity.transactionID = event.transaction.hash;
  entity.logIndex = event.logIndex;
  entity.save();
}

export function handleABIUpdated(event: ABIUpdated): void {
  let resolver = getOrCreateResolver(event.address);
  let record = getOrCreateRecord(resolver, event.params.recordId, event);
  let id = Bytes.fromByteArray(
    concat(record.id, uint256ToByteArray(event.params.contentType)),
  );
  let entity = ENSv2ResolverABI.load(id);
  if (entity == null) {
    entity = new ENSv2ResolverABI(id);
    entity.record = record.id;
    entity.contentType = event.params.contentType;
  }
  entity.blockNumber = event.block.number;
  entity.transactionID = event.transaction.hash;
  entity.logIndex = event.logIndex;
  entity.save();
}

export function handleInterfaceUpdated(event: InterfaceUpdated): void {
  let resolver = getOrCreateResolver(event.address);
  let record = getOrCreateRecord(resolver, event.params.recordId, event);
  let id = Bytes.fromByteArray(concat(record.id, event.params.interfaceId));
  let entity = ENSv2ResolverInterface.load(id);
  if (entity == null) {
    entity = new ENSv2ResolverInterface(id);
    entity.record = record.id;
    entity.interfaceId = event.params.interfaceId;
  }
  entity.implementer = event.params.implementer;
  entity.blockNumber = event.block.number;
  entity.transactionID = event.transaction.hash;
  entity.logIndex = event.logIndex;
  entity.save();
}
