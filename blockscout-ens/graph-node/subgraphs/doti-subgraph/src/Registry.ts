import { BigInt, crypto, ens } from "@graphprotocol/graph-ts";

import {
  NewOwner as NewOwnerEvent,
  Transfer as TransferEvent,
  NewResolver as NewResolverEvent,
  NewTTL as NewTTLEvent
} from "../generated/Registry/Registry";
import { NewOwner, Transfer, NewResolver, NewTTL, Domain, Account, Resolver } from "../generated/schema";
import { EMPTY_ADDRESS, EMPTY_ADDRESS_BYTEARRAY, ROOT_NODE, concat, createEventID } from "./utils";

const BIG_INT_ZERO = BigInt.fromI32(0);

/**
 * Creates a default root domain or basic domain entity.
 * @param node Node identifier hash.
 * @param timestamp Block timestamp.
 * @returns Initialized Domain entity.
 */
function createDomain(node: string, timestamp: BigInt): Domain {
  let domain = new Domain(node);
  if (node == ROOT_NODE) {
    domain = new Domain(node);
    domain.owner = EMPTY_ADDRESS;
    domain.isMigrated = true;
    domain.createdAt = timestamp;
    domain.subdomainCount = 0;
    domain.storedOffchain = false;
    domain.resolvedWithWildcard = false;
  }
  return domain;
}

/**
 * Retrieves an existing Domain entity or initializes root node if not found.
 * @param node Node hash string.
 * @param timestamp Optional timestamp for root creation.
 * @returns Domain entity or null.
 */
function getDomain(
  node: string,
  timestamp: BigInt = BIG_INT_ZERO
): Domain | null {
  let domain = Domain.load(node);
  if (domain === null && node == ROOT_NODE) {
    return createDomain(node, timestamp);
  } else {
    return domain;
  }
}

/**
 * Computes subnode hash from parent node and label.
 * @param event The NewOwner blockchain event.
 * @returns Computed subnode hex string.
 */
function makeSubnode(event: NewOwnerEvent): string {
  return crypto
    .keccak256(concat(event.params.node, event.params.label))
    .toHexString();
}

/**
 * Recursively deletes empty parent domain records if no longer owned or resolved.
 * @param domain Target Domain entity.
 * @returns Domain ID or null.
 */
function recurseDomainDelete(domain: Domain): string | null {
  if (
    (domain.resolver == null ||
      domain.resolver!.split("-")[0] == EMPTY_ADDRESS) &&
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

/**
 * Saves domain entity after running deletion check.
 * @param domain Domain entity to persist.
 */
function saveDomain(domain: Domain): void {
  recurseDomainDelete(domain);
  domain.save();
}

/**
 * Internal handler for NewOwner registry events.
 * @param event The NewOwner blockchain event.
 * @param isMigrated Migration flag.
 */
function _handleNewOwner(event: NewOwnerEvent, isMigrated: boolean): void {
  let account = new Account(event.params.owner.toHexString());
  account.save();

  let subnode = makeSubnode(event);
  let domain = getDomain(subnode, event.block.timestamp);
  let parent = getDomain(event.params.node.toHexString());

  if (domain === null) {
    domain = new Domain(subnode);
    domain.createdAt = event.block.timestamp;
    domain.subdomainCount = 0;
    domain.storedOffchain = false;
    domain.resolvedWithWildcard = false;
  }

  if (domain.parent === null && parent !== null) {
    parent.subdomainCount = parent.subdomainCount + 1;
    parent.save();
  }

  if (domain.name == null) {
    // Get label and node names
    let label = ens.nameByHash(event.params.label.toHexString());
    if (label != null) {
      domain.labelName = label;
    }

    if (label === null) {
      label = "[" + event.params.label.toHexString().slice(2) + "]";
    }
    if (
      event.params.node.toHexString() ==
      "0x0000000000000000000000000000000000000000000000000000000000000000"
    ) {
      domain.name = label;
    } else {
      parent = parent!;
      let name = parent.name;
      if (label && name) {
        domain.name = label + "." + name;
      }
    }
  }

  domain.owner = event.params.owner.toHexString();
  domain.parent = event.params.node.toHexString();
  domain.labelhash = event.params.label;
  domain.isMigrated = isMigrated;
  saveDomain(domain);

  let domainEvent = new NewOwner(createEventID(event));
  domainEvent.blockNumber = event.block.number.toI32();
  domainEvent.transactionID = event.transaction.hash;
  domainEvent.parentDomain = event.params.node.toHexString();
  domainEvent.domain = subnode;
  domainEvent.owner = event.params.owner.toHexString();
  domainEvent.save();
}

/**
 * Handles Transfer events on the registry.
 * @param event The Transfer blockchain event.
 */
export function handleTransfer(event: TransferEvent): void {
  let node = event.params.node.toHexString();

  let account = new Account(event.params.owner.toHexString());
  account.save();

  // Update the domain owner
  let domain = getDomain(node)!;

  domain.owner = event.params.owner.toHexString();
  saveDomain(domain);

  let domainEvent = new Transfer(createEventID(event));
  domainEvent.blockNumber = event.block.number.toI32();
  domainEvent.transactionID = event.transaction.hash;
  domainEvent.domain = node;
  domainEvent.owner = event.params.owner.toHexString();
  domainEvent.save();
}

/**
 * Handles NewResolver events on the registry to associate a resolver with a node.
 * @param event The NewResolver blockchain event.
 */
export function handleNewResolver(event: NewResolverEvent): void {
  let id: string | null;

  // if resolver is set to 0x0, set id to null
  if (event.params.resolver.equals(EMPTY_ADDRESS_BYTEARRAY)) {
    id = null;
  } else {
    id = event.params.resolver
      .toHexString()
      .concat("-")
      .concat(event.params.node.toHexString());
  }

  let node = event.params.node.toHexString();
  let domain = getDomain(node)!;
  domain.resolver = id;

  if (id) {
    let resolver = Resolver.load(id);
    if (resolver == null) {
      resolver = new Resolver(id);
      resolver.domain = event.params.node.toHexString();
      resolver.address = event.params.resolver;
      resolver.save();
      domain.resolvedAddress = null;
    } else {
      domain.resolvedAddress = resolver.addr;
    }
  } else {
    domain.resolvedAddress = null;
  }
  saveDomain(domain);

  let domainEvent = new NewResolver(createEventID(event));
  domainEvent.blockNumber = event.block.number.toI32();
  domainEvent.transactionID = event.transaction.hash;
  domainEvent.domain = node;
  domainEvent.resolver = id ? id : EMPTY_ADDRESS;
  domainEvent.save();
}

/**
 * Handles NewTTL events to update time-to-live values on domain records.
 * @param event The NewTTL blockchain event.
 */
export function handleNewTTL(event: NewTTLEvent): void {
  let node = event.params.node.toHexString();
  let domain = getDomain(node);
  if (domain) {
    domain.ttl = event.params.ttl;
    domain.save();
  }

  let domainEvent = new NewTTL(createEventID(event));
  domainEvent.blockNumber = event.block.number.toI32();
  domainEvent.transactionID = event.transaction.hash;
  domainEvent.domain = node;
  domainEvent.ttl = event.params.ttl;
  domainEvent.save();
}

/**
 * Handles NewOwner events on the registry.
 * @param event The NewOwner blockchain event.
 */
export function handleNewOwner(event: NewOwnerEvent): void {
  _handleNewOwner(event, true);
}
