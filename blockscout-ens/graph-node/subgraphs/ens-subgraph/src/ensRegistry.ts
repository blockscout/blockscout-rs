// Import types and APIs from graph-ts
import { Address, BigInt, Bytes, crypto, ens, store } from "@graphprotocol/graph-ts";

import {
  checkValidLabel,
  concat,
  createLegacyEventID,
  EMPTY_ADDRESS,
  EMPTY_ADDRESS_BYTEARRAY,
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
  Resolver,
  Transfer,
  WrappedDomain,
} from "./types/schema";

import { processApprovalForAll } from "./accessControl";
import { isMigrationDestination } from "./ensv2Constants";

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
// Moves to the ENSv2 Graveyard or a migration controller are the v1 voiding
// step of a migration and keep their wrapped state (corrected later by
// correctMigratedLegacyOwner).
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
  if (isMigrationDestination(newOwner)) {
    return;
  }
  domain.wrappedOwner = null;
  if (WrappedDomain.load(domain.id) != null) {
    store.remove("WrappedDomain", domain.id);
  }
}

// Handler for NewOwner events
function _handleNewOwner(event: NewOwnerEvent, isMigrated: boolean): void {
  let account = new Account(event.params.owner.toHexString());
  account.save();

  let subnode = makeSubnode(event);
  let domain = getDomain(subnode, event.block.timestamp);
  let parent = getDomain(event.params.node);

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

  detachWrappedStateIfLeftWrapper(domain, event.params.owner);
  domain.owner = event.params.owner.toHexString();
  domain.parent = event.params.node.toHexString();
  domain.labelhash = event.params.label;
  domain.isMigrated = isMigrated;
  saveDomain(domain);

  let domainEvent = new NewOwner(createLegacyEventID(event));
  domainEvent.blockNumber = event.block.number.toI32();
  domainEvent.transactionID = event.transaction.hash;
  domainEvent.parentDomain = event.params.node.toHexString();
  domainEvent.domain = subnode.toHexString();
  domainEvent.owner = event.params.owner.toHexString();
  domainEvent.save();
}

// Handler for Transfer events
export function handleTransfer(event: TransferEvent): void {
  let node = event.params.node;

  let account = new Account(event.params.owner.toHexString());
  account.save();

  // Update the domain owner
  let domain = getDomain(node)!;

  detachWrappedStateIfLeftWrapper(domain, event.params.owner);
  domain.owner = event.params.owner.toHexString();
  saveDomain(domain);

  let domainEvent = new Transfer(createLegacyEventID(event));
  domainEvent.blockNumber = event.block.number.toI32();
  domainEvent.transactionID = event.transaction.hash;
  domainEvent.domain = node.toHexString();
  domainEvent.owner = event.params.owner.toHexString();
  domainEvent.save();
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
  let domain = getDomain(node)!;
  domain.resolver = id;

  if (id) {
    let resolver = Resolver.load(id);
    if (resolver == null) {
      resolver = new Resolver(id);
      resolver.domain = event.params.node.toHexString();
      resolver.address = event.params.resolver;
      resolver.save();
      // since this is a new resolver entity, there can't be a resolved address yet so set to null
      domain.resolvedAddress = null;
    } else {
      domain.resolvedAddress = resolver.addr;
    }
  } else {
    domain.resolvedAddress = null;
  }
  saveDomain(domain);

  let domainEvent = new NewResolver(createLegacyEventID(event));
  domainEvent.blockNumber = event.block.number.toI32();
  domainEvent.transactionID = event.transaction.hash;
  domainEvent.domain = node.toHexString();
  domainEvent.resolver = id ? id : EMPTY_ADDRESS;
  domainEvent.save();
}

// Handler for NewTTL events
export function handleNewTTL(event: NewTTLEvent): void {
  let node = event.params.node;
  let domain = getDomain(node);
  // For the edge case that a domain's owner and resolver are set to empty
  // in the same transaction as setting TTL
  if (domain) {
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

export function handleNewResolverOldRegistry(event: NewResolverEvent): void {
  let node = event.params.node;
  let domain = getDomain(node, event.block.timestamp)!;
  if (node.equals(ROOT_NODE) || !domain.isMigrated) {
    handleNewResolver(event);
  }
}
export function handleNewTTLOldRegistry(event: NewTTLEvent): void {
  let domain = getDomain(event.params.node)!;
  if (domain.isMigrated == false) {
    handleNewTTL(event);
  }
}

export function handleTransferOldRegistry(event: TransferEvent): void {
  let domain = getDomain(event.params.node)!;
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
