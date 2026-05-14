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
// Resolver.NameChanged(node, name) — fires when a reverse record primary name
// is set via ReverseRegistrar.setName() or Resolver.setName().
//
// BENS reverse resolution flow:
//   1. BENS queries name_changed rows joined to domain via domain.resolver.
//   2. BENS then joins name_changed.name against the forward domain table to
//      find the canonical forward domain for the primary name.
//   3. The addr_reverse_names materialized view is built from this join.
//
// CRITICAL: Domain.name on the reverse node MUST remain "<address>.addr.reverse".
// If Domain.name is overwritten with the primary name (e.g. "flowpay.arc"),
// BENS produces duplicate reversed_domain_id rows and fails the unique index
// on addr_reverse_names.
//
// Correct behaviour:
//   - Store the primary name ONLY in NameChanged.name (the resolver event).
//   - Keep Domain.name as "<address>.addr.reverse" (set by handleReverseClaimed).
//   - Link Domain.resolver = resolver.id so BENS can join via the resolver.

export function handleNameChanged(event: NameChangedEvent): void {
  let nodeHex = event.params.node.toHexString();
  let name = event.params.name;

  if (name.length == 0) return;

  let resolverAddress = event.address;

  // Create/update the Resolver entity and link it to the domain node
  let resolver = getOrCreateResolver(resolverAddress, nodeHex);
  resolver.domain = nodeHex;
  resolver.save();

  // Link the reverse Domain to this resolver so BENS can join through it.
  // Do NOT overwrite Domain.name — it must stay as "<address>.addr.reverse".
  let domain = Domain.load(nodeHex);
  if (domain) {
    domain.resolver = resolver.id;
    domain.save();
  }

  // Store the primary name in the NameChanged resolver event.
  // BENS reads name_changed.name from here to resolve the primary name.
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
