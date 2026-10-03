import {
  Address,
  BigInt,
  Bytes,
  crypto,
  ethereum,
} from "@graphprotocol/graph-ts";

import {
  AddrChanged as AddrChangedEvent,
  AddressChanged as AddressChangedEvent,
  ContenthashChanged as ContenthashChangedEvent,
  NameRegistered as NameRegisteredEvent,
  NameRenewed as NameRenewedEvent,
  PrimaryNameSet as PrimaryNameSetEvent,
  SubdomainRegistered as SubdomainRegisteredEvent,
  Transfer as TransferEvent,
} from "../generated/NameNFT/NameNFT";
import {
  Account,
  AddrChanged,
  ContenthashChanged,
  Domain,
  DomainState,
  ExpiryExtended,
  MulticoinAddrChanged,
  NameRegistered,
  NameRenewed,
  PrimaryNameRecord,
  Registration,
  Resolver,
  TokenIdToDomain,
  Transfer,
} from "../generated/schema";
import {
  EMPTY_ADDRESS,
  GWEI_NODE,
  createEventID,
  createResolverID,
  tokenIdToDomainID,
} from "./utils";

const BIG_INT_ZERO = BigInt.fromI32(0);

function createOrLoadAccount(address: Address): Account {
  let id = address.toHexString();
  let account = Account.load(id);
  if (account === null) {
    account = new Account(id);
    account.save();
  }
  return account;
}

function createOrLoadAccountByID(id: string): Account {
  let account = Account.load(id);
  if (account === null) {
    account = new Account(id);
    account.save();
  }
  return account;
}

function createOrLoadState(domainID: string): DomainState {
  let state = DomainState.load(domainID);
  if (state === null) {
    state = new DomainState(domainID);
    state.hasExplicitAddress = false;
    state.registrationVersion = 0;
    state.parentRegistrationVersion = 0;
  }
  return state;
}

function primaryRecord(address: string): PrimaryNameRecord {
  let record = PrimaryNameRecord.load(address);
  if (record === null) {
    record = new PrimaryNameRecord(address);
    record.resolved_address = address;
  }
  return record;
}

function invalidatePrimary(address: string, domainID: string): void {
  let record = PrimaryNameRecord.load(address);
  if (record === null) return;
  let selectedDomain = record.domain_id;
  if (selectedDomain !== null && selectedDomain == domainID) {
    record.domain_name = null;
    record.save();
  }
}

function activatePrimary(address: string, domain: Domain): void {
  let record = PrimaryNameRecord.load(address);
  if (record === null) return;
  let selectedDomain = record.domain_id;
  if (selectedDomain !== null && selectedDomain == domain.id && domain.name !== null) {
    record.domain_name = domain.name;
    record.save();
  }
}

function setResolvedAddress(domain: Domain, address: string): void {
  let previous = domain.resolvedAddress;
  if (previous !== null && previous != address) {
    invalidatePrimary(previous, domain.id);
  }
  domain.resolvedAddress = address;
  activatePrimary(address, domain);
}

function createResolver(event: ethereum.Event, domain: Domain): Resolver {
  let resolver = new Resolver(createResolverID(event, domain.id));
  resolver.domain = domain.id;
  resolver.address = event.address;
  resolver.save();
  return resolver;
}

function loadResolver(domain: Domain): Resolver | null {
  if (domain.resolver === null) return null;
  return Resolver.load(domain.resolver!);
}

export function handleNameRegistered(event: NameRegisteredEvent): void {
  let domainID = tokenIdToDomainID(event.params.tokenId);
  let owner = createOrLoadAccount(event.params.owner);
  let domain = Domain.load(domainID);

  let isNewDomain = domain === null;
  if (domain === null) {
    domain = new Domain(domainID);
    domain.subdomainCount = 0;
    domain.storedOffchain = false;
    domain.resolvedWithWildcard = false;
  }

  domain.name = event.params.label.concat(".gwei");
  domain.labelName = event.params.label;
  domain.labelhash = Bytes.fromByteArray(
    crypto.keccak256(Bytes.fromUTF8(event.params.label))
  );
  if (isNewDomain || !event.params.expiresAt.equals(BIG_INT_ZERO)) {
    domain.parent = GWEI_NODE;
  }
  domain.createdAt = event.block.timestamp;
  domain.subdomainCount = 0;
  domain.owner = owner.id;
  domain.registrant = owner.id;
  if (event.params.expiresAt.equals(BIG_INT_ZERO)) {
    domain.expiryDate = null;
  } else {
    domain.expiryDate = event.params.expiresAt;
  }
  domain.isMigrated = true;
  domain.tokenId = event.params.tokenId;

  let resolver = createResolver(event, domain);
  domain.resolver = resolver.id;

  let state = createOrLoadState(domain.id);
  state.registrationVersion = state.registrationVersion + 1;
  state.parentRegistrationVersion = 0;
  state.hasExplicitAddress = false;
  state.explicitAddress = null;
  state.save();

  setResolvedAddress(domain, owner.id);
  domain.save();

  let registration = new Registration(domain.id);
  registration.domain = domain.id;
  registration.registrationDate = event.block.timestamp;
  registration.expiryDate = event.params.expiresAt;
  registration.cost = event.transaction.value;
  registration.registrant = owner.id;
  registration.labelName = event.params.label;
  registration.save();

  let registered = new NameRegistered(createEventID(event));
  registered.registration = registration.id;
  registered.blockNumber = event.block.number.toI32();
  registered.transactionID = event.transaction.hash;
  registered.registrant = owner.id;
  registered.expiryDate = event.params.expiresAt;
  registered.save();

  let tokenMapping = new TokenIdToDomain(event.params.tokenId.toString());
  tokenMapping.domain = domain.id;
  tokenMapping.save();
}

export function handleSubdomainRegistered(
  event: SubdomainRegisteredEvent
): void {
  let domainID = tokenIdToDomainID(event.params.tokenId);
  let parentID = tokenIdToDomainID(event.params.parentId);
  let domain = Domain.load(domainID);
  let parent = Domain.load(parentID);
  if (domain === null || parent === null || parent.name === null) return;

  let state = createOrLoadState(domain.id);
  let parentState = createOrLoadState(parent.id);
  let previousParent = domain.parent;
  if (
    previousParent === null ||
    previousParent != parent.id ||
    state.parentRegistrationVersion !== parentState.registrationVersion
  ) {
    parent.subdomainCount = parent.subdomainCount + 1;
    parent.save();
  }

  domain.parent = parent.id;
  domain.name = event.params.label.concat(".").concat(parent.name!);
  // GNS subdomains inherit activity from their parent chain. Snapshotting the
  // current parent expiry makes BENS's time-based active filter conservative.
  domain.expiryDate = parent.expiryDate;
  domain.save();
  state.parentRegistrationVersion = parentState.registrationVersion;
  state.save();

  let registration = Registration.load(domain.id);
  if (registration !== null && parent.expiryDate !== null) {
    registration.expiryDate = parent.expiryDate!;
    registration.save();
  }

  if (domain.resolvedAddress !== null) {
    activatePrimary(domain.resolvedAddress!, domain);
  }
}

export function handleNameRenewed(event: NameRenewedEvent): void {
  let domainID = tokenIdToDomainID(event.params.tokenId);
  let domain = Domain.load(domainID);
  let registration = Registration.load(domainID);
  if (domain === null || registration === null) return;

  domain.expiryDate = event.params.newExpiresAt;
  domain.save();
  registration.expiryDate = event.params.newExpiresAt;
  registration.save();

  let renewed = new NameRenewed(createEventID(event));
  renewed.registration = registration.id;
  renewed.blockNumber = event.block.number.toI32();
  renewed.transactionID = event.transaction.hash;
  renewed.expiryDate = event.params.newExpiresAt;
  renewed.save();

  let extended = new ExpiryExtended(createEventID(event));
  extended.domain = domain.id;
  extended.blockNumber = event.block.number.toI32();
  extended.transactionID = event.transaction.hash;
  extended.expiryDate = event.params.newExpiresAt;
  extended.save();
}

export function handleTransfer(event: TransferEvent): void {
  let domainID = tokenIdToDomainID(event.params.id);
  let domain = Domain.load(domainID);
  // The mint Transfer is emitted before NameRegistered, which supplies the
  // label and complete domain data.
  if (domain === null) return;

  let recipient = createOrLoadAccount(event.params.to);
  let state = createOrLoadState(domain.id);
  domain.owner = recipient.id;
  domain.registrant = recipient.id;
  if (state.hasExplicitAddress && state.explicitAddress !== null) {
    setResolvedAddress(domain, state.explicitAddress!);
  } else {
    setResolvedAddress(domain, recipient.id);
  }
  domain.save();
  state.save();

  let registration = Registration.load(domain.id);
  if (registration !== null) {
    registration.registrant = recipient.id;
    registration.save();
  }

  let transferred = new Transfer(createEventID(event));
  transferred.domain = domain.id;
  transferred.blockNumber = event.block.number.toI32();
  transferred.transactionID = event.transaction.hash;
  transferred.owner = recipient.id;
  transferred.save();
}

export function handleAddrChanged(event: AddrChangedEvent): void {
  let domain = Domain.load(event.params.node.toHexString());
  if (domain === null) return;

  let eventAccount = createOrLoadAccount(event.params.addr);
  let state = createOrLoadState(domain.id);
  if (event.params.addr.toHexString() == EMPTY_ADDRESS) {
    state.hasExplicitAddress = false;
    state.explicitAddress = null;
    setResolvedAddress(domain, domain.owner);
  } else {
    state.hasExplicitAddress = true;
    state.explicitAddress = eventAccount.id;
    setResolvedAddress(domain, eventAccount.id);
  }
  state.save();

  let resolver = loadResolver(domain);
  if (resolver === null) return;
  resolver.addr = eventAccount.id;
  resolver.save();
  domain.save();

  let changed = new AddrChanged(createEventID(event));
  changed.resolver = resolver.id;
  changed.blockNumber = event.block.number.toI32();
  changed.transactionID = event.transaction.hash;
  changed.addr = eventAccount.id;
  changed.save();
}

export function handleAddressChanged(event: AddressChangedEvent): void {
  let domain = Domain.load(event.params.node.toHexString());
  if (domain === null) return;
  let resolver = loadResolver(domain);
  if (resolver === null) return;

  let coinTypes = resolver.coinTypes;
  if (coinTypes === null) {
    resolver.coinTypes = [event.params.coinType];
  } else if (!coinTypes.includes(event.params.coinType)) {
    let updated = coinTypes;
    updated.push(event.params.coinType);
    resolver.coinTypes = updated;
  }
  resolver.save();

  let changed = new MulticoinAddrChanged(createEventID(event));
  changed.resolver = resolver.id;
  changed.blockNumber = event.block.number.toI32();
  changed.transactionID = event.transaction.hash;
  changed.coinType = event.params.coinType;
  changed.addr = event.params.addr;
  changed.save();
}

export function handleContenthashChanged(event: ContenthashChangedEvent): void {
  let domain = Domain.load(event.params.node.toHexString());
  if (domain === null) return;
  let resolver = loadResolver(domain);
  if (resolver === null) return;

  resolver.contentHash = event.params.contenthash;
  resolver.save();

  let changed = new ContenthashChanged(createEventID(event));
  changed.resolver = resolver.id;
  changed.blockNumber = event.block.number.toI32();
  changed.transactionID = event.transaction.hash;
  changed.hash = event.params.contenthash;
  changed.save();
}

export function handlePrimaryNameSet(event: PrimaryNameSetEvent): void {
  let account = createOrLoadAccount(event.params.addr);
  let record = primaryRecord(account.id);

  if (event.params.tokenId.equals(BIG_INT_ZERO)) {
    record.domain_id = null;
    record.domain_name = null;
    record.save();
    return;
  }

  let domain = Domain.load(tokenIdToDomainID(event.params.tokenId));
  if (domain === null) return;
  record.domain_id = domain.id;
  let resolvedAddress = domain.resolvedAddress;
  if (resolvedAddress !== null && resolvedAddress == account.id) {
    record.domain_name = domain.name;
  } else {
    record.domain_name = null;
  }
  record.save();
}
