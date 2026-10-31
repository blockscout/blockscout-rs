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
// For migrated names, domain.owner remains the ENSv1 registry-level owner,
// since it reflects ENSv1 ENSRegistry-level
// ownership, which the migration's own "graveyard" voiding step already
// legitimately moved away from the real user (a true fact about the retired
// v1 system, not something to overwrite). wrappedOwner/registrant are
// different — they're what real consumers read to find "who controls this
// name" — so those get corrected, domain.owner does not.
import { BigInt, log, store } from "@graphprotocol/graph-ts";
import { checkValidLabel, clampLegacyExpiry, ETH_NODE } from "./utils";
import { getEthRegistryAddress, getV2GracePeriod } from "./ensv2Constants";
import { pathNamehash, registryNamespaceIndexId } from "./ensv2Utils";
import {
  Domain,
  ENSv2NamePath,
  ENSv2NameSlot,
  ENSv2Namespace,
  ENSv2Registry,
  ENSv2RegistryNamespaceIndex,
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

// The single reusable implementation of the wrapped/unwrapped correction
// branch — called both at initial migration-flagged registration and at
// every subsequent TransferSingle/TransferBatch on a migratedFromV1 slot
// (transfers must keep writing to "the same legacy field").
// Re-checks WrappedDomain existence fresh every call rather than caching the
// original classification.
export function correctMigratedLegacyOwner(
  domainId: string,
  registrationId: string,
  ownerId: string,
  updateRegistration: boolean
): void {
  let wrappedDomain = WrappedDomain.load(domainId);
  if (wrappedDomain != null) {
    wrappedDomain.owner = ownerId;
    wrappedDomain.save();
    let domain = Domain.load(domainId);
    if (domain != null) {
      domain.wrappedOwner = ownerId;
      domain.save();
    }
  } else {
    let domain = Domain.load(domainId);
    if (domain != null) {
      domain.registrant = ownerId;
      domain.save();
    }
    // Several linked paths may project Domains for one ETHRegistry slot, but
    // the legacy Registration belongs only to the canonical .eth Domain.
    // Linked paths may update their own registrant without changing that
    // shared Registration (especially important for locked migrations, whose
    // canonical WrappedDomain intentionally retains the V1 registrant).
    if (updateRegistration) {
      let registration = Registration.load(registrationId);
      if (registration != null) {
        registration.registrant = ownerId;
        registration.save();
      }
    }
  }
}

// Keeps the legacy-compatibility Domain/Registration owner/registrant
// fields live on every transfer for a native ENSv2 (never-migrated) .eth
// name — correctMigratedLegacyOwner above only runs for migratedFromV1
// slots, so without this, a v2-native name's Domain.owner/registrant and
// Registration.registrant permanently retain the original registrant after
// the very first transfer. No WrappedDomain branching is
// needed here the way correctMigratedLegacyOwner has: a v2-native name has
// no legacy NameWrapper-wrapped concept to detect.
export function updateEthDomainOwner(
  domainId: string,
  registrationId: string,
  ownerId: string
): void {
  let domain = Domain.load(domainId);
  if (domain != null) {
    domain.owner = ownerId;
    domain.registrant = ownerId;
    domain.save();
  }
  let registration = Registration.load(registrationId);
  if (registration != null) {
    registration.registrant = ownerId;
    registration.save();
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
    registration.registrationDate = event.block.timestamp;
  }
  registration.domain = path.id.toHexString();
  let slotExpiryDate = slot.expiryDate;
  if (slotExpiryDate) {
    registration.expiryDate = clampLegacyExpiry(slotExpiryDate);
  }
  // Existing migrated rows are corrected by correctMigratedLegacyOwner so
  // wrapped names do not receive a registrant. New rows need this required
  // field initialized before they can be saved.
  if (!isV1Migration || isNewRegistration) {
    let registrantId = slot.registrant;
    if (registrantId) {
      registration.registrant = registrantId;
    }
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
  isV1Migration: boolean
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
  if (domain == null) {
    domain = new Domain(path.id.toHexString());
    domain.storedOffchain = false; // Blockscout/BENS extension
    domain.resolvedWithWildcard = false; // Blockscout/BENS extension
    domain.createdAt = event.block.timestamp;
    domain.subdomainCount = 0;
  }

  domain.name = path.name;
  if (checkValidLabel(path.label)) {
    domain.labelName = path.label;
  }
  domain.labelhash = path.labelhash;
  // Preserve the V1 owner on existing migrated rows. A new compatibility row
  // has no legacy value to preserve and needs its required owner initialized.
  if ((!isV1Migration || isNewDomain) && ownerId) {
    domain.owner = ownerId;
  }
  // Migrated names: registrant correction is correctMigratedLegacyOwner's
  // job below (branch-aware), not this generic assignment.
  if (!isV1Migration) {
    let registrantId = slot.registrant;
    if (registrantId) {
      domain.registrant = registrantId;
    }
  }
  domain.isMigrated = true;

  linkProjectedDomainToParent(domain, path, isNewDomain);

  // v2GracePeriod is ETHRegistrar/ETHRenewerV1-specific policy — only
  // applied for real .eth registrations, where Domain.expiryDate has always
  // meant the true reregistration-availability date, not raw expiry.
  let isEth = slot.registry.equals(getEthRegistryAddress());
  let namespace = ENSv2Namespace.load(path.namespace);
  let isCanonicalEthPath = false;
  if (isEth && namespace != null) {
    isCanonicalEthPath = namespace.baseNamehash.equals(ETH_NODE);
  }
  // A native (non-migration) ENSv2 .eth registration means the ENSv1 name
  // had expired, and with it any NameWrapper token (expiry emits no event
  // on either contract). Mirror ethRegistrar.handleNameRegistered and drop
  // the stale wrapped state so the Domain reflects the live v2 owner only.
  if (isCanonicalEthPath && !isV1Migration) {
    domain.wrappedOwner = null;
    if (WrappedDomain.load(domain.id) != null) {
      store.remove("WrappedDomain", domain.id);
    }
  }
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
    if (isCanonicalEthPath) {
      syncEthRegistration(slot, path, event, isV1Migration);
    }
    // NameRegistered normally follows the registry events in the deployed
    // registrar, but keeping this reconciliation here makes fixtures and
    // replays with the opposite ordering converge to the same native row.
    syncRegistrationFromSlot(slot);
    if (isV1Migration && ownerId) {
      correctMigratedLegacyOwner(
        domain.id,
        slot.labelhash.toHexString(),
        ownerId,
        isCanonicalEthPath
      );
    }
  }
}
