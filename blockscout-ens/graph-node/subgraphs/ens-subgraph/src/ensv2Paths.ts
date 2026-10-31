// Namespace/path materialisation — the load-bearing cost-bound mechanism.
// Two complementary bounded loops implement it, and BOTH are needed:
//
//   handleSubregistryUpdated creates namespaces by looping over the PARENT
//   slot's existing paths (bounded by parentSlot.pathCount) — never the
//   child registry's slots. This is what makes a late link cheap.
//
//   materializePathsForSlot (called from ensv2Registry.ts::handleLabelRegistered)
//   creates paths by looping over the REGISTERING slot's own registry's
//   existing namespaces (bounded by registry.namespaceCount). This is what
//   makes "project into Domain only when the namespace existed at
//   registration time" true: a namespace linked after a name is already
//   registered simply isn't in that loop yet, so no path gets materialised
//   for it.
//
// Both directions must remain bounded; this mapping never performs recursive
// backfills.
import { Address, Bytes, ethereum, log } from "@graphprotocol/graph-ts";

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
  ENSv2ResolverUpdate,
  ENSv2Resource,
  ENSv2SlotPathIndex,
  ENSv2SubregistryUpdate,
} from "./types/schema";
import {
  LabelRegistered,
  ParentUpdated,
  ResolverUpdated,
  SubregistryUpdated,
} from "./types/RootRegistry/PermissionedRegistry";


function appendSlotPathIndex(slot: ENSv2NameSlot, path: ENSv2NamePath): void {
  let index = new ENSv2SlotPathIndex(slotPathIndexId(slot.id, slot.pathCount));
  index.slot = slot.id;
  index.index = slot.pathCount;
  index.path = path.id;
  index.save();

  slot.pathCount = slot.pathCount + 1;
  slot.save();
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
    if (path != null && path.domain != null) {
      clearDomainResolver(path.namehash);
    }
  }
}

function appendPathNamespaceIndex(
  path: ENSv2NamePath,
  namespace: ENSv2Namespace
): void {
  let index = new ENSv2PathNamespaceIndex(
    pathNamespaceIndexId(path.id, path.namespaceCount)
  );
  index.path = path.id;
  index.index = path.namespaceCount;
  index.namespace = namespace.id;
  index.save();

  path.namespaceCount = path.namespaceCount + 1;
  path.save();
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
  let index = new ENSv2NamespacePathIndex(
    namespacePathIndexId(namespace.id, namespace.pathCount)
  );
  index.namespace = namespace.id;
  index.index = namespace.pathCount;
  index.path = path.id;
  index.save();

  namespace.pathCount = namespace.pathCount + 1;
  namespace.save();
}

// Idempotent: sets active = true whether creating or reactivating. Caller
// checks pre-existence (ENSv2Namespace.load(id) == null, before calling
// this) to decide whether to append indices — reactivation must never
// re-append.
function createOrReactivateNamespace(
  childRegistry: ENSv2Registry,
  parentSlot: ENSv2NameSlot,
  parentPath: ENSv2NamePath,
  event: SubregistryUpdated
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
  namespace.parentTokenId = event.params.tokenId;
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
function deactivateNamespacesFromParentSlot(
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
    if (parentPath == null) {
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
  block: ethereum.Block
): void {
  for (let i = 0; i < namespace.pathCount; i++) {
    let pathIndex = ENSv2NamespacePathIndex.load(namespacePathIndexId(namespace.id, i));
    if (pathIndex == null) {
      continue;
    }
    let path = ENSv2NamePath.load(pathIndex.path);
    if (path == null) {
      continue;
    }
    if (path.active) {
      adjustParentSubdomainCount(path, -1);
    }
    path.active = false;
    path.updatedAt = block.timestamp;
    path.updatedAtBlock = block.number;
    path.save();
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

    let isNewNamespace =
      ENSv2Namespace.load(namespaceId(childRegistry.id, parentPath.namehash)) ==
      null;
    let namespace = createOrReactivateNamespace(
      childRegistry,
      parentSlot,
      parentPath,
      event
    );
    if (isNewNamespace) {
      appendRegistryNamespaceIndex(childRegistry, namespace);
      appendPathNamespaceIndex(parentPath, namespace);
    }
  }
}

// The other bounded loop (see file header): iterates the REGISTERING
// slot's own registry's existing active namespaces, materialising or
// reactivating one path per namespace. Called from
// ensv2Registry.ts::handleLabelRegistered after the slot is saved.
export function materializePathsForSlot(
  registry: ENSv2Registry,
  slot: ENSv2NameSlot,
  event: LabelRegistered,
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

    let pathId = pathNamehash(namespace.baseNamehash, slot.labelhash);
    let path = ENSv2NamePath.load(pathId);
    if (path == null) {
      path = materializeNamePath(pathId, namespace, slot, event);
      appendSlotPathIndex(slot, path);
      appendNamespacePathIndex(namespace, path);
    } else {
      // Reactivation of a path deactivated by LabelUnregistered or a
      // namespace unlink: the projected Domain row (if any) is reused below
      // with isNewDomain=false, so the parent's subdomainCount is restored
      // here, before projection, on the inactive -> active transition only.
      if (!path.active) {
        adjustParentSubdomainCount(path, 1);
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
      projectPathToDomain(path, slot, event, isV1Migration);
    }
  }
}

function materializeNamePath(
  pathId: Bytes,
  namespace: ENSv2Namespace,
  slot: ENSv2NameSlot,
  event: LabelRegistered
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
  // slot. The bounded path index deliberately excludes late-linked names.
  for (let i = 0; i < slot.pathCount; i++) {
    let pathIndex = ENSv2SlotPathIndex.load(slotPathIndexId(slot.id, i));
    if (pathIndex == null) {
      continue;
    }
    let path = ENSv2NamePath.load(pathIndex.path);
    if (path == null) {
      continue;
    }
    if (!path.active) {
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
