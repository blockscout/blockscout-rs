import { Address, Bytes, crypto } from "@graphprotocol/graph-ts";
import { concat, createLegacyEventID } from "./utils";

import {
  ABIChanged as ABIChangedEvent,
  AddrChanged as AddrChangedEvent,
  AddressChanged as AddressChangedEvent,
  AuthorisationChanged as AuthorisationChangedEvent,
  ContenthashChanged as ContenthashChangedEvent,
  DataChanged as DataChangedEvent,
  InterfaceChanged as InterfaceChangedEvent,
  NameChanged as NameChangedEvent,
  PubkeyChanged as PubkeyChangedEvent,
  TextChanged as TextChangedEvent,
  TextChanged1 as TextChangedWithValueEvent,
  VersionChanged as VersionChangedEvent,
} from "./types/Resolver/Resolver";

import {
  AbiChanged,
  Account,
  AddrChanged,
  AuthorisationChanged,
  ContenthashChanged,
  Domain,
  ENSv2Resolver,
  ENSv2ResolverData,
  InterfaceChanged,
  MulticoinAddrChanged,
  NameChanged,
  PubkeyChanged,
  Resolver,
  TextChanged,
  VersionChanged,
} from "./types/schema";

export function handleAddrChanged(event: AddrChangedEvent): void {
  let account = new Account(event.params.a.toHexString());
  account.save();

  let resolver = new Resolver(
    createResolverID(event.params.node, event.address),
  );
  resolver.domain = event.params.node.toHexString();
  resolver.address = event.address;
  resolver.addr = event.params.a.toHexString();
  resolver.save();

  let domain = Domain.load(event.params.node.toHexString());
  if (domain && domain.resolver && domain.resolver! == resolver.id) {
    domain.resolvedAddress = event.params.a.toHexString();
    domain.save();
  }

  let resolverEvent = new AddrChanged(createLegacyEventID(event));
  resolverEvent.resolver = resolver.id;
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.addr = event.params.a.toHexString();
  resolverEvent.save();
}

export function handleMulticoinAddrChanged(event: AddressChangedEvent): void {
  let resolver = getOrCreateResolver(event.params.node, event.address, false);

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

  let resolverEvent = new MulticoinAddrChanged(createLegacyEventID(event));
  resolverEvent.resolver = resolver.id;
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.coinType = coinType;
  resolverEvent.addr = event.params.newAddress;
  resolverEvent.save();
}

export function handleNameChanged(event: NameChangedEvent): void {
  if (event.params.name.indexOf("\u0000") != -1) return;

  const resolver = getOrCreateResolver(event.params.node, event.address, true);

  let resolverEvent = new NameChanged(createLegacyEventID(event));
  resolverEvent.resolver = resolver.id;
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.name = event.params.name;
  resolverEvent.save();
}

export function handleABIChanged(event: ABIChangedEvent): void {
  const resolver = getOrCreateResolver(event.params.node, event.address, true);

  let resolverEvent = new AbiChanged(createLegacyEventID(event));
  resolverEvent.resolver = resolver.id;
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.contentType = event.params.contentType;
  resolverEvent.save();
}

export function handlePubkeyChanged(event: PubkeyChangedEvent): void {
  const resolver = getOrCreateResolver(event.params.node, event.address, true);

  let resolverEvent = new PubkeyChanged(createLegacyEventID(event));
  resolverEvent.resolver = resolver.id;
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.x = event.params.x;
  resolverEvent.y = event.params.y;
  resolverEvent.save();
}

export function handleTextChanged(event: TextChangedEvent): void {
  let resolver = getOrCreateResolver(event.params.node, event.address, false);

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

  let resolverEvent = new TextChanged(createLegacyEventID(event));
  resolverEvent.resolver = resolver.id;
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.key = event.params.key;
  resolverEvent.save();
}

export function handleTextChangedWithValue(
  event: TextChangedWithValueEvent,
): void {
  let resolver = getOrCreateResolver(event.params.node, event.address, false);

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

  let resolverEvent = new TextChanged(createLegacyEventID(event));
  resolverEvent.resolver = resolver.id;
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.key = event.params.key;
  resolverEvent.value = event.params.value;
  resolverEvent.save();
}

export function handleContentHashChanged(event: ContenthashChangedEvent): void {
  let resolver = getOrCreateResolver(event.params.node, event.address, false);
  resolver.contentHash = event.params.hash;
  resolver.save();

  let resolverEvent = new ContenthashChanged(createLegacyEventID(event));
  resolverEvent.resolver = resolver.id;
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.hash = event.params.hash;
  resolverEvent.save();
}

// PublicResolverV2's generic data profile is node-keyed like its classic
// profiles, so it belongs on this existing addressless resolver route. The
// raw data is not recoverable from the event because it is indexed; retain
// the key and update metadata without fabricating a value.
export function handleDataChanged(event: DataChangedEvent): void {
  let resolverId: Bytes = event.address;
  let resolver = ENSv2Resolver.load(resolverId);
  if (resolver == null) {
    resolver = new ENSv2Resolver(resolverId);
    resolver.address = event.address;
    resolver.save();
  }

  let keyHash = Bytes.fromByteArray(
    crypto.keccak256(Bytes.fromUTF8(event.params.key)),
  );
  let id = Bytes.fromByteArray(
    concat(concat(resolverId, event.params.node), keyHash),
  );
  let data = ENSv2ResolverData.load(id);
  if (data == null) {
    data = new ENSv2ResolverData(id);
    data.resolver = resolverId;
    data.node = event.params.node;
  }
  data.key = event.params.key;
  data.blockNumber = event.block.number;
  data.transactionID = event.transaction.hash;
  data.logIndex = event.logIndex;
  data.save();
}

export function handleInterfaceChanged(event: InterfaceChangedEvent): void {
  const resolver = getOrCreateResolver(event.params.node, event.address, true);

  let resolverEvent = new InterfaceChanged(createLegacyEventID(event));
  resolverEvent.resolver = resolver.id;
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.interfaceID = event.params.interfaceID;
  resolverEvent.implementer = event.params.implementer;
  resolverEvent.save();
}

export function handleAuthorisationChanged(
  event: AuthorisationChangedEvent,
): void {
  const resolver = getOrCreateResolver(event.params.node, event.address, true);

  let resolverEvent = new AuthorisationChanged(createLegacyEventID(event));
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.resolver = resolver.id;
  resolverEvent.owner = event.params.owner;
  resolverEvent.target = event.params.target;
  resolverEvent.isAuthorized = event.params.isAuthorised;
  resolverEvent.save();
}

export function handleVersionChanged(event: VersionChangedEvent): void {
  let resolverEvent = new VersionChanged(createLegacyEventID(event));
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.resolver = createResolverID(event.params.node, event.address);
  resolverEvent.version = event.params.newVersion;
  resolverEvent.save();

  let domain = Domain.load(event.params.node.toHexString());
  if (domain && domain.resolver && domain.resolver! == resolverEvent.resolver) {
    domain.resolvedAddress = null;
    domain.save();
  }

  let resolver = getOrCreateResolver(event.params.node, event.address, false);
  resolver.addr = null;
  resolver.contentHash = null;
  resolver.texts = null;
  resolver.coinTypes = null;
  resolver.save();
}

function getOrCreateResolver(
  node: Bytes,
  address: Address,
  saveOnNew: boolean,
): Resolver {
  let id = createResolverID(node, address);
  let resolver = Resolver.load(id);
  if (resolver == null) {
    resolver = new Resolver(id);
    resolver.domain = node.toHexString();
    resolver.address = address;
    if (saveOnNew) {
      resolver.save();
    }
  }
  return resolver as Resolver;
}

export function createResolverID(node: Bytes, resolver: Address): string {
  return resolver.toHexString().concat("-").concat(node.toHexString());
}
