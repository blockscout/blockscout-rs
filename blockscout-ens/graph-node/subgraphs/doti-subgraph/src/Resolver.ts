import { Address, Bytes, ethereum } from "@graphprotocol/graph-ts";
import {
  TextChanged as TextChangedEvent,
  PubkeyChanged as PubkeyChangedEvent,
  NameChanged as NameChangedEvent,
  InterfaceChanged as InterfaceChangedEvent,
  ContenthashChanged as ContenthashChangedEvent,
  AddrChanged as AddrChangedEvent,
  AddressChanged as AddressChangedEvent,
  ABIChanged as ABIChangedEvent,
} from "../generated/Resolver/Resolver";

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
} from "../generated/schema";
import {
  COIN_TYPE,
  COIN_TYPE_BIGINT,
  createEventID,
  isNetworkCoinType,
  maybeSaveDomainName,
  safeAddress,
} from "./utils";

/**
 * Handles legacy AddrChanged events to update the resolved Ethereum address.
 * @param event The AddrChanged blockchain event.
 */
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
    domain.resolvedAddress = safeAddress(event.params.a);
    domain.save();
  }

  let resolverEvent = new AddrChanged(createEventID(event));
  resolverEvent.resolver = resolver.id;
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.addr = event.params.a.toHexString();
  resolverEvent.save();
}

/**
 * Handles multicoin AddressChanged events (EIP-2304) for EVM and multichain addresses.
 * @param event The AddressChanged blockchain event.
 */
export function handleMulticoinAddrChanged(event: AddressChangedEvent): void {
  let coinType = event.params.coinType;
  if (isNetworkCoinType(coinType)) {
    let account = new Account(event.params.newAddress.toHexString());
    account.save();

    let resolver = new Resolver(
      createResolverID(event.params.node, event.address)
    );
    resolver.domain = event.params.node.toHexString();
    resolver.address = event.address;
    resolver.addr = event.params.newAddress.toHexString();
    resolver.save();

    let domain = Domain.load(event.params.node.toHexString());
    if (domain && domain.resolver == resolver.id) {
      domain.resolvedAddress = safeAddress(event.params.newAddress);
      domain.save();
    }

    let resolverEvent = new AddrChanged(createEventID(event));
    resolverEvent.resolver = resolver.id;
    resolverEvent.blockNumber = event.block.number.toI32();
    resolverEvent.transactionID = event.transaction.hash;
    resolverEvent.addr = event.params.newAddress.toHexString();
    resolverEvent.save();
  } else {
    _handleMulticoinAddrChanged(event);
  }
}

/**
 * Internal helper to record secondary non-native multicoin address changes.
 * @param event The AddressChanged blockchain event.
 */
function _handleMulticoinAddrChanged(event: AddressChangedEvent): void {
  let resolver = getOrCreateResolver(event.params.node, event.address);
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

/**
 * Handles NameChanged reverse resolution and forward name mapping events.
 * @param event The NameChanged blockchain event.
 */
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

/**
 * Handles ABIChanged resolver events.
 * @param event The ABIChanged blockchain event.
 */
export function handleABIChanged(event: ABIChangedEvent): void {
  let resolverEvent = new AbiChanged(createEventID(event));
  resolverEvent.resolver = createResolverID(event.params.node, event.address);
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.contentType = event.params.contentType;
  resolverEvent.save();
}

/**
 * Handles PubkeyChanged resolver events.
 * @param event The PubkeyChanged blockchain event.
 */
export function handlePubkeyChanged(event: PubkeyChangedEvent): void {
  let resolverEvent = new PubkeyChanged(createEventID(event));
  resolverEvent.resolver = createResolverID(event.params.node, event.address);
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.x = event.params.x;
  resolverEvent.y = event.params.y;
  resolverEvent.save();
}

/**
 * Handles TextChanged metadata record events (e.g. avatar, twitter, github).
 * @param event The TextChanged blockchain event.
 */
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
  resolverEvent.value = event.params.value;
  resolverEvent.save();
}

/**
 * Handles ContenthashChanged decentralized website and content hash events.
 * @param event The ContenthashChanged blockchain event.
 */
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

/**
 * Handles InterfaceChanged EIP-1820 interface support events.
 * @param event The InterfaceChanged blockchain event.
 */
export function handleInterfaceChanged(event: InterfaceChangedEvent): void {
  let resolverEvent = new InterfaceChanged(createEventID(event));
  resolverEvent.resolver = createResolverID(event.params.node, event.address);
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.interfaceID = event.params.interfaceID;
  resolverEvent.implementer = event.params.implementer;
  resolverEvent.save();
}

/**
 * Retrieves an existing Resolver entity or creates a new instance.
 * @param node The node hash identifier bytes.
 * @param address The resolver contract address.
 * @returns The Resolver entity.
 */
function getOrCreateResolver(node: Bytes, address: Address): Resolver {
  let id = createResolverID(node, address);
  let resolver = Resolver.load(id);
  if (resolver === null) {
    resolver = new Resolver(id);
    resolver.domain = node.toHexString();
    resolver.address = address;
    resolver.save();
  }
  return resolver;
}

/**
 * Formats a composite Resolver entity ID string.
 * @param node Node identifier bytes.
 * @param resolver Contract address.
 * @returns Composite ID string formatted as 'address-node'.
 */
function createResolverID(node: Bytes, resolver: Address): string {
  return resolver
    .toHexString()
    .concat("-")
    .concat(node.toHexString());
}
