import { Address, BigInt, Bytes } from '@graphprotocol/graph-ts';
import {
  BensNameRegistered,
  BensNameRenewed,
  BensResolverUpdated,
  BensNameTransferred as BensNameTransferredEvent,
  BensPrimaryNameSet
} from '../generated/HoodNameRegistry/HoodNameRegistry';
import {
  Account,
  AddrChanged,
  Domain,
  NameRegistered,
  NameRenewed,
  NameTransferred,
  NewResolver,
  Registration,
  Resolver,
  Transfer
} from '../generated/schema';

const ZERO_NODE = '0x0000000000000000000000000000000000000000000000000000000000000000';

function accountId(address: Address): string {
  return address.toHexString().toLowerCase();
}

function nodeId(node: Bytes): string {
  return node.toHexString().toLowerCase();
}

function eventId(txHash: Bytes, logIndex: BigInt, suffix: string): string {
  return txHash.toHexString() + '-' + logIndex.toString() + '-' + suffix;
}

function txId(txHash: Bytes): Bytes {
  return txHash;
}

function blockNumber(block: BigInt): i32 {
  return block.toI32();
}

function loadOrCreateAccount(address: Address): Account {
  const id = accountId(address);
  let account = Account.load(id);
  if (account == null) {
    account = new Account(id);
    account.save();
  }
  return account as Account;
}

function loadOrCreateDomain(node: Bytes, label: string, name: string, timestamp: BigInt): Domain {
  const id = nodeId(node);
  let domain = Domain.load(id);
  if (domain == null) {
    domain = new Domain(id);
    domain.subdomainCount = 0;
    domain.isMigrated = false;
    domain.createdAt = timestamp;
    domain.storedOffchain = false;
    domain.resolvedWithWildcard = false;
  }
  domain.name = name;
  domain.labelName = label;
  domain.labelhash = node;
  domain.tokenId = BigInt.fromUnsignedBytes(changetype<Bytes>(node));
  return domain as Domain;
}

function loadOrCreateResolver(domainId: string, resolverAddress: Address, resolvedAccount: Account): Resolver {
  const id = resolverAddress.toHexString().toLowerCase() + '-' + domainId;
  let resolver = Resolver.load(id);
  if (resolver == null) {
    resolver = new Resolver(id);
    resolver.texts = [];
    resolver.coinTypes = [];
  }
  resolver.domain = domainId;
  resolver.address = resolverAddress;
  resolver.addr = resolvedAccount.id;
  resolver.save();
  return resolver as Resolver;
}

function loadOrCreateRegistration(domain: Domain, registrant: Account, label: string, timestamp: BigInt, expiryDate: BigInt): Registration {
  let registration = Registration.load(domain.id);
  if (registration == null) {
    registration = new Registration(domain.id);
    registration.domain = domain.id;
    registration.registrationDate = timestamp;
    registration.registrant = registrant.id;
  }
  registration.expiryDate = expiryDate;
  registration.labelName = label;
  registration.save();
  return registration as Registration;
}

export function handleBensNameRegistered(event: BensNameRegistered): void {
  const owner = loadOrCreateAccount(event.params.owner);
  const resolved = loadOrCreateAccount(event.params.resolvedAddress);
  const domain = loadOrCreateDomain(event.params.node, event.params.label, event.params.name, event.block.timestamp);
  const resolver = loadOrCreateResolver(domain.id, event.params.resolvedAddress, resolved);

  domain.owner = owner.id;
  domain.registrant = owner.id;
  domain.resolvedAddress = resolved.id;
  domain.resolver = resolver.id;
  domain.expiryDate = event.params.expiresAt;
  domain.save();

  const registration = loadOrCreateRegistration(domain, owner, event.params.label, event.block.timestamp, event.params.expiresAt);

  const domainTransfer = new Transfer(eventId(event.transaction.hash, event.logIndex, 'domain-transfer'));
  domainTransfer.domain = domain.id;
  domainTransfer.blockNumber = blockNumber(event.block.number);
  domainTransfer.transactionID = txId(event.transaction.hash);
  domainTransfer.owner = owner.id;
  domainTransfer.save();

  const nameRegistered = new NameRegistered(eventId(event.transaction.hash, event.logIndex, 'name-registered'));
  nameRegistered.registration = registration.id;
  nameRegistered.blockNumber = blockNumber(event.block.number);
  nameRegistered.transactionID = txId(event.transaction.hash);
  nameRegistered.registrant = owner.id;
  nameRegistered.expiryDate = event.params.expiresAt;
  nameRegistered.save();

  const addrChanged = new AddrChanged(eventId(event.transaction.hash, event.logIndex, 'addr-changed'));
  addrChanged.resolver = resolver.id;
  addrChanged.blockNumber = blockNumber(event.block.number);
  addrChanged.transactionID = txId(event.transaction.hash);
  addrChanged.addr = resolved.id;
  addrChanged.save();
}

export function handleBensNameRenewed(event: BensNameRenewed): void {
  const domain = loadOrCreateDomain(event.params.node, event.params.label, event.params.name, event.block.timestamp);
  domain.expiryDate = event.params.expiresAt;
  domain.save();

  let registration = Registration.load(domain.id);
  if (registration == null) return;
  registration.expiryDate = event.params.expiresAt;
  registration.save();

  const nameRenewed = new NameRenewed(eventId(event.transaction.hash, event.logIndex, 'name-renewed'));
  nameRenewed.registration = registration.id;
  nameRenewed.blockNumber = blockNumber(event.block.number);
  nameRenewed.transactionID = txId(event.transaction.hash);
  nameRenewed.expiryDate = event.params.expiresAt;
  nameRenewed.save();
}

export function handleBensResolverUpdated(event: BensResolverUpdated): void {
  const resolved = loadOrCreateAccount(event.params.resolvedAddress);
  const domain = loadOrCreateDomain(event.params.node, event.params.label, event.params.name, event.block.timestamp);
  const resolver = loadOrCreateResolver(domain.id, event.params.resolvedAddress, resolved);

  domain.resolvedAddress = resolved.id;
  domain.resolver = resolver.id;
  domain.save();

  const newResolver = new NewResolver(eventId(event.transaction.hash, event.logIndex, 'new-resolver'));
  newResolver.domain = domain.id;
  newResolver.blockNumber = blockNumber(event.block.number);
  newResolver.transactionID = txId(event.transaction.hash);
  newResolver.resolver = resolver.id;
  newResolver.save();

  const addrChanged = new AddrChanged(eventId(event.transaction.hash, event.logIndex, 'resolver-addr-changed'));
  addrChanged.resolver = resolver.id;
  addrChanged.blockNumber = blockNumber(event.block.number);
  addrChanged.transactionID = txId(event.transaction.hash);
  addrChanged.addr = resolved.id;
  addrChanged.save();
}

export function handleBensNameTransferred(event: BensNameTransferredEvent): void {
  const newOwner = loadOrCreateAccount(event.params.newOwner);
  const domain = loadOrCreateDomain(event.params.node, event.params.label, event.params.name, event.block.timestamp);
  domain.owner = newOwner.id;
  domain.registrant = newOwner.id;
  domain.save();

  const transfer = new Transfer(eventId(event.transaction.hash, event.logIndex, 'domain-transfer'));
  transfer.domain = domain.id;
  transfer.blockNumber = blockNumber(event.block.number);
  transfer.transactionID = txId(event.transaction.hash);
  transfer.owner = newOwner.id;
  transfer.save();

  let registration = Registration.load(domain.id);
  if (registration != null) {
    registration.registrant = newOwner.id;
    registration.save();

    const nameTransferred = new NameTransferred(eventId(event.transaction.hash, event.logIndex, 'name-transferred'));
    nameTransferred.registration = registration.id;
    nameTransferred.blockNumber = blockNumber(event.block.number);
    nameTransferred.transactionID = txId(event.transaction.hash);
    nameTransferred.newOwner = newOwner.id;
    nameTransferred.save();
  }
}

export function handleBensPrimaryNameSet(event: BensPrimaryNameSet): void {
  if (nodeId(event.params.node) == ZERO_NODE || event.params.name.length == 0) {
    return;
  }

  const wallet = loadOrCreateAccount(event.params.wallet);
  const domain = loadOrCreateDomain(event.params.node, event.params.label, event.params.name, event.block.timestamp);
  domain.resolvedAddress = wallet.id;
  domain.save();
}
