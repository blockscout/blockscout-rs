// Shared handlers for the RootRegistry/ETHRegistry static sources and the
// ENSv2Registry template (dynamically discovered UserRegistry/WrapperRegistry
// instances) — all three are PermissionedRegistry instances emitting the
// same event set, so one set of handler functions serves all of them
// (event-type imports below are taken from RootRegistry's generated types
// per this repo's existing precedent in ensRegistry.ts, which does the same
// for its dual ENSRegistry/ENSRegistryOld sources — structurally identical
// classes regardless of which data source's codegen output they come from).
//
// Registry lifecycle, ownership, namespace, resolver, and role handlers.
import { Address, BigInt, Bytes, ethereum, log } from "@graphprotocol/graph-ts";

import {
  getOrCreateRegistry,
  getOrCreateRootNamespace,
  kindForAddress,
} from "./ensv2Discovery";
import {
  appendRegistrySlotIndex,
  nameSlotId,
  pathNamehash,
  resourceId,
  slotPathIndexId,
  toSlotId,
  tokenEntityId,
} from "./ensv2Utils";
import { cutOffLegacyHoldoutWithinEvent } from "./legacyGovernance";
import {
  checkValidLabel,
  concat,
  createEventID,
  createOrLoadAccount,
  i32ToBytes,
} from "./utils";
import {
  clearsOnUnregister,
  getEthRegistryAddress,
  isMigrationController,
} from "./ensv2Constants";
import {
  retireProjectedDomain,
  syncProjectedExpiry,
  syncProjectedHolder,
  syncProjectedToken,
} from "./ensv2Domain";
import { processEACRolesChanged } from "./ensv2Roles";
import { addToSubdomainCount } from "./subdomainCount";
import { syncRegistrationFromSlot } from "./ensv2Registrar";
import { processApprovalForAll } from "./accessControl";
import {
  handleParentUpdated as handleParentUpdatedPaths,
  handleResolverUpdated as handleResolverUpdatedPaths,
  handleSubregistryUpdated as handleSubregistryUpdatedPaths,
  clearProjectedDomainResolversForSlot,
  continueQueuedBackfills,
  deactivateDescendantsOfPath,
  resetBackfillBudget,
  materializePathsForSlot,
  removeFromNamespacePaths,
  unlinkSubregistryFromSlot,
} from "./ensv2Paths";
import {
  Domain,
  ENSv2LabelRegistered,
  ENSv2LabelRenewed,
  ENSv2LabelUnregistered,
  ENSv2NamePath,
  ENSv2Namespace,
  ENSv2NameSlot,
  ENSv2Registry,
  ENSv2Resource,
  ENSv2SlotPathIndex,
  ENSv2Token,
  ENSv2TokenRegenerated,
  ENSv2TokenTransferred,
  Registration,
} from "./types/schema";

import {
  ApprovalForAll,
  EACRolesChanged,
  ExpiryUpdated,
  LabelRegistered,
  LabelReserved,
  LabelUnregistered,
  ParentUpdated,
  ResolverUpdated,
  SubregistryUpdated,
  TokenRegenerated,
  TokenResource,
  TransferBatch,
  TransferSingle,
  Upgraded,
  URIUpdated,
} from "./types/RootRegistry/PermissionedRegistry";

// Ensures an ENSv2Registry row exists for whichever data source (static
// Root/ETH, or a template-discovered instance) fired the current event, and
// bootstraps the root namespace the first time any RootRegistry event
// arrives (RootRegistry has no parent to emit a SubregistryUpdated that
// would otherwise create it).
function bootstrapRegistry(address: Address, block: ethereum.Block): void {
  let registry = getOrCreateRegistry(address, address, block);
  let kind = registry.kind;
  if (kind == "ROOT") {
    getOrCreateRootNamespace(registry.id, block);
  }
}

// End the current token/resource ownership incarnation while retaining its
// entities as historical current-state rows. The stable slot itself remains
// reusable for a later registration.
function deactivateSlotIncarnation(
  slot: ENSv2NameSlot,
  timestamp: BigInt,
  blockNumber: BigInt
): void {
  let currentTokenId = slot.currentToken;
  if (currentTokenId) {
    let token = ENSv2Token.load(currentTokenId);
    if (token != null) {
      token.active = false;
      token.owner = null;
      token.updatedAtBlock = blockNumber;
      token.save();
    }
  }

  let currentResourceId = slot.currentResource;
  if (currentResourceId) {
    let resource = ENSv2Resource.load(currentResourceId);
    if (resource != null) {
      resource.active = false;
      resource.endedAt = timestamp;
      resource.updatedAtBlock = blockNumber;
      resource.save();
    }
  }

  slot.currentToken = null;
  slot.currentResource = null;
  slot.owner = null;
  slot.registrant = null;
}

// A slot is a token id with its version bits cleared, and a registry derives
// the token id from the label, so a slot has one labelhash for life. An
// event that gives an existing slot another one can only come from a
// contract that emits whatever it likes, and is ignored: every labelhash a
// slot were allowed to take would be another path on it, and the handlers
// for a slot walk all of its paths.
function ignoreRelabelledSlot(slot: ENSv2NameSlot, labelHash: Bytes): void {
  log.warning(
    "registry {}: slot {} has labelhash {}, ignoring an event that gives it {}",
    [
      slot.registry.toHexString(),
      slot.slotId.toString(),
      slot.labelhash.toHexString(),
      labelHash.toHexString(),
    ]
  );
}

export function handleLabelRegistered(event: LabelRegistered): void {
  resetBackfillBudget();
  bootstrapRegistry(event.address, event.block);

  let registryId = event.address;
  let slotId = toSlotId(event.params.tokenId);
  let id = nameSlotId(registryId, slotId);

  let slot = ENSv2NameSlot.load(id);
  if (slot != null && !slot.labelhash.equals(event.params.labelHash)) {
    ignoreRelabelledSlot(slot, event.params.labelHash);
    return;
  }
  // Row existence alone isn't "was this previously registered" — a plain
  // reservation (handleLabelReserved) already creates this row before any
  // real registration happens, so a reserved-then-first-registered slot was
  // wrongly counted as a re-registration. The contract itself distinguishes
  // this exact case (PermissionedRegistry.sol::_register, ROLE_WAS_RESERVED),
  // so check the pre-mutation status rather than mere row existence.
  let isReRegistration = slot != null && slot.status != "RESERVED";
  let isNewSlot = false;
  if (slot == null) {
    slot = new ENSv2NameSlot(id);
    slot.registry = registryId;
    slot.slotId = slotId;
    slot.pathCount = 0;
    slot.createdAt = event.block.timestamp;
    slot.createdAtBlock = event.block.number;
    isNewSlot = true;
  }

  slot.labelhash = event.params.labelHash;
  if (checkValidLabel(event.params.label)) {
    slot.label = event.params.label;
  }
  let account = createOrLoadAccount(event.params.owner);
  // A migration: the sender is a migration controller, or a WrapperRegistry
  // registering in itself, which only its own NameWrapper receiver does
  // (LockedWrapperReceiver._inject, for an emancipated subname sent to it).
  let isV1Migration = isMigrationController(event.params.sender);
  if (!isV1Migration && event.params.sender.equals(event.address)) {
    let ownRegistry = ENSv2Registry.load(registryId);
    isV1Migration = ownRegistry != null && ownRegistry.kind == "WRAPPER";
  }
  slot.owner = account.id;
  slot.registrant = account.id;
  slot.status = "REGISTERED";
  slot.expiryDate = event.params.expiry;
  slot.migratedFromV1 = isV1Migration;
  // The contract unconditionally reinitializes resolver/subregistry
  // alongside expiry on every (re-)registration (PermissionedRegistry.sol
  // ::_register), including to the zero address — which fires no
  // ResolverUpdated/SubregistryUpdated event. Without resetting here, a
  // re-registration that doesn't set a resolver/subregistry in the same
  // call would leave these fields pointing at the previous owner's values.
  // handleResolverUpdated/
  // handleSubregistryUpdated will overwrite these again later in the same
  // transaction if the registration call did set them.
  let previousSubregistry = slot.subregistry;
  slot.resolver = null;
  slot.resolverAddress = null;
  slot.subregistry = null;
  slot.subregistryAddress = null;
  slot.updatedAt = event.block.timestamp;
  slot.updatedAtBlock = event.block.number;
  if (isNewSlot) {
    appendRegistrySlotIndex(slot);
  }
  slot.save();

  clearProjectedDomainResolversForSlot(slot);

  // The subregistry the slot pointed at before this registration is gone on
  // chain (see above), and with it every name that hung off this one through
  // it: the previous holder's subnames, when a lapsed name is registered
  // again. No SubregistryUpdated announces it, and a later one would find
  // slot.subregistry already null, so retire them here. If this registration
  // sets the same subregistry again, the SubregistryUpdated that follows
  // links and backfills it afresh.
  if (previousSubregistry) {
    unlinkSubregistryFromSlot(slot, previousSubregistry, event.block);
  }

  let history = new ENSv2LabelRegistered(createEventID(event));
  history.slot = slot.id;
  history.blockNumber = event.block.number;
  history.transactionID = event.transaction.hash;
  history.logIndex = event.logIndex;
  history.owner = account.id;
  history.expiryDate = event.params.expiry;
  history.isReRegistration = isReRegistration;
  history.sender = event.params.sender;
  history.isV1Migration = isV1Migration;
  history.save();

  // Reload after bootstrap because, for a ROOT registry,
  // bootstrapRegistry also calls getOrCreateRootNamespace, which does its
  // OWN independent ENSv2Registry.load(...)/namespaceCount+=1/.save() on
  // the same id (ensv2Discovery.ts). graph-ts entities are snapshots, not
  // live references, so that mutation is invisible to any registry object
  // obtained before it ran — only a fresh load after bootstrapRegistry
  // returns sees the incremented namespaceCount materializePathsForSlot's
  // loop below depends on. Passing bootstrapRegistry's own returned
  // registry object here instead (removing this "redundant" reload) was
  // tried and reverted after it silently broke path materialisation for
  // every name under root.
  let registry = ENSv2Registry.load(registryId)!;
  // The other bounded loop: materialise a path for
  // each namespace this registry currently, actively serves.
  materializePathsForSlot(registry, slot, event, isV1Migration);

  continueQueuedBackfills(event);
}

export function handleLabelReserved(event: LabelReserved): void {
  resetBackfillBudget();
  bootstrapRegistry(event.address, event.block);

  let registryId = event.address;
  let slotId = toSlotId(event.params.tokenId);
  let id = nameSlotId(registryId, slotId);

  let slot = ENSv2NameSlot.load(id);
  if (slot != null && !slot.labelhash.equals(event.params.labelHash)) {
    ignoreRelabelledSlot(slot, event.params.labelHash);
    return;
  }
  let isNewSlot = false;
  if (slot == null) {
    slot = new ENSv2NameSlot(id);
    slot.registry = registryId;
    slot.slotId = slotId;
    slot.pathCount = 0;
    slot.migratedFromV1 = false;
    slot.createdAt = event.block.timestamp;
    slot.createdAtBlock = event.block.number;
    isNewSlot = true;
  }

  slot.labelhash = event.params.labelHash;
  if (checkValidLabel(event.params.label)) {
    slot.label = event.params.label;
  }
  slot.status = "RESERVED";
  slot.expiryDate = event.params.expiry;
  slot.updatedAt = event.block.timestamp;
  slot.updatedAtBlock = event.block.number;
  if (isNewSlot) {
    appendRegistrySlotIndex(slot);
  }
  slot.save();
  // LabelReserved is represented as current slot state only.

  // A label reserved or registered in a WrapperRegistry is ENSv2's from
  // then on: the registry stops sending the ENSv1 name to ENSv1
  // (_isMigratableChild reads a non-zero expiry as "v2 is the authority").
  // A registration clears the ENSv1 row by projecting over it; a
  // reservation projects nothing, so it is cleared here. The contract
  // refuses to reserve a label whose ENSv1 name is still emancipated, so
  // this is not expected to find one.
  let registry = ENSv2Registry.load(registryId);
  if (registry != null && registry.kind == "WRAPPER") {
    let namespaces = registry.materializedNamespaces;
    for (let i = 0; i < namespaces.length; i++) {
      let namespace = ENSv2Namespace.load(namespaces[i]);
      if (namespace == null) {
        continue;
      }
      let parentPathId = namespace.parentPath;
      if (!parentPathId) {
        continue;
      }
      let parentPath = ENSv2NamePath.load(parentPathId);
      // Only a locked migration leaves a name unmarked with ENSv1 names
      // still showing an address below it (legacyGovernance.ts rule 3).
      if (
        parentPath == null ||
        parentPath.legacySubtreeVoided ||
        !parentPath.domain
      ) {
        continue;
      }
      cutOffLegacyHoldoutWithinEvent(
        pathNamehash(namespace.baseNamehash, event.params.labelHash),
        event.block
      );
    }
  }
}

export function handleLabelUnregistered(event: LabelUnregistered): void {
  resetBackfillBudget();
  bootstrapRegistry(event.address, event.block);

  let registryId = event.address;
  let slotId = toSlotId(event.params.tokenId);
  let id = nameSlotId(registryId, slotId);

  let slot = ENSv2NameSlot.load(id);
  if (slot == null) {
    log.warning(
      "LabelUnregistered for unknown slot {} on registry {}",
      [slotId.toString(), registryId.toHexString()]
    );
    return;
  }

  // AVAILABLE means there is no active native owner or resource incarnation.
  // Label/labelhash remain on the stable slot as identity/history metadata.
  slot.status = "AVAILABLE";
  slot.expiryDate = event.block.timestamp;
  deactivateSlotIncarnation(slot, event.block.timestamp, event.block.number);
  slot.updatedAt = event.block.timestamp;
  slot.updatedAtBlock = event.block.number;
  slot.save();

  // Deactivate this slot's materialised paths. The loop is bounded by
  // slot.pathCount via ENSv2SlotPathIndex
  // mechanism already used to materialise them (not an unbounded scan).
  // materializePathsForSlot already reactivates a path on re-registration,
  // so this and that together give the primary case a coherent lifecycle.
  // Names below the slot are deactivated through the path's namespace index
  // (deactivateDescendantsOfPath).
  for (let i = 0; i < slot.pathCount; i++) {
    let pathIndex = ENSv2SlotPathIndex.load(slotPathIndexId(slot.id, i));
    if (pathIndex == null) {
      continue;
    }
    let path = ENSv2NamePath.load(pathIndex.path);
    // A path another registry's slot has since taken over is not this
    // slot's to deactivate.
    if (path == null || !path.slot.equals(slot.id)) {
      continue;
    }
    let wasActive = path.active;
    if (wasActive) {
      removeFromNamespacePaths(path);
    }
    path.active = false;
    path.updatedAt = event.block.timestamp;
    path.updatedAtBlock = event.block.number;
    path.save();
    if (wasActive) {
      // Mirror ENSv1: a child stops counting towards its parent's
      // subdomainCount when it is no longer live (subdomainCount.ts).
      let parentDomainId = retireProjectedDomain(path, event.block.timestamp);
      if (parentDomainId) {
        addToSubdomainCount(parentDomainId!, -1);
      }
      // getSubregistry() returns zero for an unregistered (expired) name on
      // every implementation, so nothing below it resolves any more.
      deactivateDescendantsOfPath(path, event.block);
    }
  }

  // Registries on an implementation with contracts-v2 #501 also clear the
  // slot's resolver and subregistry on unregister, without emitting
  // ResolverUpdated or SubregistryUpdated. Mirror it, and unlink the
  // subregistry's namespaces, which no longer hang off this name. Older
  // implementations keep both on chain (and restore them on a revive), so
  // their slots keep them here too.
  let registry = ENSv2Registry.load(registryId);
  if (registry != null && clearsOnUnregister(registry.implementation)) {
    let previousSubregistry = slot.subregistry;
    slot.resolver = null;
    slot.resolverAddress = null;
    slot.subregistry = null;
    slot.subregistryAddress = null;
    slot.save();
    if (previousSubregistry) {
      unlinkSubregistryFromSlot(slot, previousSubregistry, event.block);
    }
  }

  let history = new ENSv2LabelUnregistered(createEventID(event));
  history.slot = slot.id;
  history.blockNumber = event.block.number;
  history.transactionID = event.transaction.hash;
  history.logIndex = event.logIndex;
  history.sender = event.params.sender;
  history.save();
}

export function handleExpiryUpdated(event: ExpiryUpdated): void {
  // Renewals are the most frequent registry event: they carry unfinished
  // backfills forward too.
  resetBackfillBudget();
  continueQueuedBackfills(event);
  bootstrapRegistry(event.address, event.block);

  let registryId = event.address;
  let slotId = toSlotId(event.params.tokenId);
  let id = nameSlotId(registryId, slotId);

  let slot = ENSv2NameSlot.load(id);
  if (slot == null) {
    log.warning(
      "ExpiryUpdated for unknown slot {} on registry {}",
      [slotId.toString(), registryId.toHexString()]
    );
    return;
  }

  slot.expiryDate = event.params.newExpiry;
  slot.updatedAt = event.block.timestamp;
  slot.updatedAtBlock = event.block.number;
  slot.save();

  // Legacy sync — REGISTERED only, in every registry. ExpiryUpdated also
  // fires for premigrated RESERVED names renewed through ETHRenewerV1, whose
  // renew() already calls the authoritative v1
  // BaseRegistrarImplementation.renew() in the same transaction; the ENSv1
  // handlers maintain Registration/Domain for those, so a RESERVED slot is
  // left alone.
  if (slot.status == "REGISTERED") {
    if (slot.registry.equals(getEthRegistryAddress())) {
      // Keep the registrar-enriched native row in sync even when no
      // ETHRegistrar NameRenewed event accompanies it (for example, a direct
      // registry renewal).
      syncRegistrationFromSlot(slot);
    }
    syncProjectedExpiry(slot, event.params.newExpiry);
  }

  let history = new ENSv2LabelRenewed(createEventID(event));
  history.slot = slot.id;
  history.blockNumber = event.block.number;
  history.transactionID = event.transaction.hash;
  history.logIndex = event.logIndex;
  history.newExpiryDate = event.params.newExpiry;
  history.save();
}

export function handleSubregistryUpdated(event: SubregistryUpdated): void {
  bootstrapRegistry(event.address, event.block);
  handleSubregistryUpdatedPaths(event);
  continueQueuedBackfills(event);
}

export function handleResolverUpdated(event: ResolverUpdated): void {
  resetBackfillBudget();
  bootstrapRegistry(event.address, event.block);
  handleResolverUpdatedPaths(event);
  continueQueuedBackfills(event);
}

export function handleTokenResource(event: TokenResource): void {
  bootstrapRegistry(event.address, event.block);

  let registryId = event.address;
  let slotId = toSlotId(event.params.tokenId);
  let slot = ENSv2NameSlot.load(nameSlotId(registryId, slotId));
  if (slot == null) {
    log.warning(
      "TokenResource for unknown slot {} on registry {}",
      [slotId.toString(), registryId.toHexString()]
    );
    return;
  }

  let resourceEntity = ENSv2Resource.load(
    resourceId(registryId, event.params.resource)
  );
  if (resourceEntity == null) {
    resourceEntity = new ENSv2Resource(
      resourceId(registryId, event.params.resource)
    );
    resourceEntity.registry = registryId;
    resourceEntity.resource = event.params.resource;
    resourceEntity.active = true;
    resourceEntity.createdAt = event.block.timestamp;
    resourceEntity.createdAtBlock = event.block.number;
  }
  resourceEntity.slot = slot.id;
  resourceEntity.updatedAtBlock = event.block.number;

  // Nullable-Bytes comparison, not `!==`/`!=` (AssemblyScript
  // compiler gotcha): guard with a truthy check, then use .equals() on the
  // narrowed value. Captured before the token load/create below so the OLD
  // token id is still available for the deactivation branch that mirrors
  // the resource deactivation a few lines down.
  let previousTokenId = slot.currentToken;

  let token = ENSv2Token.load(tokenEntityId(registryId, event.params.tokenId));
  if (token == null) {
    token = new ENSv2Token(tokenEntityId(registryId, event.params.tokenId));
    token.registry = registryId;
    token.tokenId = event.params.tokenId;
    token.active = true;
    token.createdAtBlock = event.block.number;
  }
  token.slot = slot.id;
  token.resource = event.params.resource;
  token.resourceEntity = resourceEntity.id;
  token.updatedAtBlock = event.block.number;
  token.save();

  resourceEntity.currentToken = token.id;
  resourceEntity.save();

  // Re-registration burns the old token and mints a new one. Regeneration is
  // handled separately by handleTokenRegenerated.
  if (previousTokenId) {
    let isDifferentToken = !previousTokenId.equals(token.id);
    if (isDifferentToken) {
      let oldToken = ENSv2Token.load(previousTokenId);
      if (oldToken != null) {
        oldToken.active = false;
        oldToken.updatedAtBlock = event.block.number;
        oldToken.save();
      }
    }
  }

  // Narrow nullable Bytes before calling .equals().
  let previousResourceId = slot.currentResource;
  if (previousResourceId) {
    let isDifferentResource = !previousResourceId.equals(resourceEntity.id);
    if (isDifferentResource) {
      let oldResource = ENSv2Resource.load(previousResourceId);
      if (oldResource != null) {
        oldResource.active = false;
        oldResource.endedAt = event.block.timestamp;
        oldResource.save();
      }
    }
  }

  slot.currentResource = resourceEntity.id;
  slot.currentToken = token.id;
  slot.updatedAt = event.block.timestamp;
  slot.updatedAtBlock = event.block.number;
  slot.save();
  syncProjectedToken(slot);

  // Complete the registrar enrichment if NameRegistered was observed before
  // this registry-side resource event. For non-ETH registries the helper is
  // deliberately a no-op.
  syncRegistrationFromSlot(slot);
}

export function handleTokenRegenerated(event: TokenRegenerated): void {
  bootstrapRegistry(event.address, event.block);

  let registryId = event.address;
  let oldToken = ENSv2Token.load(
    tokenEntityId(registryId, event.params.oldTokenId)
  );
  if (oldToken == null) {
    log.warning("TokenRegenerated for unknown old token {} on registry {}", [
      event.params.oldTokenId.toString(),
      registryId.toHexString(),
    ]);
    return;
  }

  let newToken = new ENSv2Token(
    tokenEntityId(registryId, event.params.newTokenId)
  );
  newToken.registry = registryId;
  newToken.tokenId = event.params.newTokenId;
  newToken.slot = oldToken.slot;
  newToken.resource = oldToken.resource;
  newToken.resourceEntity = oldToken.resourceEntity;
  newToken.owner = oldToken.owner;
  newToken.active = true;
  newToken.createdAtBlock = event.block.number;
  newToken.updatedAtBlock = event.block.number;
  newToken.save();

  oldToken.active = false;
  oldToken.updatedAtBlock = event.block.number;
  oldToken.save();

  // ENSv2TokenRegenerated.slot is non-null — only write history (and
  // repoint the slot/resource's currentToken) when the old token actually
  // had a resolved slot.
  let oldTokenSlotId = oldToken.slot;
  if (oldTokenSlotId) {
    let slot = ENSv2NameSlot.load(oldTokenSlotId);
    if (slot != null) {
      let slotOwner = slot.owner;
      if (slotOwner) {
        // TransferSingle burn runs before TokenRegenerated and correctly
        // clears the old token's owner. The still-registered slot remains
        // authoritative for the replacement token.
        newToken.owner = slotOwner;
        newToken.save();
      }
      slot.currentToken = newToken.id;
      // Every other slot-touching handler in this file sets updatedAt
      // alongside updatedAtBlock.
      slot.updatedAt = event.block.timestamp;
      slot.updatedAtBlock = event.block.number;
      slot.save();
      syncProjectedToken(slot);
    }
    let oldTokenResourceEntityId = oldToken.resourceEntity;
    if (oldTokenResourceEntityId) {
      let resourceEntity = ENSv2Resource.load(oldTokenResourceEntityId);
      if (resourceEntity != null) {
        resourceEntity.currentToken = newToken.id;
        resourceEntity.updatedAtBlock = event.block.number;
        resourceEntity.save();
      }
    }

    let history = new ENSv2TokenRegenerated(createEventID(event));
    history.slot = oldTokenSlotId;
    history.blockNumber = event.block.number;
    history.transactionID = event.transaction.hash;
    history.logIndex = event.logIndex;
    history.oldTokenId = event.params.oldTokenId;
    history.newTokenId = event.params.newTokenId;
    history.save();
  }
}

function makeTokenTransfer(
  registryId: Bytes,
  tokenId: BigInt,
  from: Address,
  to: Address,
  block: ethereum.Block,
  transactionID: Bytes,
  logIndex: BigInt,
  eventId: Bytes
): void {
  let token = ENSv2Token.load(tokenEntityId(registryId, tokenId));
  if (token == null) {
    // Fresh mint's TransferSingle can arrive before TokenResource — create
    // a placeholder now, TokenResource reconciles slot/resource later
    // (mirrors nameWrapper.ts's placeholder-then-reconcile pattern for
    // WrappedDomain).
    token = new ENSv2Token(tokenEntityId(registryId, tokenId));
    token.registry = registryId;
    token.tokenId = tokenId;
    token.active = true;
    token.createdAtBlock = block.number;
  }
  let toAccount = createOrLoadAccount(to);
  let isBurn = to.equals(Address.zero());
  if (isBurn) {
    token.owner = null;
    token.active = false;
  } else {
    token.owner = toAccount.id;
    token.active = true;
  }
  token.updatedAtBlock = block.number;
  token.save();

  let tokenSlotId = token.slot;
  if (tokenSlotId) {
    let slot = ENSv2NameSlot.load(tokenSlotId);
    if (slot != null) {
      let expiry = slot.expiryDate;
      let isExpiredBurn = false;
      if (isBurn && expiry) {
        isExpiredBurn = expiry.le(block.timestamp);
      }
      let isLifecycleBurn =
        isBurn && (slot.status == "AVAILABLE" || isExpiredBurn);
      if (isLifecycleBurn) {
        // Covers both unregister's LabelUnregistered -> burn order and the
        // pre-LabelRegistered burn used when replacing an expired name.
        slot.status = "AVAILABLE";
        deactivateSlotIncarnation(slot, block.timestamp, block.number);
      } else if (!isBurn) {
        slot.owner = toAccount.id;
        slot.registrant = toAccount.id;
      }
      slot.updatedAtBlock = block.number;
      slot.save();

      // Keep the projected legacy rows on the current holder, migrated or
      // native, in every registry. Burns write nothing: a regeneration burn
      // is an implementation detail, an expiry burn precedes the
      // re-registration that re-projects the name, and an unregister burn
      // follows the LabelUnregistered that already retired it.
      if (!isBurn) {
        syncProjectedHolder(slot, toAccount.id);
      }
    }

    let history = new ENSv2TokenTransferred(eventId);
    history.slot = tokenSlotId;
    history.blockNumber = block.number;
    history.transactionID = transactionID;
    history.logIndex = logIndex;
    history.from = createOrLoadAccount(from).id;
    history.to = toAccount.id;
    history.tokenId = tokenId;
    history.save();
  }
  // token.slot == null: current-state token row is still updated above, but
  // ENSv2TokenTransferred.slot is non-null so no history row can be written
  // for this transfer until a later TokenResource resolves the slot.
}

// Ids one TransferBatch moves. A real batch pays a storage write per token
// and cannot come near this in one transaction; the log itself can list tens
// of thousands of ids for the price of its data, and each id walks the
// slot's paths.
const MAX_BATCH_TRANSFER_IDS = 1000;

// A transfer of value 0 moves nothing: ERC1155Singleton._update changes the
// owner only when value > 0 and emits the event either way, so a holder can
// have a registry emit a "transfer" of their own token to anyone. Following
// it would show that address as the owner.
export function handleTransferSingle(event: TransferSingle): void {
  bootstrapRegistry(event.address, event.block);
  if (event.params.value.isZero()) {
    return;
  }

  makeTokenTransfer(
    event.address,
    event.params.id,
    event.params.from,
    event.params.to,
    event.block,
    event.transaction.hash,
    event.logIndex,
    Bytes.fromByteArray(concat(createEventID(event), i32ToBytes(0)))
  );
}

export function handleTransferBatch(event: TransferBatch): void {
  bootstrapRegistry(event.address, event.block);

  let registryId = event.address;
  let ids = event.params.ids;
  let values = event.params.values;
  let seen = new Map<string, boolean>();
  for (let i = 0; i < ids.length; i++) {
    if (i >= values.length || values[i].isZero()) {
      continue;
    }
    // A token changes hands once per batch; a repeated id says nothing new.
    let key = ids[i].toString();
    if (seen.has(key)) {
      continue;
    }
    if (seen.size >= MAX_BATCH_TRANSFER_IDS) {
      log.warning(
        "registry {}: TransferBatch moves more than {} tokens, the rest are not followed",
        [registryId.toHexString(), MAX_BATCH_TRANSFER_IDS.toString()]
      );
      break;
    }
    seen.set(key, true);
    makeTokenTransfer(
      registryId,
      ids[i],
      event.params.from,
      event.params.to,
      event.block,
      event.transaction.hash,
      event.logIndex,
      Bytes.fromByteArray(concat(createEventID(event), i32ToBytes(i)))
    );
  }
}

export function handleParentUpdated(event: ParentUpdated): void {
  bootstrapRegistry(event.address, event.block);
  handleParentUpdatedPaths(event);
}

// ApprovalForAll is a global per-account operator grant, not scoped to a
// slot/namespace, so it needs no path/materialisation interaction — unlike
// every other handler in this file, it does no more than bootstrap the
// registry row and delegate.
export function handleENSv2ApprovalForAll(event: ApprovalForAll): void {
  bootstrapRegistry(event.address, event.block);
  processApprovalForAll(
    event.address,
    event.params.account,
    event.params.operator,
    event.params.approved,
    event.block
  );
}

export function handleEACRolesChanged(event: EACRolesChanged): void {
  bootstrapRegistry(event.address, event.block);
  processEACRolesChanged(
    event.address,
    event.params.resource,
    event.params.account,
    event.params.oldRoleBitmap,
    event.params.newRoleBitmap,
    event.block,
    event.transaction.hash,
    event.logIndex
  );
}

// PermissionedRegistry.setURI() updates current registry-level metadata.
export function handleURIUpdated(event: URIUpdated): void {
  bootstrapRegistry(event.address, event.block);

  let registry = ENSv2Registry.load(event.address);
  if (registry == null) {
    return;
  }
  registry.uri = event.params.uri;
  registry.uriRenderer = event.params.renderer;
  registry.updatedAtBlock = event.block.number;
  registry.save();
}

// ERC1967 Upgraded: a UUPS upgrade of the registry proxy in place
// (upgradeToAndCall), which keeps the address, storage and every name. Keep
// `implementation` on the code the proxy now runs, so behaviour that differs
// between implementations (clearsOnUnregister) follows the upgrade. The proxy
// constructor emits the same event before VerifiableFactory's ProxyDeployed;
// that one names the implementation handleProxyDeployed already stored.
export function handleUpgraded(event: Upgraded): void {
  bootstrapRegistry(event.address, event.block);

  let registry = ENSv2Registry.load(event.address);
  if (registry == null) {
    return;
  }
  let implementation = event.params.implementation;
  registry.implementation = implementation;
  // A registry keeps its kind across upgrades; only one not yet classified
  // takes the kind of its new implementation.
  if (registry.kind == "UNKNOWN") {
    registry.kind = kindForAddress(event.address, implementation);
  }
  registry.updatedAtBlock = event.block.number;
  registry.save();
}
