// Namespace/path materialisation.
//
//   materializePathsForSlot (called from ensv2Registry.ts::handleLabelRegistered)
//   creates paths by looping over the REGISTERING slot's own registry's
//   existing namespaces (bounded by registry.namespaceCount).
//
//   handleSubregistryUpdated creates namespaces by looping over the PARENT
//   slot's existing paths (bounded by parentSlot.pathCount). A namespace
//   that is new or reactivated is then backfilled (backfillNamespace): every
//   name already registered in the child registry gets its path and Domain
//   projection, and so do the registries those names link to in turn. This
//   is the subname copy flow (deploy a registry, register the names, then
//   point the parent at it), where every name is registered before the link.
//   The backfill is bounded by the child registry's slotCount per level and
//   by MAX_BACKFILL_DEPTH levels.
import { Address, BigInt, Bytes, ethereum, log } from "@graphprotocol/graph-ts";

import { checkValidLabel, createEventID } from "./utils";
import {
  appendRegistryNamespaceIndex,
  isZeroAddress,
  nameSlotId,
  namespaceId,
  namespaceLinkId,
  namespacePathIndexId,
  pathNamehash,
  pathNamespaceIndexId,
  registryNamespaceIndexId,
  registrySlotIndexId,
  slotPathIndexId,
  toSlotId,
} from "./ensv2Utils";
import { getOrCreateRegistry } from "./ensv2Discovery";
import { getOrCreateResolver } from "./ensv2Resolver";
import {
  attachDomainResolver,
  clearDomainResolver,
} from "./ensv2ResolverProjection";
import {
  adjustParentSubdomainCount,
  projectPathToDomain,
  retireProjectedDomain,
} from "./ensv2Domain";
import {
  ENSv2NamePath,
  ENSv2Namespace,
  ENSv2NamespaceLink,
  ENSv2NamespacePathIndex,
  ENSv2NameSlot,
  ENSv2PathNamespaceIndex,
  ENSv2Registry,
  ENSv2RegistryNamespaceIndex,
  ENSv2RegistrySlotIndex,
  ENSv2ResolverUpdate,
  ENSv2Resource,
  ENSv2SlotPathIndex,
  ENSv2SubregistryUpdate,
  ENSv2Token,
} from "./types/schema";
import {
  ParentUpdated,
  ResolverUpdated,
  SubregistryUpdated,
} from "./types/RootRegistry/PermissionedRegistry";


// Names nested deeper than this below a (de)activated link are left as
// they are. Guards against registries that (directly or indirectly) link
// to themselves, whose namehashes would otherwise grow without end.
const MAX_BACKFILL_DEPTH = 16;

// The append helpers below allocate from a fresh load, for the same reason
// as appendRegistryNamespaceIndex (ensv2Utils.ts), and sync the caller's
// snapshot.
function appendSlotPathIndex(slot: ENSv2NameSlot, path: ENSv2NamePath): void {
  let current = ENSv2NameSlot.load(slot.id);
  if (current == null) {
    current = slot;
  }
  let next = current.pathCount;
  let index = new ENSv2SlotPathIndex(slotPathIndexId(current.id, next));
  index.slot = current.id;
  index.index = next;
  index.path = path.id;
  index.save();

  current.pathCount = next + 1;
  current.save();
  slot.pathCount = next + 1;
}

// A registration silently resets its slot resolver even when no
// ResolverUpdated(..., address(0)) event is emitted. Clear every previously
// materialised compatibility Domain before any later same-transaction event
// can attach a new resolver.
export function clearProjectedDomainResolversForSlot(
  slot: ENSv2NameSlot,
): void {
  for (let i = 0; i < slot.pathCount; i++) {
    let pathIndex = ENSv2SlotPathIndex.load(slotPathIndexId(slot.id, i));
    if (pathIndex == null) {
      continue;
    }
    let path = ENSv2NamePath.load(pathIndex.path);
    // Skip paths another registry's slot has since taken over (see
    // materializePathsForSlot).
    if (path != null && path.domain != null && path.slot.equals(slot.id)) {
      clearDomainResolver(path.namehash);
    }
  }
}

function appendPathNamespaceIndex(
  path: ENSv2NamePath,
  namespace: ENSv2Namespace
): void {
  let current = ENSv2NamePath.load(path.id);
  if (current == null) {
    current = path;
  }
  let next = current.namespaceCount;
  let index = new ENSv2PathNamespaceIndex(pathNamespaceIndexId(current.id, next));
  index.path = current.id;
  index.index = next;
  index.namespace = namespace.id;
  index.save();

  current.namespaceCount = next + 1;
  current.save();
  path.namespaceCount = next + 1;
}

// Reverse of appendPathNamespaceIndex above — lets deactivatePathsForNamespace
// below find every path materialised under a namespace in bounded time when
// that namespace is deactivated, the same way ENSv2SlotPathIndex
// already lets handleLabelUnregistered (ensv2Registry.ts) deactivate a
// slot's own paths. Called once, at path creation, alongside
// appendSlotPathIndex — a path's (namespace, slot) pair is fixed for its
// whole life, so it only ever needs indexing once.
function appendNamespacePathIndex(
  namespace: ENSv2Namespace,
  path: ENSv2NamePath
): void {
  let current = ENSv2Namespace.load(namespace.id);
  if (current == null) {
    current = namespace;
  }
  let next = current.pathCount;
  let index = new ENSv2NamespacePathIndex(namespacePathIndexId(current.id, next));
  index.namespace = current.id;
  index.index = next;
  index.path = path.id;
  index.save();

  current.pathCount = next + 1;
  current.save();
  namespace.pathCount = next + 1;
}

// Idempotent: sets active = true whether creating or reactivating. Caller
// checks pre-existence (ENSv2Namespace.load(id) == null, before calling
// this) to decide whether to append indices — reactivation must never
// re-append.
function createOrReactivateNamespace(
  childRegistry: ENSv2Registry,
  parentSlot: ENSv2NameSlot,
  parentPath: ENSv2NamePath,
  parentTokenId: BigInt,
  event: ethereum.Event
): ENSv2Namespace {
  let id = namespaceId(childRegistry.id, parentPath.namehash);
  let namespace = ENSv2Namespace.load(id);
  if (namespace == null) {
    namespace = new ENSv2Namespace(id);
    namespace.registry = childRegistry.id;
    namespace.pathCount = 0;
    namespace.createdAt = event.block.timestamp;
    namespace.createdAtBlock = event.block.number;
  }
  namespace.parentPath = parentPath.id;
  namespace.parentSlot = parentSlot.id;
  namespace.parentRegistry = parentSlot.registry;
  namespace.parentSlotId = parentSlot.slotId;
  namespace.parentTokenId = parentTokenId;
  let parentResourceId = parentSlot.currentResource;
  if (parentResourceId) {
    let resourceEntity = ENSv2Resource.load(parentResourceId);
    if (resourceEntity != null) {
      namespace.parentResource = resourceEntity.resource;
    }
  }
  namespace.baseName = parentPath.name;
  namespace.baseNamehash = parentPath.namehash;
  namespace.active = true;
  namespace.updatedAtBlock = event.block.number;
  namespace.transactionID = event.transaction.hash;
  namespace.logIndex = event.logIndex;
  namespace.save();
  return namespace;
}

function upsertNamespaceLink(
  parentRegistryId: Bytes,
  parentSlot: ENSv2NameSlot,
  previousChildAddress: Bytes | null,
  event: SubregistryUpdated
): void {
  // A slot can only point at one subregistry at a time. Deactivate the
  // superseded link and narrow nullable Bytes before calling .equals().
  if (previousChildAddress) {
    let newChildAddress: Bytes = isZeroAddress(event.params.subregistry)
      ? Bytes.empty()
      : event.params.subregistry;
    let isSameTarget = previousChildAddress.equals(newChildAddress);
    if (!isSameTarget) {
      let oldLinkId = namespaceLinkId(
        parentRegistryId,
        parentSlot.slotId,
        previousChildAddress
      );
      let oldLink = ENSv2NamespaceLink.load(oldLinkId);
      if (oldLink != null) {
        oldLink.active = false;
        oldLink.save();
      }
    }
  }

  if (isZeroAddress(event.params.subregistry)) {
    return;
  }

  let childAddress = event.params.subregistry;
  let linkId = namespaceLinkId(parentRegistryId, parentSlot.slotId, childAddress);
  let link = ENSv2NamespaceLink.load(linkId);
  if (link == null) {
    link = new ENSv2NamespaceLink(linkId);
    link.parentRegistry = parentRegistryId;
    link.parentSlot = parentSlot.id;
    link.parentSlotId = parentSlot.slotId;
    link.childRegistryAddress = event.params.subregistry;
    link.childRegistry = childAddress;
  }
  link.parentTokenId = event.params.tokenId;
  // Keep the link's parent resource aligned with its namespace metadata.
  let parentResourceId = parentSlot.currentResource;
  if (parentResourceId) {
    let resourceEntity = ENSv2Resource.load(parentResourceId);
    if (resourceEntity != null) {
      link.parentResource = resourceEntity.resource;
    }
  }
  link.active = true;
  link.transactionID = event.transaction.hash;
  link.blockNumber = event.block.number;
  link.logIndex = event.logIndex;
  link.save();
}

// Reconstructs the same namespace ids the creation loop would have produced
// (childRegistry + each existing path's namehash) and deactivates them —
// never deletes.
export function deactivateNamespacesFromParentSlot(
  parentSlot: ENSv2NameSlot,
  previousChildRegistryId: Bytes,
  block: ethereum.Block
): void {
  for (let i = 0; i < parentSlot.pathCount; i++) {
    let pathIndex = ENSv2SlotPathIndex.load(slotPathIndexId(parentSlot.id, i));
    if (pathIndex == null) {
      continue;
    }
    let parentPath = ENSv2NamePath.load(pathIndex.path);
    if (parentPath == null || !parentPath.slot.equals(parentSlot.id)) {
      continue;
    }
    let namespace = ENSv2Namespace.load(
      namespaceId(previousChildRegistryId, parentPath.namehash)
    );
    if (namespace == null) {
      continue;
    }
    namespace.active = false;
    namespace.updatedAtBlock = block.number;
    namespace.save();
    deactivatePathsForNamespace(namespace, block);
  }
}

// Paths materialised under a namespace (one per slot ever
// registered in the namespace's registry while it was active — see
// materializePathsForSlot) stayed active:true forever even after their
// owning namespace was deactivated above. Bounded by namespace.pathCount
// via ENSv2NamespacePathIndex, the same pattern
// handleLabelUnregistered (ensv2Registry.ts) already uses for a slot's own
// paths — not an unbounded scan over the child registry's slots.
function deactivatePathsForNamespace(
  namespace: ENSv2Namespace,
  block: ethereum.Block,
  depth: i32 = 0
): void {
  for (let i = 0; i < namespace.pathCount; i++) {
    let pathIndex = ENSv2NamespacePathIndex.load(namespacePathIndexId(namespace.id, i));
    if (pathIndex == null) {
      continue;
    }
    let path = ENSv2NamePath.load(pathIndex.path);
    // A path another registry's namespace has since taken over is no longer
    // this namespace's to deactivate.
    if (path == null || !path.namespace.equals(namespace.id)) {
      continue;
    }
    let wasActive = path.active;
    if (wasActive) {
      adjustParentSubdomainCount(path, -1);
    }
    path.active = false;
    path.updatedAt = block.timestamp;
    path.updatedAtBlock = block.number;
    path.save();
    // The name is no longer reachable through ENSv2: its compatibility row
    // must stop reading as live.
    if (wasActive) {
      let slot = ENSv2NameSlot.load(path.slot);
      if (slot != null) {
        retireProjectedDomain(path, slot, block.timestamp);
      }
      deactivateDescendantsOfPath(path, block, depth + 1);
    }
  }
}

// A path that stops resolving takes every name below it along:
// getSubregistry() returns zero for an expired or unregistered name, and the
// names of a superseded subregistry are no longer reachable. Deactivate the
// namespaces linked below the path and, recursively, their paths. Re-linking
// (handleSubregistryUpdated) reactivates and backfills them.
export function deactivateDescendantsOfPath(
  path: ENSv2NamePath,
  block: ethereum.Block,
  depth: i32
): void {
  if (depth >= MAX_BACKFILL_DEPTH) {
    return;
  }
  let current = ENSv2NamePath.load(path.id);
  if (current == null) {
    return;
  }
  for (let i = 0; i < current.namespaceCount; i++) {
    let index = ENSv2PathNamespaceIndex.load(pathNamespaceIndexId(current.id, i));
    if (index == null) {
      continue;
    }
    let namespace = ENSv2Namespace.load(index.namespace);
    if (namespace == null || !namespace.active) {
      continue;
    }
    namespace.active = false;
    namespace.updatedAtBlock = block.number;
    namespace.save();
    deactivatePathsForNamespace(namespace, block, depth);
  }
}

export function handleSubregistryUpdated(event: SubregistryUpdated): void {
  let parentRegistryId = event.address;
  let parentSlotId = toSlotId(event.params.tokenId);
  let parentSlot = ENSv2NameSlot.load(nameSlotId(parentRegistryId, parentSlotId));
  if (parentSlot == null) {
    log.warning("SubregistryUpdated for unknown slot {} on registry {}", [
      parentSlotId.toString(),
      parentRegistryId.toHexString(),
    ]);
    return;
  }

  let previousChildAddress = parentSlot.subregistry;

  parentSlot.subregistryAddress = isZeroAddress(event.params.subregistry)
    ? null
    : event.params.subregistry;
  parentSlot.subregistry = isZeroAddress(event.params.subregistry)
    ? null
    : event.params.subregistry;
  parentSlot.updatedAt = event.block.timestamp;
  parentSlot.updatedAtBlock = event.block.number;
  parentSlot.save();

  let history = new ENSv2SubregistryUpdate(createEventID(event));
  history.slot = parentSlot.id;
  history.blockNumber = event.block.number;
  history.transactionID = event.transaction.hash;
  history.logIndex = event.logIndex;
  history.subregistryAddress = isZeroAddress(event.params.subregistry)
    ? null
    : event.params.subregistry;
  history.sender = event.params.sender;
  history.save();

  upsertNamespaceLink(parentRegistryId, parentSlot, previousChildAddress, event);

  if (isZeroAddress(event.params.subregistry)) {
    if (previousChildAddress) {
      deactivateNamespacesFromParentSlot(
        parentSlot,
        previousChildAddress,
        event.block
      );
    }
    return;
  }

  // Direct non-zero-to-non-zero swap (registry A -> registry B, no
  // intervening clear-to-zero) — setSubregistry has no on-chain requirement
  // to pass through zero first, so this is reachable, not hypothetical.
  // Without this, namespaces from the superseded registry A stay
  // active:true forever and can resurface if A later gets its own
  // registrations.
  if (previousChildAddress && !previousChildAddress.equals(event.params.subregistry)) {
    deactivateNamespacesFromParentSlot(parentSlot, previousChildAddress, event.block);
  }

  let childRegistry = getOrCreateRegistry(
    event.params.subregistry,
    event.params.subregistry,
    event.block
  );

  // Loop over the PARENT slot's existing paths only — never the child
  // registry's own slots. This loop direction is the entire mechanism
  // enforcing the bounded-cost projection rule.
  for (let i = 0; i < parentSlot.pathCount; i++) {
    let pathIndex = ENSv2SlotPathIndex.load(slotPathIndexId(parentSlot.id, i));
    if (pathIndex == null) {
      continue;
    }
    let parentPath = ENSv2NamePath.load(pathIndex.path);
    if (parentPath == null || !parentPath.active) {
      continue;
    }

    linkAndBackfillNamespace(
      childRegistry,
      parentSlot,
      parentPath,
      event.params.tokenId,
      event,
      0
    );
  }
}


// Creates or reactivates the namespace (childRegistry under parentPath).
// When it was not already active, names registered in childRegistry before
// the link have no path under it yet, so backfill them.
function linkAndBackfillNamespace(
  childRegistry: ENSv2Registry,
  parentSlot: ENSv2NameSlot,
  parentPath: ENSv2NamePath,
  parentTokenId: BigInt,
  event: ethereum.Event,
  depth: i32
): void {
  let existing = ENSv2Namespace.load(
    namespaceId(childRegistry.id, parentPath.namehash)
  );
  let wasActive = existing != null && existing.active;
  let namespace = createOrReactivateNamespace(
    childRegistry,
    parentSlot,
    parentPath,
    parentTokenId,
    event
  );
  if (existing == null) {
    appendRegistryNamespaceIndex(childRegistry, namespace);
    appendPathNamespaceIndex(parentPath, namespace);
  }
  if (!wasActive) {
    backfillNamespace(childRegistry.id, namespace, event, depth);
  }
}

// Projects every name already REGISTERED in the namespace's registry, then
// follows each one's subregistry. Same projection as a registration, except
// that the slot's resolver is attached here (at registration time the
// registry emits ResolverUpdated afterwards instead).
function backfillNamespace(
  registryId: Bytes,
  namespace: ENSv2Namespace,
  event: ethereum.Event,
  depth: i32
): void {
  if (depth >= MAX_BACKFILL_DEPTH) {
    log.warning("backfillNamespace: depth limit reached at namespace {}", [
      namespace.id.toHexString(),
    ]);
    return;
  }
  // Fresh load: the caller's snapshot may predate namespace index appends.
  let registry = ENSv2Registry.load(registryId);
  if (registry == null) {
    return;
  }
  // The RootRegistry has no parent, and the ETHRegistry is linked once, at
  // deployment, before any .eth registration.
  if (registry.kind == "ROOT" || registry.kind == "ETH") {
    return;
  }
  let slotCount = registry.slotCount;
  for (let i = 0; i < slotCount; i++) {
    let index = ENSv2RegistrySlotIndex.load(registrySlotIndexId(registryId, i));
    if (index == null) {
      continue;
    }
    let slot = ENSv2NameSlot.load(index.slot);
    if (slot == null || slot.status != "REGISTERED") {
      continue;
    }
    let path = materializePathInNamespace(
      registry,
      namespace,
      slot,
      event,
      slot.migratedFromV1
    );
    if (path.domain != null) {
      let resolverAddress = slot.resolverAddress;
      if (resolverAddress) {
        attachDomainResolver(Address.fromBytes(resolverAddress), path.namehash);
      }
    }
    let subregistry = slot.subregistry;
    if (subregistry) {
      let childRegistry = getOrCreateRegistry(
        subregistry,
        Address.fromBytes(subregistry),
        event.block
      );
      linkAndBackfillNamespace(
        childRegistry,
        slot,
        path,
        currentTokenIdOf(slot),
        event,
        depth + 1
      );
    }
  }
}

function currentTokenIdOf(slot: ENSv2NameSlot): BigInt {
  let tokenEntityId = slot.currentToken;
  if (tokenEntityId) {
    let token = ENSv2Token.load(tokenEntityId);
    if (token != null) {
      return token.tokenId;
    }
  }
  return slot.slotId;
}

// The other bounded loop (see file header): iterates the REGISTERING
// slot's own registry's existing active namespaces, materialising or
// reactivating one path per namespace. Called from
// ensv2Registry.ts::handleLabelRegistered after the slot is saved.
export function materializePathsForSlot(
  registry: ENSv2Registry,
  slot: ENSv2NameSlot,
  event: ethereum.Event,
  isV1Migration: boolean
): void {
  for (let i = 0; i < registry.namespaceCount; i++) {
    let index = ENSv2RegistryNamespaceIndex.load(
      registryNamespaceIndexId(registry.id, i)
    );
    if (index == null) {
      continue;
    }
    let namespace = ENSv2Namespace.load(index.namespace);
    if (namespace == null || !namespace.active) {
      continue;
    }
    materializePathInNamespace(registry, namespace, slot, event, isV1Migration);
  }
}

// Materialises (or rebinds and reactivates) the slot's path under one
// namespace and projects it.
function materializePathInNamespace(
  registry: ENSv2Registry,
  namespace: ENSv2Namespace,
  slot: ENSv2NameSlot,
  event: ethereum.Event,
  isV1Migration: boolean
): ENSv2NamePath {
  let pathId = pathNamehash(namespace.baseNamehash, slot.labelhash);
  let path = ENSv2NamePath.load(pathId);
  let pathReactivated = false;
  if (path == null) {
    path = materializeNamePath(pathId, namespace, slot, event);
    appendSlotPathIndex(slot, path);
    appendNamespacePathIndex(namespace, path);
  } else {
    // A path id is the namehash, so a parent that switched its subregistry
    // to a new registry hands the same path to the new registry's slot.
    // Rebind it, or the new slot's transfers, renewals and unregistration
    // never reach the projected Domain.
    if (!path.slot.equals(slot.id)) {
      path.slot = slot.id;
      path.registry = slot.registry;
      path.namespace = namespace.id;
      appendSlotPathIndex(slot, path);
      appendNamespacePathIndex(namespace, path);
    }
    // Reactivation of a path deactivated by LabelUnregistered or a
    // namespace unlink: the projected Domain row (if any) is reused below,
    // so the parent's subdomainCount is restored here, before projection,
    // on the inactive -> active transition only.
    if (!path.active) {
      adjustParentSubdomainCount(path, 1);
      pathReactivated = true;
    }
    path.active = true;
    path.updatedAt = event.block.timestamp;
    path.updatedAtBlock = event.block.number;
    path.save();
  }

  // RootRegistry paths are native ENSv2 metadata for the root namespace,
  // not compatibility projections. Their IDs are ordinary ENSv1 namehashes
  // (for example, <eth> and <reverse>), so projecting them would overwrite
  // the existing ENSv1 Domain rows for those nodes.
  if (registry.kind != "ROOT") {
    projectPathToDomain(path, slot, event, isV1Migration, pathReactivated);
  }
  return path;
}

function materializeNamePath(
  pathId: Bytes,
  namespace: ENSv2Namespace,
  slot: ENSv2NameSlot,
  event: ethereum.Event
): ENSv2NamePath {
  let path = new ENSv2NamePath(pathId);
  path.slot = slot.id;
  path.registry = slot.registry;
  path.namespace = namespace.id;

  let parentPathId = namespace.parentPath;
  let depth = 0;
  let name: string | null = null;
  let label: string | null = null;
  if (checkValidLabel(slot.label)) {
    label = slot.label;
  } else {
    label = "[" + slot.labelhash.toHexString().slice(2) + "]";
  }
  path.label = label;

  if (parentPathId) {
    path.parent = parentPathId;
    let parentPath = ENSv2NamePath.load(parentPathId);
    if (parentPath != null) {
      depth = parentPath.depth + 1;
      if (parentPath.name !== null && label !== null) {
        name = (label as string) + "." + (parentPath.name as string);
      }
    }
  } else {
    // Root-level namespace (e.g. the "eth" registration under RootRegistry).
    depth = 0;
    name = label;
  }

  path.name = name;
  path.labelhash = slot.labelhash;
  path.namehash = pathNamehash(namespace.baseNamehash, slot.labelhash);
  path.depth = depth;
  path.active = true;
  path.namespaceCount = 0;
  path.createdAt = event.block.timestamp;
  path.updatedAt = event.block.timestamp;
  path.createdAtBlock = event.block.number;
  path.updatedAtBlock = event.block.number;
  path.save();
  return path;
}

export function handleResolverUpdated(event: ResolverUpdated): void {
  let registryId = event.address;
  let slotId = toSlotId(event.params.tokenId);
  let slot = ENSv2NameSlot.load(nameSlotId(registryId, slotId));
  if (slot == null) {
    log.warning("ResolverUpdated for unknown slot {} on registry {}", [
      slotId.toString(),
      registryId.toHexString(),
    ]);
    return;
  }

  if (isZeroAddress(event.params.resolver)) {
    slot.resolverAddress = null;
    slot.resolver = null;
  } else {
    slot.resolverAddress = event.params.resolver;
    // Share resolver creation with the resolver event mapping.
    let resolverEntity = getOrCreateResolver(event.params.resolver);
    slot.resolver = resolverEntity.id;
  }
  slot.updatedAt = event.block.timestamp;
  slot.updatedAtBlock = event.block.number;
  slot.save();

  // Keep the existing compatibility projection synchronized with the native
  // slot.
  for (let i = 0; i < slot.pathCount; i++) {
    let pathIndex = ENSv2SlotPathIndex.load(slotPathIndexId(slot.id, i));
    if (pathIndex == null) {
      continue;
    }
    let path = ENSv2NamePath.load(pathIndex.path);
    if (path == null) {
      continue;
    }
    if (!path.active || !path.slot.equals(slot.id)) {
      continue;
    }
    if (path.domain == null) {
      continue;
    }
    if (isZeroAddress(event.params.resolver)) {
      clearDomainResolver(path.namehash);
    } else {
      attachDomainResolver(event.params.resolver, path.namehash);
    }
  }

  let history = new ENSv2ResolverUpdate(createEventID(event));
  history.slot = slot.id;
  history.blockNumber = event.block.number;
  history.transactionID = event.transaction.hash;
  history.logIndex = event.logIndex;
  history.resolverAddress = isZeroAddress(event.params.resolver)
    ? null
    : event.params.resolver;
  history.sender = event.params.sender;
  history.save();
}

// Enrichment only: dynamically linked registries may legitimately return
// getParent() = (0x0, ""). This handler stores current state only.
export function handleParentUpdated(event: ParentUpdated): void {
  let registryId = event.address;
  let registry = ENSv2Registry.load(registryId);
  if (registry == null) {
    // The wrapper normally bootstraps this row first. Keep a guard so
    // unexpected event order does not terminate indexing.
    log.warning("ParentUpdated for unknown registry {}", [
      registryId.toHexString(),
    ]);
    return;
  }

  if (isZeroAddress(event.params.parent)) {
    registry.canonicalParentRegistry = null;
    // Keep the registry and label fields consistent when the parent is clear.
    registry.canonicalParentLabel = null;
  } else {
    let parentRegistry = getOrCreateRegistry(
      event.params.parent,
      event.params.parent,
      event.block
    );
    registry.canonicalParentRegistry = parentRegistry.id;
    if (checkValidLabel(event.params.label)) {
      registry.canonicalParentLabel = event.params.label;
    } else {
      // Do not retain the previous parent's label beside a new parent or
      // persist an invalid replacement label.
      registry.canonicalParentLabel = null;
    }
  }
  registry.updatedAtBlock = event.block.number;
  registry.save();
}
