import { Address, Bytes, ethereum } from "@graphprotocol/graph-ts";
import {
  AuthorisationChanged as AuthorisationChangedEvent,
  TextChanged as TextChangedEvent,
  TextChanged1 as TextChanged1Event,
  PubkeyChanged as PubkeyChangedEvent,
  NameChanged as NameChangedEvent,
  InterfaceChanged as InterfaceChangedEvent,
  ContenthashChanged as ContenthashChangedEvent,
  AddrChanged as AddrChangedEvent,
  AddressChanged as AddressChangedEvent,
  ABIChanged as ABIChangedEvent,
  VersionChanged as VersionChangedEvent
} from "../generated/{{ cookiecutter.resolver_name }}/{{ cookiecutter.resolver_name }}"

import {
  AuthorisationChanged,
  TextChanged,
  PubkeyChanged,
  NameChanged,
  InterfaceChanged,
  ContenthashChanged,
  AddrChanged,
  AbiChanged,
  VersionChanged,
  Account,
  Resolver,
  Domain,
  MulticoinAddrChanged
} from "../generated/schema"
import { createEventID, maybeSaveDomainName } from "./utils";

export function handleAddrChanged(event: AddrChangedEvent): void {
  let account = new Account(event.params.a.toHexString());
  account.save();

  let resolver = new Resolver(
    createResolverID(event.params.node, event.address)
  );
  resolver.domain = event.params.node.toHexString();
  resolver.address = event.address;
  resolver.addr = event.params.a.toHexString();
  resolver.save();

  let domain = Domain.load(event.params.node.toHexString());
  if (domain && domain.resolver == resolver.id) {
    domain.resolvedAddress = event.params.a.toHexString();
    domain.save();
  }

  let resolverEvent = new AddrChanged(createEventID(event));
  resolverEvent.resolver = resolver.id;
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.addr = event.params.a.toHexString();
  resolverEvent.save();
}

export function handleMulticoinAddrChanged(event: AddressChangedEvent): void {
  let resolver = getOrCreateResolver(event.params.node, event.address);


  // TODO: if protocol doesn't emit AddrChanged event you can check that
  // coin_type is equal to native coin type and update resolved_address of domain manually
  let coinType = event.params.coinType;
  
  if (resolver.coinTypes == null) {
    resolver.coinTypes = [coinType];
    resolver.save();
  } else {
    let coinTypes = resolver.coinTypes!;
    if (!coinTypes.includes(coinType)) {
      coinTypes.push(coinType);
      resolver.coinTypes = coinTypes;
      resolver.save();
    }
  }

  let resolverEvent = new MulticoinAddrChanged(createEventID(event));
  resolverEvent.resolver = resolver.id;
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.coinType = coinType;
  resolverEvent.addr = event.params.newAddress;
  resolverEvent.save();
}

export function handleNameChanged(event: NameChangedEvent): void {
  const name = event.params.name;
  if (name.indexOf("\u0000") != -1) return;
  maybeSaveDomainName(name);

  let resolverEvent = new NameChanged(createEventID(event));
  resolverEvent.resolver = createResolverID(event.params.node, event.address);
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.name = event.params.name;
  resolverEvent.save();
}

export function handleABIChanged(event: ABIChangedEvent): void {
  let resolverEvent = new AbiChanged(createEventID(event));
  resolverEvent.resolver = createResolverID(event.params.node, event.address);
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.contentType = event.params.contentType;
  resolverEvent.save();
}

export function handlePubkeyChanged(event: PubkeyChangedEvent): void {
  let resolverEvent = new PubkeyChanged(createEventID(event));
  resolverEvent.resolver = createResolverID(event.params.node, event.address);
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.x = event.params.x;
  resolverEvent.y = event.params.y;
  resolverEvent.save();
}

export function handleTextChanged(event: TextChangedEvent): void {
  let resolver = getOrCreateResolver(event.params.node, event.address);

  let key = event.params.key;
  if (resolver.texts == null) {
    resolver.texts = [key];
    resolver.save();
  } else {
    let texts = resolver.texts!;
    if (!texts.includes(key)) {
      texts.push(key);
      resolver.texts = texts;
      resolver.save();
    }
  }

  let resolverEvent = new TextChanged(createEventID(event));
  resolverEvent.resolver = createResolverID(event.params.node, event.address);
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.key = event.params.key;
  resolverEvent.save();
}

export function handleContentHashChanged(event: ContenthashChangedEvent): void {
  let resolver = getOrCreateResolver(event.params.node, event.address);
  resolver.contentHash = event.params.hash;
  resolver.save();

  let resolverEvent = new ContenthashChanged(createEventID(event));
  resolverEvent.resolver = createResolverID(event.params.node, event.address);
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.hash = event.params.hash;
  resolverEvent.save();
}

export function handleInterfaceChanged(event: InterfaceChangedEvent): void {
  let resolverEvent = new InterfaceChanged(createEventID(event));
  resolverEvent.resolver = createResolverID(event.params.node, event.address);
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.interfaceID = event.params.interfaceID;
  resolverEvent.implementer = event.params.implementer;
  resolverEvent.save();
}

export function handleAuthorisationChanged(
  event: AuthorisationChangedEvent
): void {
  let resolverEvent = new AuthorisationChanged(createEventID(event));
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.resolver = createResolverID(event.params.node, event.address);
  resolverEvent.owner = event.params.owner;
  resolverEvent.target = event.params.target;
  resolverEvent.isAuthorized = event.params.isAuthorised;
  resolverEvent.save(); 
}

export function handleVersionChanged(event: VersionChangedEvent): void {
  let resolverEvent = new VersionChanged(createEventID(event));
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.resolver = createResolverID(event.params.node, event.address);
  resolverEvent.version = event.params.newVersion;
  resolverEvent.save();

  let domain = Domain.load(event.params.node.toHexString());
  if (domain && domain.resolver === resolverEvent.resolver) {
    domain.resolvedAddress = null;
    domain.save();
  }

  let resolver = getOrCreateResolver(event.params.node, event.address);
  resolver.addr = null;
  resolver.contentHash = null;
  resolver.texts = null;
  resolver.coinTypes = null;
  resolver.save();
}

function getOrCreateResolver(node: Bytes, address: Address): Resolver {
  let id = createResolverID(node, address);
  let resolver = Resolver.load(id);
  if (resolver === null) {
    resolver = new Resolver(id);
    resolver.domain = node.toHexString();
    resolver.address = address;
  }
  return resolver as Resolver;
}

function createResolverID(node: Bytes, resolver: Address): string {
  return resolver
    .toHexString()
    .concat("-")
    .concat(node.toHexString());
}
