import { Address, BigInt, Bytes } from "@graphprotocol/graph-ts";

import {
  ABIChanged as ABIChangedEvent,
  Approved as ApprovedEvent,
  ContentHashChanged as ContentHashChangedEvent,
  InterfaceChanged as InterfaceChangedEvent,
  NameChanged as NameChangedEvent,
  PubkeyChanged as PubkeyChangedEvent,
  PublicResolver,
  TextChanged as TextChangedEvent,
  VersionChanged as VersionChangedEvent,
} from "./types/PublicResolver/PublicResolver";
import {
  AbiChanged,
  AuthorisationChanged,
  ContenthashChanged,
  Domain,
  InterfaceChanged,
  NameChanged,
  PubkeyChanged,
  Resolver,
  TextChanged,
  VersionChanged,
} from "./types/schema";
import { loadDomain } from "./domain";
import { syncReverseNode } from "./primaryName";
import { createEventID } from "./utils";

// Adapted from the Blockscout ens-subgraph resolver mappings. RNS differs in
// three ways: setAddr always reverts (no AddrChanged, see domain.ts), there is
// no multicoin AddressChanged, and the content hash event is spelled
// ContentHashChanged.

function createResolverID(node: Bytes, resolver: Address): string {
  return resolver
    .toHexString()
    .concat("-")
    .concat(node.toHexString());
}

function getOrCreateResolver(node: Bytes, address: Address): Resolver {
  let id = createResolverID(node, address);
  let resolver = Resolver.load(id);
  if (resolver == null) {
    resolver = new Resolver(id);
    resolver.domain = node.toHexString();
    resolver.address = address;
  }
  return resolver;
}

function nodeToBigInt(node: Bytes): BigInt {
  let littleEndian = new Uint8Array(node.length);
  for (let i = 0; i < node.length; i++) {
    littleEndian[i] = node[node.length - 1 - i];
  }
  return BigInt.fromUnsignedBytes(Bytes.fromUint8Array(littleEndian));
}

// After a reverse name or a reverse node's records change, refresh the primary
// name of its address, but only when this resolver is the node's resolver.
function syncIfCurrentResolver(domain: Domain | null, resolverId: string): void {
  if (domain == null) return;
  let current = domain.resolver;
  if (current !== null && current == resolverId) {
    syncReverseNode(domain);
  }
}

export function handleNameChanged(event: NameChangedEvent): void {
  if (event.params.name.indexOf("\u0000") != -1) return;

  // A reverse node claimed before the L2 genesis may not be loaded yet; load it
  // so the primary name can be computed. Its name is then overwritten below
  // with the stored value this event carries.
  let domain = Domain.load(event.params.node.toHexString());
  if (domain == null) {
    let registry = PublicResolver.bind(event.address).try_getRNSUnified();
    if (!registry.reverted) {
      domain = loadDomain(
        nodeToBigInt(event.params.node),
        registry.value,
        event.block
      );
    }
  }

  let resolver = getOrCreateResolver(event.params.node, event.address);
  resolver.name = event.params.name;
  resolver.save();

  let resolverEvent = new NameChanged(createEventID(event));
  resolverEvent.resolver = resolver.id;
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.name = event.params.name;
  resolverEvent.save();

  syncIfCurrentResolver(domain, resolver.id);
}

// clearRecords() bumps the record version, which empties every record of the
// node, the name included. Unlike ENS the resolved address stays: RNS addr()
// is ownerOf(), not a stored record.
export function handleVersionChanged(event: VersionChangedEvent): void {
  let resolver = getOrCreateResolver(event.params.node, event.address);
  resolver.name = null;
  resolver.contentHash = null;
  resolver.texts = null;
  resolver.save();

  let resolverEvent = new VersionChanged(createEventID(event));
  resolverEvent.resolver = resolver.id;
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.version = event.params.newVersion;
  resolverEvent.save();

  syncIfCurrentResolver(
    Domain.load(event.params.node.toHexString()),
    resolver.id
  );
}

export function handleABIChanged(event: ABIChangedEvent): void {
  let resolver = getOrCreateResolver(event.params.node, event.address);
  resolver.save();

  let resolverEvent = new AbiChanged(createEventID(event));
  resolverEvent.resolver = resolver.id;
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.contentType = event.params.contentType;
  resolverEvent.save();
}

export function handlePubkeyChanged(event: PubkeyChangedEvent): void {
  let resolver = getOrCreateResolver(event.params.node, event.address);
  resolver.save();

  let resolverEvent = new PubkeyChanged(createEventID(event));
  resolverEvent.resolver = resolver.id;
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.x = event.params.x;
  resolverEvent.y = event.params.y;
  resolverEvent.save();
}

export function handleTextChanged(event: TextChangedEvent): void {
  let key = event.params.key;
  // Postgres rejects NUL in text columns; storing it would halt indexing.
  if (key.indexOf("\u0000") != -1) return;

  let resolver = getOrCreateResolver(event.params.node, event.address);
  let texts = resolver.texts;
  if (texts == null) {
    resolver.texts = [key];
  } else if (!texts.includes(key)) {
    texts.push(key);
    resolver.texts = texts;
  }
  resolver.save();

  let value = event.params.value;
  let resolverEvent = new TextChanged(createEventID(event));
  resolverEvent.resolver = resolver.id;
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.key = key;
  resolverEvent.value = value.indexOf("\u0000") == -1 ? value : null;
  resolverEvent.save();
}

export function handleContentHashChanged(event: ContentHashChangedEvent): void {
  let resolver = getOrCreateResolver(event.params.node, event.address);
  resolver.contentHash = event.params.hash;
  resolver.save();

  let resolverEvent = new ContenthashChanged(createEventID(event));
  resolverEvent.resolver = resolver.id;
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.hash = event.params.hash;
  resolverEvent.save();
}

export function handleInterfaceChanged(event: InterfaceChangedEvent): void {
  let resolver = getOrCreateResolver(event.params.node, event.address);
  resolver.save();

  let resolverEvent = new InterfaceChanged(createEventID(event));
  resolverEvent.resolver = resolver.id;
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.interfaceID = event.params.interfaceID;
  resolverEvent.implementer = event.params.implementer;
  resolverEvent.save();
}

export function handleApproved(event: ApprovedEvent): void {
  let resolver = getOrCreateResolver(event.params.node, event.address);
  resolver.save();

  let resolverEvent = new AuthorisationChanged(createEventID(event));
  resolverEvent.resolver = resolver.id;
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.owner = event.params.owner;
  resolverEvent.target = event.params.delegate;
  resolverEvent.isAuthorized = event.params.approved;
  resolverEvent.save();
}
