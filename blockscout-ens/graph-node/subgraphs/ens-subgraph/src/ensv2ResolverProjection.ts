// Bounded compatibility projection for explicitly linked PermissionedResolver
// records. Native record/link entities remain authoritative; these helpers
// only touch a legacy Resolver when its Domain already exists and is currently
// configured to use the same resolver address.
//
// The PermissionedResolver data source has no address filter: any contract
// can emit these events, as often and for as many nodes as it likes. One
// record update therefore refreshes at most MAX_LEGACY_FANOUT names, and
// builds the record's legacy snapshot once however many names share it.
//
// A legacy snapshot must never be out of date: a Domain may show no address,
// never a wrong one. So a name is either tracked, and refreshed by every
// update of the record it resolves through, or it is never given a snapshot
// at all (recordIsTrackedFor, defaultRecordIsTrackedFor). Which it is does
// not change: a link's position in its record and a name's position among
// the resolver's attachments are fixed when they are created. A resolver
// with untracked names is flagged (legacyProjectionTruncated).
import { Address, BigInt, Bytes } from "@graphprotocol/graph-ts";
import { createResolverID } from "./resolver";
import { concat, createOrLoadAccount, i32ToBytes, ROOT_NODE } from "./utils";
import { getEnsV1ResolverAddress } from "./ensv2Constants";
import { countsAsChild, saveCounted } from "./subdomainCount";
import {
  resolverAddressId,
  resolverAddressIndexId,
  resolverLinkId,
  resolverLinkIndexId,
  resolverMembershipId,
  resolverTextIndexId,
} from "./ensv2Utils";
import {
  Domain,
  ENSv2Resolver,
  ENSv2ResolverAttachment,
  ENSv2ResolverAttachmentIndex,
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

// Names one event may refresh on the legacy side: the first this many links
// of the updated record, and the first this many names pointed at the
// resolver when its default record changes. Names beyond that are not
// projected from the record at all.
export const MAX_LEGACY_FANOUT = 100;

// Coin types and text keys of a record that its legacy snapshot lists
// (Resolver.coinTypes and Resolver.texts). The source has no address filter
// and a record keeps an index row for every coin type and key it was ever
// given, so without a limit each update of a record would walk a list that
// any contract can lengthen by one per event. The resolved address does not
// depend on it: it is read by coin type. A record past the limit lists its
// first MAX_LEGACY_SNAPSHOT_KEYS and its resolver is flagged
// (ENSv2Resolver.legacyProjectionTruncated).
export const MAX_LEGACY_SNAPSHOT_KEYS = 1000;

// A text key longer than this is left out of the snapshot's list, and the
// resolver flagged: the list is copied onto the legacy Resolver row of every
// name that follows the record, and a key has no length limit on chain.
export const MAX_LEGACY_TEXT_KEY_LENGTH = 255;

let maxLegacySnapshotKeys: i32 = MAX_LEGACY_SNAPSHOT_KEYS;

// Tests only: reaching the real limit means rebuilding a thousand-entry
// snapshot a thousand times.
export function setMaxLegacySnapshotKeysForTesting(limit: i32): void {
  maxLegacySnapshotKeys = limit;
}

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

// A row can be live through its resolver alone (a retired ENSv2 name that
// ENSv1 gave a resolver afterwards), so taking the resolver off can take it
// out of its parent's subdomainCount (subdomainCount.ts).
export function clearDomainResolver(node: Bytes): void {
  let domain = Domain.load(node.toHexString());
  if (domain == null) {
    return;
  }
  let wasCounted = countsAsChild(domain);
  domain.resolver = null;
  domain.resolvedAddress = null;
  saveCounted(domain, wasCounted);
}

// Shared by the ENSv2 registry and PermissionedResolver mappings.
export function getOrCreateResolver(address: Address): ENSv2Resolver {
  let id: Bytes = address;
  let resolver = ENSv2Resolver.load(id);
  if (resolver == null) {
    resolver = new ENSv2Resolver(id);
    resolver.address = address;
    resolver.recordBased = false;
    resolver.attachmentCount = 0;
    resolver.legacyProjectionTruncated = false;
    resolver.save();
  }
  return resolver;
}

// `rememberAnyResolver`: remember the name for the resolver's default record
// whatever the resolver turns out to be (recordResolverAttachment). True for
// an ENSv2 registry (ResolverUpdated, or the late-link backfill) and for an
// ENSv1 NewResolver from the first ENSv2 block on.
export function attachDomainResolver(
  resolverAddress: Address,
  node: Bytes,
  rememberAnyResolver: boolean = false,
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
  let wasCounted = countsAsChild(domain);
  domain.resolver = legacy.id;
  saveCounted(domain, wasCounted);

  recordResolverAttachment(resolverAddress, node, rememberAnyResolver);
  if (projectEffectiveRecord(resolverAddress, node)) {
    return;
  }
  // Neither the node nor the resolver's default has a link: this is a
  // classic node-keyed resolver (PublicResolverV2 and friends) whose records
  // the ENSv1 resolver handlers in resolver.ts already keep on the legacy
  // Resolver row. Same behaviour as handleNewResolver in ensRegistry.ts:
  // keep the snapshot and carry its addr over to the Domain.
  domain.resolvedAddress = legacy.addr;
  domain.save();
}

// Projects the record a PermissionedResolver answers with for `node`, as
// `_record()` picks it: the node's own linked record, else the default
// record (linked to the root node), else nothing. Returns false when the
// address has no link for the node and no default link, i.e. it is not
// using the recordId model for this node.
export function projectEffectiveRecord(
  resolverAddress: Address,
  node: Bytes,
): boolean {
  let resolverId: Bytes = resolverAddress;
  let own = ENSv2ResolverLink.load(resolverLinkId(resolverId, node));
  if (own != null && own.active) {
    let ownRecordId = own.record;
    if (ownRecordId) {
      let record = ENSv2ResolverRecord.load(ownRecordId);
      if (record != null) {
        if (recordIsTrackedFor(record, node)) {
          replaceLegacyResolverSnapshot(record, node);
        } else if (loadEligibleLegacyResolver(resolverAddress, node) != null) {
          clearLegacyResolverSnapshot(resolverAddress, node);
          flagLegacyProjectionTruncated(resolverId);
        }
        return true;
      }
    }
  }
  let fallback: ENSv2ResolverLink | null = null;
  if (!node.equals(ROOT_NODE)) {
    fallback = ENSv2ResolverLink.load(resolverLinkId(resolverId, ROOT_NODE));
  }
  if (fallback != null && fallback.active) {
    let fallbackRecordId = fallback.record;
    if (fallbackRecordId) {
      let record = ENSv2ResolverRecord.load(fallbackRecordId);
      if (record != null) {
        // Nothing to write, and nothing to remember, for a node whose
        // Domain does not point at this resolver.
        if (loadEligibleLegacyResolver(resolverAddress, node) == null) {
          return true;
        }
        if (defaultRecordIsTrackedFor(resolverAddress, node)) {
          replaceLegacyResolverSnapshot(record, node);
        } else {
          clearLegacyResolverSnapshot(resolverAddress, node);
          flagLegacyProjectionTruncated(resolverId);
        }
        return true;
      }
    }
  }
  if (own == null && fallback == null) {
    return false;
  }
  // The link exists but is explicitly inactive (linkToRecord(name, 0)) or
  // points at nothing, and there is no default record: the native side says
  // this name has no record here, so the legacy projection must say the same.
  clearLegacyResolverSnapshot(resolverAddress, node);
  return true;
}

// Whether updates of `record` reach `node`: its link is among the first
// MAX_LEGACY_FANOUT of the record (projectRecordToActiveMemberships).
function recordIsTrackedFor(record: ENSv2ResolverRecord, node: Bytes): boolean {
  let membership = ENSv2ResolverRecordLinkMembership.load(
    resolverMembershipId(record.id, node),
  );
  return membership != null && membership.index < MAX_LEGACY_FANOUT;
}

// Whether changes of the resolver's default record reach `node`: it is among
// the first MAX_LEGACY_FANOUT names pointed at the resolver
// (refreshDefaultRecordConsumers). A name that came to the resolver before
// it had any link was not remembered then (see recordResolverAttachment);
// it is now, so that it is tracked before it is given the default record.
function defaultRecordIsTrackedFor(resolverAddress: Address, node: Bytes): boolean {
  if (node.equals(ROOT_NODE)) {
    return true;
  }
  recordResolverAttachment(resolverAddress, node, false);
  let resolverId: Bytes = resolverAddress;
  let attachment = ENSv2ResolverAttachment.load(
    Bytes.fromByteArray(concat(resolverId, node)),
  );
  return attachment != null && attachment.index < MAX_LEGACY_FANOUT;
}

// Remembers the names pointed at a resolver, so a change of a
// PermissionedResolver's default record reaches the names that rely on it.
//
// A name can select a PermissionedResolver's address before the proxy is
// deployed (the address is counterfactual), and so before it emits anything
// that marks it record-based. Such an attachment (`rememberAnyResolver`) is
// therefore remembered whatever the resolver turns out to be, except the
// ENSV1Resolver that names reserved for migration point at: every ENSv2
// registry attachment, and every ENSv1 NewResolver from the first ENSv2
// block on (getEnsv2FirstBlock; about 12,000 a month on mainnet that do not
// clear the resolver). Before that block no PermissionedResolver can exist,
// so an ENSv1 attachment is remembered only for a resolver already known to
// be record-based: remembering every ENSv1 name since 2017 would index
// millions of classic-resolver names that can never have a default record.
function recordResolverAttachment(
  resolverAddress: Address,
  node: Bytes,
  rememberAnyResolver: boolean,
): void {
  if (node.equals(ROOT_NODE)) {
    return;
  }
  let resolver: ENSv2Resolver | null = null;
  if (rememberAnyResolver) {
    if (resolverAddress.equals(getEnsV1ResolverAddress())) {
      return;
    }
    resolver = getOrCreateResolver(resolverAddress);
  } else {
    resolver = ENSv2Resolver.load(resolverAddress);
    if (resolver == null || !resolver.recordBased) {
      return;
    }
  }
  if (resolver == null) {
    return;
  }
  let id = Bytes.fromByteArray(concat(resolver.id, node));
  if (ENSv2ResolverAttachment.load(id) != null) {
    return;
  }
  let attachment = new ENSv2ResolverAttachment(id);
  attachment.resolver = resolver.id;
  attachment.node = node;
  attachment.index = resolver.attachmentCount;
  attachment.save();

  let index = new ENSv2ResolverAttachmentIndex(
    Bytes.fromByteArray(concat(resolver.id, i32ToBytes(resolver.attachmentCount))),
  );
  index.resolver = resolver.id;
  index.index = resolver.attachmentCount;
  index.attachment = attachment.id;
  index.save();
  resolver.attachmentCount = resolver.attachmentCount + 1;
  resolver.save();
}

function flagLegacyProjectionTruncated(resolverId: Bytes): void {
  let resolver = ENSv2Resolver.load(resolverId);
  if (resolver != null && !resolver.legacyProjectionTruncated) {
    resolver.legacyProjectionTruncated = true;
    resolver.save();
  }
}

// A record changed: refresh the legacy snapshot of the names linked to it,
// the first MAX_LEGACY_FANOUT of them, which are the tracked ones
// (recordIsTrackedFor); the others have no snapshot to refresh. If one of
// them is the root node the record is the resolver's default, and the names
// relying on it follow.
export function projectRecordToActiveMemberships(
  record: ENSv2ResolverRecord,
): void {
  let count = record.linkIndexCount;
  if (count > MAX_LEGACY_FANOUT) {
    count = MAX_LEGACY_FANOUT;
    flagLegacyProjectionTruncated(record.resolver);
  }
  let snapshot: LegacyRecordSnapshot | null = null;
  for (let i = 0; i < count; i++) {
    let membership = activeRecordMembershipAt(record, i);
    if (membership != null) {
      snapshot = replaceLegacyResolverSnapshot(record, membership.node, snapshot);
    }
  }
  // Whether the record is the resolver's default is read from the root
  // link itself, not from the capped list above: a record linked to the
  // root node after MAX_LEGACY_FANOUT other names is still the default, and
  // the names relying on it were given its snapshot (projectEffectiveRecord)
  // and must follow its updates.
  let rootLink = ENSv2ResolverLink.load(resolverLinkId(record.resolver, ROOT_NODE));
  if (rootLink != null && rootLink.active) {
    let rootRecordId = rootLink.record;
    if (rootRecordId && rootRecordId.equals(record.id)) {
      refreshDefaultRecordConsumers(record.resolver);
    }
  }
}

// The default record changed (relinked, unlinked or updated): re-project
// the names on this resolver that have no active record of its own, the
// first MAX_LEGACY_FANOUT ever pointed at it, which are the tracked ones
// (defaultRecordIsTrackedFor). Same outcome per name as
// projectEffectiveRecord, with the default record loaded and its snapshot
// built once. Only a Domain that still points at this resolver is written.
export function refreshDefaultRecordConsumers(resolverId: Bytes): void {
  let resolver = ENSv2Resolver.load(resolverId);
  if (resolver == null) {
    return;
  }
  let resolverAddress = Address.fromBytes(resolver.address);
  let count = resolver.attachmentCount;
  if (count > MAX_LEGACY_FANOUT) {
    count = MAX_LEGACY_FANOUT;
    flagLegacyProjectionTruncated(resolver.id);
  }

  let fallback = ENSv2ResolverLink.load(resolverLinkId(resolver.id, ROOT_NODE));
  let defaultRecord: ENSv2ResolverRecord | null = null;
  if (fallback != null && fallback.active) {
    let fallbackRecordId = fallback.record;
    if (fallbackRecordId) {
      defaultRecord = ENSv2ResolverRecord.load(fallbackRecordId);
    }
  }

  let snapshot: LegacyRecordSnapshot | null = null;
  for (let i = 0; i < count; i++) {
    let index = ENSv2ResolverAttachmentIndex.load(
      Bytes.fromByteArray(concat(resolver.id, i32ToBytes(i))),
    );
    if (index == null) {
      continue;
    }
    let attachment = ENSv2ResolverAttachment.load(index.attachment);
    if (attachment == null) {
      continue;
    }
    let own = ENSv2ResolverLink.load(resolverLinkId(resolver.id, attachment.node));
    if (own != null && own.active) {
      continue;
    }
    if (defaultRecord != null) {
      snapshot = replaceLegacyResolverSnapshot(
        defaultRecord,
        attachment.node,
        snapshot,
      );
    } else if (own != null || fallback != null) {
      // Unlinked, and no default record to fall back on.
      clearLegacyResolverSnapshot(resolverAddress, attachment.node);
    }
  }
}

// The legacy fields a record projects. They depend on the record alone, so
// one snapshot serves every name that uses the record.
class LegacyRecordSnapshot {
  contentHash: Bytes | null = null;
  addr: string | null = null;
  coinTypes: Array<BigInt> | null = null;
  texts: Array<string> | null = null;
}

function buildLegacyRecordSnapshot(
  record: ENSv2ResolverRecord,
): LegacyRecordSnapshot {
  let snapshot = new LegacyRecordSnapshot();
  snapshot.contentHash = record.contenthash;

  let truncated = false;
  let coinTypes = new Array<BigInt>();
  let addressCount = record.addressIndexCount;
  if (addressCount > maxLegacySnapshotKeys) {
    addressCount = maxLegacySnapshotKeys;
    truncated = true;
  }
  for (let i = 0; i < addressCount; i++) {
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
  }
  // The two coin types addr() is answered from, read by id so that neither
  // depends on where it sits in the list. Empty means unset, as on chain.
  // (Non-null locals: comparing nullable Bytes against null crashes the
  // AssemblyScript compiler.)
  let ethBytes = new Bytes(0);
  let defaultBytes = new Bytes(0);
  let ethAddress = ENSv2ResolverAddress.load(
    resolverAddressId(record.id, ETH_COIN_TYPE),
  );
  if (ethAddress != null) {
    ethBytes = ethAddress.addressBytes;
  }
  let defaultAddress = ENSv2ResolverAddress.load(
    resolverAddressId(record.id, DEFAULT_EVM_COIN_TYPE),
  );
  if (defaultAddress != null) {
    defaultBytes = defaultAddress.addressBytes;
  }
  // addr() as PermissionedResolver answers it (AbstractRecordResolver
  // _getAddress): the coin type 60 address, or the ENSIP-19 default EVM
  // address when coin type 60 is unset or empty. A non-empty coin type 60
  // value, even the zero address, overrides the default.
  let effective = ethBytes.length > 0 ? ethBytes : defaultBytes;
  if (effective.length == 20) {
    snapshot.addr = createOrLoadAccount(effective).id;
  }
  if (coinTypes.length > 0) {
    snapshot.coinTypes = coinTypes;
  }

  let texts = new Array<string>();
  let textCount = record.textIndexCount;
  if (textCount > maxLegacySnapshotKeys) {
    textCount = maxLegacySnapshotKeys;
    truncated = true;
  }
  for (let i = 0; i < textCount; i++) {
    let index = ENSv2ResolverTextIndex.load(
      resolverTextIndexId(record.id, i),
    );
    if (index == null) {
      continue;
    }
    let text = ENSv2ResolverText.load(index.text);
    if (text != null) {
      if (text.key.length > MAX_LEGACY_TEXT_KEY_LENGTH) {
        truncated = true;
      } else {
        texts.push(text.key);
      }
    }
  }
  if (truncated) {
    flagLegacyProjectionTruncated(record.resolver);
  }
  if (texts.length > 0) {
    snapshot.texts = texts;
  }
  return snapshot;
}

// Writes the record's legacy snapshot to `node`, if the legacy projection is
// valid for it (loadEligibleLegacyResolver). Pass the snapshot returned by a
// previous call for the same record to reuse it; it is built on the first
// eligible name, so a record nobody's Domain uses costs nothing.
export function replaceLegacyResolverSnapshot(
  record: ENSv2ResolverRecord,
  node: Bytes,
  snapshot: LegacyRecordSnapshot | null = null,
): LegacyRecordSnapshot | null {
  let nativeResolver = ENSv2Resolver.load(record.resolver);
  if (nativeResolver == null) {
    return snapshot;
  }
  let legacy = loadEligibleLegacyResolver(nativeResolver.address, node);
  if (legacy == null) {
    return snapshot;
  }
  if (snapshot == null) {
    snapshot = buildLegacyRecordSnapshot(record);
  }

  legacy.contentHash = snapshot.contentHash;
  legacy.addr = snapshot.addr;
  legacy.coinTypes = snapshot.coinTypes;
  legacy.texts = snapshot.texts;
  legacy.save();

  let domain = Domain.load(node.toHexString());
  if (domain != null) {
    domain.resolvedAddress = legacy.addr;
    domain.save();
  }
  return snapshot;
}
