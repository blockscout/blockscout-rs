import { store } from "@graphprotocol/graph-ts";

import { Domain, PrimaryNameRecord, Resolver } from "./types/schema";
import {
  addressOfReverseName,
  EMPTY_ADDRESS,
  isForwardName,
  isUsableName,
  namehash,
  reverseNodeOf,
} from "./utils";

// The name stored on an address's <addr>.addr.reverse node, read from the
// resolver currently set on that node.
function storedReverseName(address: string): string | null {
  let reverse = Domain.load(reverseNodeOf(address));
  if (reverse == null) return null;
  let resolverId = reverse.resolver;
  if (resolverId === null) return null;
  let resolver = Resolver.load(resolverId);
  if (resolver == null) return null;
  return resolver.name;
}

function clearPrimaryNameRecord(address: string): void {
  if (PrimaryNameRecord.load(address) != null) {
    store.remove("PrimaryNameRecord", address);
  }
}

// Rebuilds the row BENS reads with its `primary_name_record` address-resolve
// technique.
//
// RNS PublicResolver.name(<addr>.addr.reverse) returns the stored name only
// while RNSUnified.ownerOf(namehash(name)) is that address
// (rns-contracts src/resolvers/PublicResolver.sol, name()). The record follows
// the same rule, so it must be refreshed whenever the reverse name, the reverse
// node's resolver or the named domain's owner changes. Expiry is not checked:
// a name that expires keeps its record until its next event.
export function refreshPrimaryNameRecord(address: string): void {
  if (address == EMPTY_ADDRESS) return;

  let name = storedReverseName(address);
  if (!isUsableName(name)) {
    clearPrimaryNameRecord(address);
    return;
  }

  let domainId = namehash(name!);
  let domain = Domain.load(domainId);
  if (domain == null || !isForwardName(domain) || domain.owner != address) {
    clearPrimaryNameRecord(address);
    return;
  }

  let record = PrimaryNameRecord.load(address);
  if (record == null) {
    record = new PrimaryNameRecord(address);
  }
  record.resolved_address = address;
  record.domain_id = domainId;
  record.domain_name = name!;
  record.save();
}

// Called after a <addr>.addr.reverse node or its name record changes.
export function syncReverseNode(domain: Domain): void {
  let address = addressOfReverseName(domain.name);
  if (address !== null) {
    refreshPrimaryNameRecord(address);
  }
}
