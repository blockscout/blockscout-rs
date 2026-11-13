// Bounded compatibility projection for explicitly linked PermissionedResolver
// records. Native record/link entities remain authoritative; these helpers
// only touch a legacy Resolver when its Domain already exists and is currently
// configured to use the same resolver address.
import { Address, BigInt, Bytes } from "@graphprotocol/graph-ts";
import { createResolverID } from "./resolver";
import { createOrLoadAccount } from "./utils";
import {
  resolverAddressIndexId,
  resolverLinkId,
  resolverLinkIndexId,
  resolverMembershipId,
  resolverTextIndexId,
} from "./ensv2Utils";
import {
  Domain,
  ENSv2Resolver,
  ENSv2ResolverAddress,
  ENSv2ResolverAddressIndex,
  ENSv2ResolverLink,
  ENSv2ResolverRecord,
  ENSv2ResolverRecordLinkIndex,
  ENSv2ResolverRecordLinkMembership,
  ENSv2ResolverText,
  ENSv2ResolverTextIndex,
  Resolver,
} from "./types/schema";

const ETH_COIN_TYPE = BigInt.fromI32(60);
// ENSIP-19 default EVM coin type (1 << 31).
const DEFAULT_EVM_COIN_TYPE = BigInt.fromString("2147483648");

export function activateRecordMembership(
  record: ENSv2ResolverRecord,
  link: ENSv2ResolverLink,
): ENSv2ResolverRecordLinkMembership {
  let id = resolverMembershipId(record.id, link.node);
  let membership = ENSv2ResolverRecordLinkMembership.load(id);
  if (membership == null) {
    membership = new ENSv2ResolverRecordLinkMembership(id);
    membership.record = record.id;
    membership.link = link.id;
    membership.node = link.node;
    membership.index = record.linkIndexCount;

    let index = new ENSv2ResolverRecordLinkIndex(
      resolverLinkIndexId(record.id, record.linkIndexCount),
    );
    index.record = record.id;
    index.index = record.linkIndexCount;
    index.membership = membership.id;
    index.save();

    record.linkIndexCount = record.linkIndexCount + 1;
    record.save();
  }
  membership.link = link.id;
  membership.active = true;
  membership.save();
  return membership;
}

export function deactivateRecordMembership(
  record: ENSv2ResolverRecord,
  node: Bytes,
): void {
  let membership = ENSv2ResolverRecordLinkMembership.load(
    resolverMembershipId(record.id, node),
  );
  if (membership == null) {
    return;
  }
  if (!membership.active) {
    return;
  }
  membership.active = false;
  membership.save();
}

// Loads one defensively validated current member. A stale index or membership
// is ignored unless the live link still points at this exact record.
export function activeRecordMembershipAt(
  record: ENSv2ResolverRecord,
  indexNumber: i32,
): ENSv2ResolverRecordLinkMembership | null {
  let index = ENSv2ResolverRecordLinkIndex.load(
    resolverLinkIndexId(record.id, indexNumber),
  );
  if (index == null) {
    return null;
  }
  let membership = ENSv2ResolverRecordLinkMembership.load(index.membership);
  if (membership == null) {
    return null;
  }
  if (!membership.active) {
    return null;
  }
  let link = ENSv2ResolverLink.load(membership.link);
  if (link == null) {
    return null;
  }
  if (!link.active) {
    return null;
  }
  let linkedRecord = link.record;
  if (!linkedRecord) {
    return null;
  }
  if (!linkedRecord.equals(record.id)) {
    return null;
  }
  return membership;
}

export function appendAddressIndex(
  record: ENSv2ResolverRecord,
  address: ENSv2ResolverAddress,
): void {
  address.index = record.addressIndexCount;
  let index = new ENSv2ResolverAddressIndex(
    resolverAddressIndexId(record.id, record.addressIndexCount),
  );
  index.record = record.id;
  index.index = record.addressIndexCount;
  index.address = address.id;
  index.save();
  record.addressIndexCount = record.addressIndexCount + 1;
  record.save();
}

export function appendTextIndex(
  record: ENSv2ResolverRecord,
  text: ENSv2ResolverText,
): void {
  text.index = record.textIndexCount;
  let index = new ENSv2ResolverTextIndex(
    resolverTextIndexId(record.id, record.textIndexCount),
  );
  index.record = record.id;
  index.index = record.textIndexCount;
  index.text = text.id;
  index.save();
  record.textIndexCount = record.textIndexCount + 1;
  record.save();
}

// Returns null unless the legacy compatibility projection is already valid
// for this node. This guard prevents resolver events from materialising a
// Domain or writing through a stale registry resolver association.
export function loadEligibleLegacyResolver(
  resolverAddress: Bytes,
  node: Bytes,
): Resolver | null {
  let domain = Domain.load(node.toHexString());
  if (domain == null) {
    return null;
  }
  let configuredResolver = domain.resolver;
  if (!configuredResolver) {
    return null;
  }
  let id = createResolverID(node, Address.fromBytes(resolverAddress));
  if (!(configuredResolver! == id)) {
    return null;
  }
  let resolver = Resolver.load(id);
  if (resolver == null) {
    resolver = new Resolver(id);
    resolver.domain = domain.id;
    resolver.address = resolverAddress;
    resolver.save();
  }
  return resolver;
}

export function clearLegacyResolverSnapshot(
  resolverAddress: Bytes,
  node: Bytes,
): void {
  let legacy = loadEligibleLegacyResolver(resolverAddress, node);
  if (legacy == null) {
    return;
  }
  legacy.addr = null;
  legacy.contentHash = null;
  legacy.texts = null;
  legacy.coinTypes = null;
  legacy.save();

  let domain = Domain.load(node.toHexString());
  if (domain != null) {
    domain.resolvedAddress = null;
    domain.save();
  }
}

export function clearDomainResolver(node: Bytes): void {
  let domain = Domain.load(node.toHexString());
  if (domain == null) {
    return;
  }
  domain.resolver = null;
  domain.resolvedAddress = null;
  domain.save();
}

export function attachDomainResolver(
  resolverAddress: Address,
  node: Bytes,
): void {
  let domain = Domain.load(node.toHexString());
  if (domain == null) {
    return;
  }
  let id = createResolverID(node, resolverAddress);
  let legacy = Resolver.load(id);
  if (legacy == null) {
    legacy = new Resolver(id);
    legacy.domain = domain.id;
    legacy.address = resolverAddress;
    legacy.save();
  }
  domain.resolver = legacy.id;

  let link = ENSv2ResolverLink.load(resolverLinkId(resolverAddress, node));
  if (link == null) {
    // No ENSv2ResolverLink means this is not a PermissionedResolver record
    // we project from native entities; it is a classic node-keyed resolver
    // (PublicResolverV2 and friends) whose records the ENSv1 resolver
    // handlers in resolver.ts already keep on the legacy Resolver row. Same
    // behaviour as handleNewResolver in ensRegistry.ts: keep the snapshot
    // and carry its addr over to the Domain rather than wiping it.
    domain.resolvedAddress = legacy.addr;
    domain.save();
    return;
  }
  domain.save();

  if (link.active) {
    let recordId = link.record;
    if (recordId) {
      let record = ENSv2ResolverRecord.load(recordId);
      if (record != null) {
        replaceLegacyResolverSnapshot(record, node);
        return;
      }
    }
  }
  // The link exists but is explicitly inactive (linkToRecord(name, 0)) or
  // points at nothing: the native side says this name has no record here,
  // so the legacy projection must say the same.
  clearLegacyResolverSnapshot(resolverAddress, node);
}

export function replaceLegacyResolverSnapshot(
  record: ENSv2ResolverRecord,
  node: Bytes,
): void {
  let nativeResolver = ENSv2Resolver.load(record.resolver);
  if (nativeResolver == null) {
    return;
  }
  let legacy = loadEligibleLegacyResolver(nativeResolver.address, node);
  if (legacy == null) {
    return;
  }

  legacy.contentHash = record.contenthash;
  legacy.addr = null;

  let coinTypes = new Array<BigInt>();
  // Empty means unset, as on chain. (Non-null locals: comparing nullable
  // Bytes against null crashes the AssemblyScript compiler.)
  let ethBytes = new Bytes(0);
  let defaultBytes = new Bytes(0);
  for (let i = 0; i < record.addressIndexCount; i++) {
    let index = ENSv2ResolverAddressIndex.load(
      resolverAddressIndexId(record.id, i),
    );
    if (index == null) {
      continue;
    }
    let address = ENSv2ResolverAddress.load(index.address);
    if (address == null) {
      continue;
    }
    coinTypes.push(address.coinType);
    if (address.coinType.equals(ETH_COIN_TYPE)) {
      ethBytes = address.addressBytes;
    } else if (address.coinType.equals(DEFAULT_EVM_COIN_TYPE)) {
      defaultBytes = address.addressBytes;
    }
  }
  // addr() as PermissionedResolver answers it (AbstractRecordResolver
  // _getAddress): the coin type 60 address, or the ENSIP-19 default EVM
  // address when coin type 60 is unset or empty. A non-empty coin type 60
  // value, even the zero address, overrides the default.
  let effective = ethBytes.length > 0 ? ethBytes : defaultBytes;
  if (effective.length == 20) {
    legacy.addr = createOrLoadAccount(effective).id;
  }
  if (coinTypes.length == 0) {
    legacy.coinTypes = null;
  } else {
    legacy.coinTypes = coinTypes;
  }

  let texts = new Array<string>();
  for (let i = 0; i < record.textIndexCount; i++) {
    let index = ENSv2ResolverTextIndex.load(
      resolverTextIndexId(record.id, i),
    );
    if (index == null) {
      continue;
    }
    let text = ENSv2ResolverText.load(index.text);
    if (text != null) {
      texts.push(text.key);
    }
  }
  if (texts.length == 0) {
    legacy.texts = null;
  } else {
    legacy.texts = texts;
  }
  legacy.save();

  let domain = Domain.load(node.toHexString());
  if (domain != null) {
    domain.resolvedAddress = legacy.addr;
    domain.save();
  }
}
