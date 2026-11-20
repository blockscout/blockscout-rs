import { Address, BigInt, Bytes, ethereum, log } from "@graphprotocol/graph-ts";

import { PublicResolver } from "./types/RNSUnified/PublicResolver";
import { RNSUnified } from "./types/RNSUnified/RNSUnified";
import { Domain, Resolver } from "./types/schema";
import { refreshPrimaryNameRecord, syncReverseNode } from "./primaryName";
import {
  createOrLoadAccount,
  EMPTY_ADDRESS,
  isForwardName,
  isReverseNode,
  isUsableName,
  isValidLabel,
  labelhash,
  nodeId,
  ROOT_NODE,
} from "./utils";

function newDomain(id: string, node: BigInt, timestamp: BigInt): Domain {
  let domain = new Domain(id);
  domain.subdomainCount = 0;
  domain.isMigrated = true;
  domain.createdAt = timestamp;
  domain.owner = createOrLoadAccount(EMPTY_ADDRESS).id;
  domain.storedOffchain = false;
  domain.resolvedWithWildcard = false;
  // The ERC-721 token id is the namehash (RNSUnified.mint, LibRNSDomain.toId).
  domain.tokenId = node;
  return domain;
}

export function loadRoot(timestamp: BigInt): Domain {
  let root = Domain.load(ROOT_NODE);
  if (root == null) {
    root = newDomain(ROOT_NODE, BigInt.zero(), timestamp);
    root.save();
  }
  return root;
}

// Loads `id` or creates it below `parent`. Not saved.
export function createChild(
  id: string,
  node: BigInt,
  parent: Domain,
  timestamp: BigInt
): Domain {
  let domain = Domain.load(id);
  if (domain == null) {
    domain = newDomain(id, node, timestamp);
    parent.subdomainCount = parent.subdomainCount + 1;
    parent.save();
  }
  return domain;
}

// Sets parent, label and name from the immutable half of an RNS record.
export function applyLabel(domain: Domain, parent: Domain, label: string): void {
  domain.parent = parent.id;
  domain.labelhash = labelhash(label);
  if (!isValidLabel(label)) {
    log.warning("RNS: label of {} cannot be stored", [domain.id]);
    domain.name = null;
    domain.labelName = null;
    return;
  }
  let parentName = parent.name;
  if (parent.id == ROOT_NODE) {
    domain.name = label;
  } else if (parentName !== null) {
    domain.name = label + "." + parentName;
  }
  // BENS only lists rows with a label_name. Leaving it empty on top-level and
  // reverse-tree nodes keeps them out of lookups, as unknown labels do on ENS.
  domain.labelName = isForwardName(domain) ? label : null;
}

export function setOwner(domain: Domain, owner: string): void {
  domain.owner = createOrLoadAccount(owner).id;
  // One token per name: the NFT owner is also the registrant.
  if (isForwardName(domain) && owner != EMPTY_ADDRESS) {
    domain.registrant = owner;
  } else {
    domain.registrant = null;
  }
}

export function setResolver(domain: Domain, resolver: Address): void {
  if (resolver.equals(Address.zero())) {
    domain.resolver = null;
    return;
  }
  let id = resolver.toHexString().concat("-").concat(domain.id);
  if (Resolver.load(id) == null) {
    let entity = new Resolver(id);
    entity.domain = domain.id;
    entity.address = resolver;
    entity.save();
  }
  domain.resolver = id;
}

// Saves a domain after owner or resolver changes.
//
// RNS PublicResolver.setAddr always reverts and addr(node) returns
// RNSUnified.ownerOf(node) (rns-contracts src/resolvers/PublicResolver.sol), so
// no AddrChanged is ever emitted: a forward name with a resolver resolves to its
// NFT owner. Names held without a resolver, such as the reserved names the RNS
// auction owns, resolve to nothing.
export function saveDomain(domain: Domain): void {
  let previousOwner: string | null = null;
  let stored = Domain.load(domain.id);
  if (stored != null) {
    previousOwner = stored.owner;
  }

  if (
    isForwardName(domain) &&
    domain.resolver !== null &&
    domain.owner != EMPTY_ADDRESS
  ) {
    domain.resolvedAddress = domain.owner;
  } else {
    domain.resolvedAddress = null;
  }
  domain.save();

  if (isForwardName(domain)) {
    let owner = domain.owner;
    if (previousOwner === null) {
      refreshPrimaryNameRecord(owner);
    } else if (previousOwner != owner) {
      refreshPrimaryNameRecord(previousOwner);
      refreshPrimaryNameRecord(owner);
    }
  } else if (isReverseNode(domain)) {
    syncReverseNode(domain);
  }
}

// Reads getRecord at the current block. Its expiry applies RESERVATION_ROLE
// (RNSUnified._expiry): names held by a reserved account, which is how the RNS
// auction holds most tokens, never expire. Its owner is 0 for an expired name,
// so it only fills an owner that is unknown, e.g. a name loaded while expired
// and renewed later.
export function refreshFromChain(
  domain: Domain,
  node: BigInt,
  registry: Address
): void {
  let call = RNSUnified.bind(registry).try_getRecord(node);
  if (call.reverted) return;
  domain.expiryDate = call.value.mut.expiry;
  let owner = call.value.mut.owner;
  if (domain.owner == EMPTY_ADDRESS && !owner.equals(Address.zero())) {
    setOwner(domain, owner.toHexString());
  }
}

// PublicResolver.name() of a reverse node returns the stored name only while
// the address owns the named domain, the only case BENS shows anyway.
function readReverseName(domain: Domain, resolverAddress: Address): void {
  let resolverId = domain.resolver;
  if (resolverId === null) return;
  let call = PublicResolver.bind(resolverAddress).try_name(
    Bytes.fromHexString(domain.id)
  );
  if (call.reverted || !isUsableName(call.value)) return;
  let resolver = Resolver.load(resolverId);
  if (resolver == null) return;
  resolver.name = call.value;
  resolver.save();
}

// Returns the domain for `node`, loading it from chain state when the store
// does not have it yet. That is every name minted before the Ronin L2 genesis
// until the backfill reaches it: no event of theirs is reachable on L2.
// getRecord is read at the handler's block, so the result is the end-of-block
// state; the caller then applies its own event on top. Returns null when the
// token does not exist, e.g. it was burned in this block.
export function loadDomain(
  node: BigInt,
  registry: Address,
  block: ethereum.Block
): Domain | null {
  let id = nodeId(node);
  let existing = Domain.load(id);
  if (existing != null) return existing;
  if (node.isZero()) return loadRoot(block.timestamp);

  let call = RNSUnified.bind(registry).try_getRecord(node);
  if (call.reverted) {
    log.warning("RNS: getRecord({}) reverted at block {}", [
      id,
      block.number.toString(),
    ]);
    return null;
  }
  let record = call.value;
  let parent = loadDomain(record.immut.parentId, registry, block);
  if (parent == null) return null;

  let domain = createChild(id, node, parent, block.timestamp);
  applyLabel(domain, parent, record.immut.label);
  setOwner(domain, record.mut.owner.toHexString());
  setResolver(domain, record.mut.resolver);
  domain.expiryDate = record.mut.expiry;
  if (isReverseNode(domain)) {
    readReverseName(domain, record.mut.resolver);
  }
  saveDomain(domain);
  return domain;
}
