// Option B compatibility projection: every non-root ENSv2NamePath materialised
// by ensv2Paths.ts::materializePathsForSlot gets a legacy Domain row (this is
// what lets existing ENSv1 consumers keep working unchanged for ENSv2-origin
// names), and the canonical <label>.eth path additionally gets a legacy
// Registration row. RootRegistry paths are native ENSv2 metadata only because
// their IDs collide with existing ENSv1 root-label Domain rows. An ETHRegistry
// slot may be reachable through several linked namespaces, but
// Registration.domain must remain the canonical .eth Domain rather than
// whichever linked path was materialised last. Only ever creates rows for
// names that never existed in ENSv1.
//
// The projection is authoritative for a live ENSv2 name (see
// legacyGovernance.ts): every registration, native or migration, writes the
// ENSv2 holder into Domain.owner/registrant (and the canonical .eth
// Registration), retires the ENSv1 NameWrapper state and the ENSv1 resolver,
// and later ENSv2 transfers, renewals and unregistrations keep the row in
// sync in every registry. ENSv1 events for a governed node only write their
// history rows.
import { BigInt, Bytes, log, store } from "@graphprotocol/graph-ts";
import {
  checkValidLabel,
  clampLegacyExpiry,
  EMPTY_ADDRESS,
  ETH_NODE,
} from "./utils";
import { getEthRegistryAddress, getV2GracePeriod } from "./ensv2Constants";
import {
  pathNamehash,
  registryNamespaceIndexId,
  slotPathIndexId,
} from "./ensv2Utils";
import {
  Domain,
  ENSv2NamePath,
  ENSv2NameSlot,
  ENSv2Namespace,
  ENSv2Registry,
  ENSv2RegistryNamespaceIndex,
  ENSv2SlotPathIndex,
  Registration,
  WrappedDomain,
} from "./types/schema";
import { LabelRegistered } from "./types/RootRegistry/PermissionedRegistry";
import { syncRegistrationFromSlot } from "./ensv2Registrar";

// Recovers a slot's namehash (Domain.id) without a ENSv2NamePath in hand —
// needed at transfer time, when only the slot is available.
// Read-only mirror of materializePathsForSlot's namespace loop; ENSv2NameSlot
// deliberately has no direct Domain/namehash field of its own, only
// labelhash, so this has to be recomputed rather than stored.
export function getEthDomainId(slot: ENSv2NameSlot): string | null {
  let registry = ENSv2Registry.load(slot.registry);
  if (registry == null) {
    return null;
  }
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
    // Only the genuine "eth" namespace can be the canonical parent —
    // without this check, any active namespace whose namehash happens to
    // resolve ETHRegistry as a subregistry would match, and setSubregistry
    // has no on-chain restriction on which registry a caller points their
    // own subregistry at. Filtering on baseNamehash makes
    // this deterministic instead of "whichever link was indexed first."
    if (!namespace.baseNamehash.equals(ETH_NODE)) {
      continue;
    }
    let pathId = pathNamehash(namespace.baseNamehash, slot.labelhash);
    let path = ENSv2NamePath.load(pathId);
    // Nullable Bytes require a truthy check before dereferencing.
    if (path != null) {
      let domainId = path.domain;
      if (domainId) {
        return domainId;
      }
    }
  }
  return null;
}

// True when `path` is the canonical <label>.eth path of an ETHRegistry slot,
// the only path the legacy Registration belongs to.
function isCanonicalEthPath(slot: ENSv2NameSlot, path: ENSv2NamePath): boolean {
  if (!slot.registry.equals(getEthRegistryAddress())) {
    return false;
  }
  let namespace = ENSv2Namespace.load(path.namespace);
  if (namespace == null) {
    return false;
  }
  return namespace.baseNamehash.equals(ETH_NODE);
}

// The slot's own active, projected paths. The slot path index can hold paths
// that a later registration in another registry took over (a path id is the
// namehash, shared by every registry that serves the same name), so a path is
// only the slot's while path.slot points back at it.
export function projectedPathsForSlot(slot: ENSv2NameSlot): ENSv2NamePath[] {
  let paths = new Array<ENSv2NamePath>();
  for (let i = 0; i < slot.pathCount; i++) {
    let pathIndex = ENSv2SlotPathIndex.load(slotPathIndexId(slot.id, i));
    if (pathIndex == null) {
      continue;
    }
    let path = ENSv2NamePath.load(pathIndex.path);
    if (path == null || !path.active || !path.slot.equals(slot.id)) {
      continue;
    }
    if (!path.domain) {
      continue;
    }
    paths.push(path);
  }
  return paths;
}

// ENSv2 transfer: the new holder controls the name in every registry.
export function syncProjectedHolder(slot: ENSv2NameSlot, ownerId: string): void {
  let paths = projectedPathsForSlot(slot);
  for (let i = 0; i < paths.length; i++) {
    let path = paths[i];
    let domain = Domain.load(path.domain!);
    if (domain == null) {
      continue;
    }
    domain.owner = ownerId;
    domain.registrant = ownerId;
    domain.save();
    if (isCanonicalEthPath(slot, path)) {
      let registration = Registration.load(slot.labelhash.toHexString());
      if (registration != null) {
        registration.registrant = ownerId;
        registration.save();
      }
    }
  }
}

// The slot's current token (TokenResource on registration, TokenRegenerated)
// on every projected row, so consumers find a name's ENSv2 token without
// joining the native entities.
export function syncProjectedToken(slot: ENSv2NameSlot): void {
  let paths = projectedPathsForSlot(slot);
  for (let i = 0; i < paths.length; i++) {
    let domain = Domain.load(paths[i].domain!);
    if (domain == null) {
      continue;
    }
    domain.registryToken = slot.currentToken;
    domain.save();
  }
}

// ENSv2 renewal (ExpiryUpdated) of a REGISTERED slot, in every registry.
export function syncProjectedExpiry(slot: ENSv2NameSlot, expiry: BigInt): void {
  let isEth = slot.registry.equals(getEthRegistryAddress());
  let paths = projectedPathsForSlot(slot);
  for (let i = 0; i < paths.length; i++) {
    let path = paths[i];
    let domain = Domain.load(path.domain!);
    if (domain == null) {
      continue;
    }
    domain.expiryDate = isEth
      ? clampLegacyExpiry(expiry.plus(getV2GracePeriod()))
      : clampLegacyExpiry(expiry);
    domain.save();
    if (isCanonicalEthPath(slot, path)) {
      let registration = Registration.load(slot.labelhash.toHexString());
      if (registration != null) {
        registration.expiryDate = clampLegacyExpiry(expiry);
        registration.save();
      }
    }
  }
}

// The name stopped existing in ENSv2 (LabelUnregistered, or its namespace was
// unlinked): apply the ENSv1 deletion convention so the row stops reading as
// live. The caller has already taken the path out of its parent's
// subdomainCount (adjustParentSubdomainCount), so this saves without
// re-running the ENSv1 delete recursion. A canonical .eth Registration keeps
// its (required) registrant and ends now.
export function retireProjectedDomain(
  path: ENSv2NamePath,
  slot: ENSv2NameSlot,
  timestamp: BigInt
): void {
  let domainId = path.domain;
  if (!domainId) {
    return;
  }
  let domain = Domain.load(domainId!);
  if (domain == null) {
    return;
  }
  domain.owner = EMPTY_ADDRESS;
  domain.registrant = null;
  domain.wrappedOwner = null;
  domain.registryToken = null;
  domain.resolver = null;
  domain.resolvedAddress = null;
  domain.expiryDate = timestamp;
  domain.save();
  if (WrappedDomain.load(domain.id) != null) {
    store.remove("WrappedDomain", domain.id);
  }
  if (isCanonicalEthPath(slot, path)) {
    let registration = Registration.load(slot.labelhash.toHexString());
    if (registration != null) {
      registration.expiryDate = timestamp;
      registration.save();
    }
  }
}

// Link a projected Domain to its projected parent when one exists. Root
// Registry paths deliberately have no legacy Domain projection, so a child
// such as foo.eth must fall back to the existing ENSv1 Domain("eth") row.
// The fallback is conditional: never create a legacy parent just to support
// a bounded ENSv2 projection.
function linkProjectedDomainToParent(
  domain: Domain,
  path: ENSv2NamePath,
  isNewDomain: boolean
): void {
  let parentPathId = path.parent;
  if (!parentPathId) {
    return;
  }

  let parentPath = ENSv2NamePath.load(parentPathId);
  if (parentPath == null) {
    return;
  }

  let projectedParentId = parentPath.domain;
  if (projectedParentId) {
    domain.parent = projectedParentId;

    // A Domain row created by an ENSv1 handler has already been counted by
    // that handler. Only a newly-created compatibility row contributes a new
    // direct child, and therefore increments the parent's count.
    if (isNewDomain) {
      let parentDomain = Domain.load(projectedParentId);
      if (parentDomain != null) {
        parentDomain.subdomainCount = parentDomain.subdomainCount + 1;
        parentDomain.save();
      }
    }
    return;
  }

  // RootRegistry paths are native-only, but their namehash IDs are the same
  // IDs used by the legacy root-label Domain rows (eth, reverse).
  let legacyParent = Domain.load(parentPath.id.toHexString());
  if (legacyParent == null) {
    return;
  }

  domain.parent = legacyParent.id;
  if (isNewDomain) {
    legacyParent.subdomainCount = legacyParent.subdomainCount + 1;
    legacyParent.save();
  }
}

// ENSv1 only counts a child in Domain.subdomainCount while it is live
// (recurseDomainDelete decrements when the registry owner is cleared). The
// ENSv2 analogue of that liveness is the projected path's `active` flag, so
// callers invoke this on an active-flag transition: -1 when a projected path
// is deactivated (LabelUnregistered, namespace unlink), +1 when a previously
// deactivated path is reactivated by a re-registration. Creation is counted
// separately by linkProjectedDomainToParent. Root paths and late-linked paths
// have no Domain projection and are ignored.
export function adjustParentSubdomainCount(
  path: ENSv2NamePath,
  delta: i32
): void {
  let domainId = path.domain;
  if (domainId == null) {
    return;
  }
  let domain = Domain.load(domainId!);
  if (domain == null) {
    return;
  }
  let parentId = domain.parent;
  if (parentId == null) {
    return;
  }
  let parent = Domain.load(parentId!);
  if (parent == null) {
    return;
  }
  let next = parent.subdomainCount + delta;
  parent.subdomainCount = next < 0 ? 0 : next;
  parent.save();
}

function syncEthRegistration(
  slot: ENSv2NameSlot,
  path: ENSv2NamePath,
  event: LabelRegistered,
  isV1Migration: boolean
): void {
  let id = slot.labelhash.toHexString();
  let registration = Registration.load(id);
  let isNewRegistration = registration == null;
  if (registration == null) {
    registration = new Registration(id);
  }
  // A migration continues the ENSv1 registration, so it keeps the original
  // date. A native registration is a new one, even when the label still has
  // a lapsed ENSv1 Registration row.
  if (isNewRegistration || !isV1Migration) {
    registration.registrationDate = event.block.timestamp;
  }
  registration.domain = path.id.toHexString();
  let slotExpiryDate = slot.expiryDate;
  if (slotExpiryDate) {
    registration.expiryDate = clampLegacyExpiry(slotExpiryDate);
  }
  let registrantId = slot.registrant;
  if (registrantId) {
    registration.registrant = registrantId;
  }
  if (checkValidLabel(slot.label)) {
    registration.labelName = slot.label;
  }
  registration.save();
}

export function projectPathToDomain(
  path: ENSv2NamePath,
  slot: ENSv2NameSlot,
  event: LabelRegistered,
  isV1Migration: boolean,
  pathReactivated: boolean
): void {
  let ownerId = slot.owner;
  if (ownerId == null) {
    // handleLabelRegistered sets slot.owner before this projection.
    log.warning(
      "projectPathToDomain: slot {} has no owner, skipping projection",
      [slot.id.toHexString()]
    );
    return;
  }

  let domain = Domain.load(path.id.toHexString());
  let isNewDomain = domain == null;
  // An existing row in the ENSv1 deleted state (owner zero address, no
  // resolver, no children) is no longer counted in its parent's
  // subdomainCount (ensRegistry.recurseDomainDelete, or retireProjectedDomain
  // before a path reactivation). Reviving it counts it again, unless the
  // path reactivation in materializePathsForSlot already did.
  let countsAsNewChild = isNewDomain;
  if (domain == null) {
    domain = new Domain(path.id.toHexString());
    domain.storedOffchain = false; // Blockscout/BENS extension
    domain.resolvedWithWildcard = false; // Blockscout/BENS extension
    domain.createdAt = event.block.timestamp;
    domain.subdomainCount = 0;
  } else if (
    !pathReactivated &&
    domain.owner == EMPTY_ADDRESS &&
    !domain.resolver &&
    domain.subdomainCount == 0
  ) {
    countsAsNewChild = true;
  }

  domain.name = path.name;
  if (checkValidLabel(path.label)) {
    domain.labelName = path.label;
  }
  domain.labelhash = path.labelhash;
  // The ENSv2 holder controls the name, migrated or not. The ENSv1 registry
  // owner of a migrated name is a sink (the Graveyard) or the NameWrapper,
  // neither of which controls it any more.
  domain.owner = ownerId!;
  let registrantId = slot.registrant;
  if (registrantId) {
    domain.registrant = registrantId;
  }
  domain.isMigrated = true;
  // Null until this registration's TokenResource, which the registry emits
  // after LabelRegistered (PermissionedRegistry._register) and which
  // syncProjectedToken writes through.
  domain.registryToken = slot.currentToken;

  linkProjectedDomainToParent(domain, path, countsAsNewChild);

  // v2GracePeriod is ETHRegistrar/ETHRenewerV1-specific policy — only
  // applied for real .eth registrations, where Domain.expiryDate has always
  // meant the true reregistration-availability date, not raw expiry.
  let isEth = slot.registry.equals(getEthRegistryAddress());
  let namespace = ENSv2Namespace.load(path.namespace);
  let canonicalEth = false;
  if (isEth && namespace != null) {
    canonicalEth = namespace.baseNamehash.equals(ETH_NODE);
  }
  // Any ENSv2 registration retires the ENSv1 NameWrapper state: a native
  // registration means the ENSv1 name (and its wrapper token) had lapsed, and
  // a migration parks the wrapper token in the Graveyard. Mirror
  // ethRegistrar.handleNameRegistered and drop it.
  domain.wrappedOwner = null;
  if (WrappedDomain.load(domain.id) != null) {
    store.remove("WrappedDomain", domain.id);
  }
  // The registry sets the resolver on every (re-)registration, and emits
  // ResolverUpdated after LabelRegistered when it is non-zero
  // (PermissionedRegistry._register); handleResolverUpdated re-attaches it
  // then. Without this reset an ENSv1 resolver the label had before would
  // stay attached to a native ENSv2 name.
  domain.resolver = null;
  domain.resolvedAddress = null;
  // Never-expiring slots carry uint64 max; clamp after adding the grace
  // period so the sum cannot overflow past the legacy maximum either.
  let slotExpiryDate = slot.expiryDate;
  if (isEth && slotExpiryDate) {
    domain.expiryDate = clampLegacyExpiry(
      slotExpiryDate.plus(getV2GracePeriod())
    );
  } else if (slotExpiryDate) {
    domain.expiryDate = clampLegacyExpiry(slotExpiryDate);
  } else {
    domain.expiryDate = null;
  }
  domain.save();

  path.domain = domain.id;
  path.save();

  if (isEth) {
    // ETHRegistry can serve multiple namespaces. The legacy Registration
    // model has only one Domain relation, so bind it exclusively to the
    // canonical <label>.eth path. Calling this for every linked path made the
    // result depend on namespace iteration order and detached
    // Domain.registration from the canonical .eth Domain.
    if (canonicalEth) {
      syncEthRegistration(slot, path, event, isV1Migration);
    }
    // NameRegistered normally follows the registry events in the deployed
    // registrar, but keeping this reconciliation here makes fixtures and
    // replays with the opposite ordering converge to the same native row.
    syncRegistrationFromSlot(slot);
  }
}
