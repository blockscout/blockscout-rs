import { Bytes } from "@graphprotocol/graph-ts";
import {
  AddrChanged as AddrChangedEvent,
  NameChanged as NameChangedEvent,
} from "../generated/Resolver/Resolver";
import {
  Domain,
  Resolver,
  AddrChanged as AddrChangedEntity,
  NameChanged as NameChangedEntity,
} from "../generated/schema";
import { getOrCreateAccount, getOrCreateResolver } from "./utils";

// ─── AddrChanged ─────────────────────────────────────────────────────────────
// Resolver.AddrChanged(node, a) — EVM address record updated.
// Updates Resolver.addr and mirrors Domain.resolvedAddress.

export function handleAddrChanged(event: AddrChangedEvent): void {
  let nodeHex = event.params.node.toHexString();
  let resolverAddress = event.address;

  let resolver = getOrCreateResolver(resolverAddress, nodeHex);
  let addrAccount = getOrCreateAccount(event.params.a);
  resolver.addr = addrAccount.id;
  resolver.domain = nodeHex;
  resolver.save();

  // Mirror resolvedAddress on Domain for fast BENS queries
  let domain = Domain.load(nodeHex);
  if (domain) {
    domain.resolvedAddress = addrAccount.id;
    // Ensure Domain.resolver points to this resolver
    domain.resolver = resolver.id;
    domain.save();
  }

  // AddrChanged resolver event
  let eventId =
    event.transaction.hash.toHexString() +
    "-" +
    event.logIndex.toString() +
    "-addrchanged";
  let addrChangedEvent = new AddrChangedEntity(eventId);
  addrChangedEvent.resolver = resolver.id;
  addrChangedEvent.addr = addrAccount.id;
  addrChangedEvent.blockNumber = event.block.number.toI32();
  addrChangedEvent.transactionID = event.transaction.hash;
  addrChangedEvent.save();
}

// ─── NameChanged ─────────────────────────────────────────────────────────────
// Resolver.NameChanged(node, name) — fires when a reverse record name is set.
//
// In the BENS model, reverse resolution is handled by querying Domain entities
// whose parent is the addr.reverse root node. The Domain.name field on the
// reverse node Domain holds the primary name string (e.g. "alice.arc").
//
// This handler updates Domain.name for the reverse node Domain so BENS can
// read it via its reverse_registry technique.

export function handleNameChanged(event: NameChangedEvent): void {
  let nodeHex = event.params.node.toHexString();
  let name = event.params.name;

  if (name.length == 0) return;

  let resolverAddress = event.address;

  // Update the Resolver entity
  let resolver = getOrCreateResolver(resolverAddress, nodeHex);
  resolver.domain = nodeHex;
  resolver.save();

  // Update Domain.name for the reverse node
  // The reverse node Domain was created by handleReverseClaimed in reverseRegistrar.ts
  let domain = Domain.load(nodeHex);
  if (domain) {
    domain.name = name;
    domain.resolver = resolver.id;
    domain.save();
  }

  // NameChanged resolver event
  let eventId =
    event.transaction.hash.toHexString() +
    "-" +
    event.logIndex.toString() +
    "-namechanged";
  let nameChangedEvent = new NameChangedEntity(eventId);
  nameChangedEvent.resolver = resolver.id;
  nameChangedEvent.name = name;
  nameChangedEvent.blockNumber = event.block.number.toI32();
  nameChangedEvent.transactionID = event.transaction.hash;
  nameChangedEvent.save();
}
