// Import types and APIs from graph-ts
import {
  Address,
  BigInt,
  Bytes,
  crypto,
  ens,
  log,
  store,
} from "@graphprotocol/graph-ts";

import {
  checkValidLabel,
  concat,
  createLegacyEventID,
  EMPTY_ADDRESS,
  EMPTY_ADDRESS_BYTEARRAY,
  ETH_NODE,
  ROOT_NODE,
} from "./utils";
import { createResolverID } from "./resolver";

// Import event types from the registry contract ABI
import {
  ApprovalForAll as ApprovalForAllEvent,
  NewOwner as NewOwnerEvent,
  NewResolver as NewResolverEvent,
  NewTTL as NewTTLEvent,
  Transfer as TransferEvent,
} from "./types/ENSRegistry/EnsRegistry";

// Import entity types generated from the GraphQL schema
import {
  Account,
  Domain,
  NewOwner,
  NewResolver,
  NewTTL,
  Transfer,
  WrappedDomain,
} from "./types/schema";

import { processApprovalForAll } from "./accessControl";
import { isSink } from "./ensv2Constants";
import { isGovernedByEnsv2 } from "./legacyGovernance";
import { attachDomainResolver } from "./ensv2ResolverProjection";

const BIG_INT_ZERO = BigInt.fromI32(0);

function createDomain(node: Bytes, timestamp: BigInt): Domain {
  let domain = new Domain(node.toHexString());
  domain.storedOffchain = false; // Blockscout/BENS extension
  domain.resolvedWithWildcard = false; // Blockscout/BENS extension
  if (node.equals(ROOT_NODE)) {
    domain = new Domain(node.toHexString());
    domain.storedOffchain = false; // Blockscout/BENS extension
    domain.resolvedWithWildcard = false; // Blockscout/BENS extension
    domain.owner = EMPTY_ADDRESS;
    domain.isMigrated = true;
    domain.createdAt = timestamp;
    domain.subdomainCount = 0;
  }
  return domain;
}

function getDomain(
  node: Bytes,
  timestamp: BigInt = BIG_INT_ZERO
): Domain | null {
  let domain = Domain.load(node.toHexString());
  if (domain == null && node.equals(ROOT_NODE)) {
    return createDomain(node, timestamp);
  } else {
    return domain;
  }
}

function makeSubnode(event: NewOwnerEvent): Bytes {
  return Bytes.fromByteArray(
    crypto.keccak256(concat(event.params.node, event.params.label))
  );
}

// The `domain.resolver!.split("-")[0] == EMPTY_ADDRESS` disjunct the
// original ENSv1 subgraph had here is dead code: handleNewResolver (below)
// is Domain.resolver's only write site, and it sets the field to null
// exactly when the resolver address is the zero address, never to a
// zero-address-prefixed id (traced every write site to confirm before
// leaving it out, not assumed).
function recurseDomainDelete(domain: Domain): string | null {
  if (
    !domain.resolver &&
    domain.owner == EMPTY_ADDRESS &&
    domain.subdomainCount == 0
  ) {
    const parentDomain = Domain.load(domain.parent!);
    if (parentDomain != null) {
      parentDomain.subdomainCount = parentDomain.subdomainCount - 1;
      parentDomain.save();
      return recurseDomainDelete(parentDomain);
    }

    return null;
  }

  return domain.id;
}

function saveDomain(domain: Domain): void {
  recurseDomainDelete(domain);
  domain.save();
}

// A wrapped name's registry owner is the NameWrapper. The registry can move
// it elsewhere without any NameWrapper event: the parent calls
// setSubnodeOwner on a subname whose PARENT_CANNOT_CONTROL fuse is not
// burned, or the .eth registrant calls BaseRegistrar.reclaim. After that
// NameWrapper.isWrapped(node) is false and the ERC-1155 token is detached,
// so the wrapped state must go, exactly as NameUnwrapped would clear it.
// Moves into a sink never reach here (see applyRegistryOwner).
function detachWrappedStateIfLeftWrapper(
  domain: Domain,
  newOwner: Address
): void {
  // Nullable string: truthiness check, `== null` trips the AssemblyScript
  // compileBinaryOverload crash (see nameWrapper.handleNameUnwrapped).
  if (!domain.wrappedOwner) {
    return;
  }
  let newOwnerId = newOwner.toHexString();
  if (newOwnerId == domain.owner) {
    return;
  }
  removeWrappedState(domain);
}

function removeWrappedState(domain: Domain): void {
  domain.wrappedOwner = null;
  if (WrappedDomain.load(domain.id) != null) {
    store.remove("WrappedDomain", domain.id);
  }
}

// Writes a new ENSv1 registry owner into the row (legacyGovernance.ts rule 2).
// A sink is never stored: a .eth 2LD handed to one keeps its current owner
// (a migration's ENSv2 projection follows in the same transaction; a
// retirement leaves the lapsed name as it was), and a subname handed to one
// (only Graveyard.clear() does that) is deleted with the ENSv1 convention.
// The detached NameWrapper state of a deleted subname goes with it; its
// wrapper expiry no longer describes anything.
function applyRegistryOwner(
  domain: Domain,
  parentNode: Bytes,
  newOwner: Address,
  isNewDomain: boolean
): void {
  if (!isSink(newOwner)) {
    detachWrappedStateIfLeftWrapper(domain, newOwner);
    domain.owner = newOwner.toHexString();
    return;
  }
  if (parentNode.equals(ETH_NODE)) {
    if (isNewDomain) {
      // Required field; a sink is never stored.
      domain.owner = EMPTY_ADDRESS;
    }
    return;
  }
  domain.owner = EMPTY_ADDRESS;
  if (domain.wrappedOwner) {
    removeWrappedState(domain);
    domain.expiryDate = null;
  }
}

// Handler for NewOwner events
function _handleNewOwner(event: NewOwnerEvent, isMigrated: boolean): void {
  let account = new Account(event.params.owner.toHexString());
  account.save();

  let subnode = makeSubnode(event);
  let domainEvent = new NewOwner(createLegacyEventID(event));
  domainEvent.blockNumber = event.block.number.toI32();
  domainEvent.transactionID = event.transaction.hash;
  domainEvent.parentDomain = event.params.node.toHexString();
  domainEvent.domain = subnode.toHexString();
  domainEvent.owner = event.params.owner.toHexString();

  // A live ENSv2 name is the ENSv2 projection's (legacyGovernance.ts).
  if (isGovernedByEnsv2(subnode, event.block.number)) {
    domainEvent.save();
    return;
  }

  let domain = getDomain(subnode, event.block.timestamp);
  let parent = getDomain(event.params.node);

  let isNewDomain = domain == null;
  if (domain == null) {
    domain = new Domain(subnode.toHexString());
    domain.storedOffchain = false; // Blockscout/BENS extension
    domain.resolvedWithWildcard = false; // Blockscout/BENS extension
    domain.createdAt = event.block.timestamp;
    domain.subdomainCount = 0;
  }

  if (!domain.parent && parent != null) {
    parent.subdomainCount = parent.subdomainCount + 1;
    parent.save();
  }

  if (domain.name == null) {
    // Get label and node names
    let label = ens.nameByHash(event.params.label.toHexString());
    if (checkValidLabel(label)) {
      domain.labelName = label;
    } else {
      label = "[" + event.params.label.toHexString().slice(2) + "]";
    }
    if (event.params.node.equals(ROOT_NODE)) {
      domain.name = label;
    } else {
      parent = parent!;
      let name = parent.name;
      if (label && name) {
        domain.name = label + "." + name;
      }
    }
  }

  applyRegistryOwner(domain, event.params.node, event.params.owner, isNewDomain);
  domain.parent = event.params.node.toHexString();
  domain.labelhash = event.params.label;
  domain.isMigrated = isMigrated;
  saveDomain(domain);

  domainEvent.save();
}

// Handler for Transfer events
export function handleTransfer(event: TransferEvent): void {
  let node = event.params.node;

  let account = new Account(event.params.owner.toHexString());
  account.save();

  let domainEvent = new Transfer(createLegacyEventID(event));
  domainEvent.blockNumber = event.block.number.toI32();
  domainEvent.transactionID = event.transaction.hash;
  domainEvent.domain = node.toHexString();
  domainEvent.owner = event.params.owner.toHexString();
  domainEvent.save();

  if (isGovernedByEnsv2(node, event.block.number)) {
    return;
  }
  let domain = getDomain(node);
  if (domain == null) {
    log.warning("handleTransfer: no Domain for {}, skipping", [
      node.toHexString(),
    ]);
    return;
  }
  let parentNode = domain.parent
    ? Bytes.fromHexString(domain.parent!)
    : ROOT_NODE;
  applyRegistryOwner(domain, parentNode, event.params.owner, false);
  saveDomain(domain);
}

// Handler for NewResolver events
export function handleNewResolver(event: NewResolverEvent): void {
  let id: string | null;

  // if resolver is set to 0x0, set id to null
  // we don't want to create a resolver entity for 0x0
  if (event.params.resolver.equals(EMPTY_ADDRESS_BYTEARRAY)) {
    id = null;
  } else {
    id = createResolverID(event.params.node, event.params.resolver);
  }

  let node = event.params.node;
  let domainEvent = new NewResolver(createLegacyEventID(event));
  domainEvent.blockNumber = event.block.number.toI32();
  domainEvent.transactionID = event.transaction.hash;
  domainEvent.domain = node.toHexString();
  domainEvent.resolver = id ? id : EMPTY_ADDRESS;
  domainEvent.save();

  // A live ENSv2 name takes its resolver from ENSv2 (legacyGovernance.ts).
  if (isGovernedByEnsv2(node, event.block.number)) {
    return;
  }
  let domain = getDomain(node);
  if (domain == null) {
    log.warning("handleNewResolver: no Domain for {}, skipping", [
      node.toHexString(),
    ]);
    return;
  }
  domain.resolver = id;

  if (id) {
    saveDomain(domain);
    // Same attach as an ENSv2 ResolverUpdated. A classic resolver keeps its
    // legacy Resolver row and carries its addr over (a new row has none). A
    // PermissionedResolver has no node-keyed events: its records were set
    // through Linked / *Updated, which only project onto the node while the
    // registry points at that resolver, so the snapshot is rebuilt here from
    // the linked record.
    attachDomainResolver(event.params.resolver, node);
    return;
  }
  domain.resolvedAddress = null;
  saveDomain(domain);
}

// Handler for NewTTL events
export function handleNewTTL(event: NewTTLEvent): void {
  let node = event.params.node;
  let domain = getDomain(node);
  // For the edge case that a domain's owner and resolver are set to empty
  // in the same transaction as setting TTL. A live ENSv2 name is the ENSv2
  // projection's (legacyGovernance.ts).
  if (domain && !isGovernedByEnsv2(node, event.block.number)) {
    domain.ttl = event.params.ttl;
    domain.save();
  }

  let domainEvent = new NewTTL(createLegacyEventID(event));
  domainEvent.blockNumber = event.block.number.toI32();
  domainEvent.transactionID = event.transaction.hash;
  domainEvent.domain = node.toHexString();
  domainEvent.ttl = event.params.ttl;
  domainEvent.save();
}

export function handleNewOwner(event: NewOwnerEvent): void {
  _handleNewOwner(event, true);
}

export function handleNewOwnerOldRegistry(event: NewOwnerEvent): void {
  let subnode = makeSubnode(event);
  let domain = getDomain(subnode);

  if (domain == null || domain.isMigrated == false) {
    _handleNewOwner(event, false);
  }
}

// The three gated handlers below need the Domain to read isMigrated. Both
// registries are indexed from their deployment blocks, so every node the
// 2017 registry emits for already has a Domain from its NewOwner (two full
// mainnet syncs agree) — but a missing row must not abort indexing, so each
// warns and skips instead of asserting non-null.
export function handleNewResolverOldRegistry(event: NewResolverEvent): void {
  let node = event.params.node;
  let domain = getDomain(node, event.block.timestamp);
  if (domain == null) {
    log.warning(
      "handleNewResolverOldRegistry: no Domain for {}, skipping",
      [node.toHexString()]
    );
    return;
  }
  if (node.equals(ROOT_NODE) || !domain.isMigrated) {
    handleNewResolver(event);
  }
}
export function handleNewTTLOldRegistry(event: NewTTLEvent): void {
  let node = event.params.node;
  let domain = getDomain(node);
  if (domain == null) {
    log.warning("handleNewTTLOldRegistry: no Domain for {}, skipping", [
      node.toHexString(),
    ]);
    return;
  }
  if (domain.isMigrated == false) {
    handleNewTTL(event);
  }
}

export function handleTransferOldRegistry(event: TransferEvent): void {
  let node = event.params.node;
  let domain = getDomain(node);
  if (domain == null) {
    log.warning("handleTransferOldRegistry: no Domain for {}, skipping", [
      node.toHexString(),
    ]);
    return;
  }
  if (domain.isMigrated == false) {
    handleTransfer(event);
  }
}

// setApprovalForAll is scoped to the calling EOA globally, not to any
// domain's isMigrated state (confirmed by reading ENSRegistryWithFallback.sol)
// — so unlike NewOwner/NewResolver/NewTTL/Transfer above, both registries
// index it unconditionally rather than gating the old one on migration status.
export function handleApprovalForAll(event: ApprovalForAllEvent): void {
  processApprovalForAll(
    event.address,
    event.params.owner,
    event.params.operator,
    event.params.approved,
    event.block
  );
}

export function handleApprovalForAllOldRegistry(
  event: ApprovalForAllEvent
): void {
  processApprovalForAll(
    event.address,
    event.params.owner,
    event.params.operator,
    event.params.approved,
    event.block
  );
}
