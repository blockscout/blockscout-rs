import { Address, BigInt, Bytes, crypto, ethereum, store } from "@graphprotocol/graph-ts";
import {
  Transfer as TransferEvent,
  NameRegistered as NameRegisteredEvent,
  NameRenewed as NameRenewedEvent,
  NameExpiredFinalized as NameExpiredFinalizedEvent,
  NameTransferred as NameTransferredEvent,
  ResolvedAddressUpdated as ResolvedAddressUpdatedEvent,
  PrimaryNameSet as PrimaryNameSetEvent,
  PrimaryNameCleared as PrimaryNameClearedEvent,
  FreeNameClaimed as FreeNameClaimedEvent,
  RNSRegistryV2,
} from "../generated/RNSRegistryV2/RNSRegistryV2";
import {
  Account, AddrChanged, Domain, ExpiryFinalizedIndexed, FreeClaimIndexed,
  NameRegistered, NameRenewed, NameTransferred, PrimaryChangedIndexed,
  PrimaryNameRecord, Registration, Resolver, TokenIdToDomain, Transfer,
} from "../generated/schema";

const ZERO_ADDRESS = "0x0000000000000000000000000000000000000000";
const ZERO_NODE = "0x0000000000000000000000000000000000000000000000000000000000000000";
const REGISTRY_ADDRESS = "0x08ed77b2ec313c7ad5ce23747b07d483071485f9";
const GRACE_SECONDS = BigInt.fromI32(7776000);

function account(address: Address): Account {
  const id = address.toHexString().toLowerCase();
  let value = Account.load(id);
  if (value == null) {
    value = new Account(id);
    value.save();
  }
  return value;
}

function accountFromString(id: string): Account {
  return account(Address.fromString(id));
}

function isZero(address: Address): bool {
  return address.toHexString().toLowerCase() == ZERO_ADDRESS;
}

function namehash(parent: Bytes, label: string): Bytes {
  const labelhash = changetype<Bytes>(crypto.keccak256(Bytes.fromUTF8(label)));
  return changetype<Bytes>(crypto.keccak256(changetype<Bytes>(parent.concat(labelhash))));
}

function rootId(): string {
  return namehash(Bytes.fromHexString(ZERO_NODE), "rns").toHexString().toLowerCase();
}

function domainId(label: string): string {
  return namehash(Bytes.fromHexString(rootId()), label).toHexString().toLowerCase();
}

function tokenIdFromLabel(label: string): BigInt {
  const hash = crypto.keccak256(Bytes.fromUTF8(label));
  // Graph BigInt.fromUnsignedBytes takes little-endian bytes; EVM uint256 is big-endian.
  const littleEndian = new Bytes(32);
  for (let i = 0; i < 32; i++) littleEndian[i] = hash[31 - i];
  return BigInt.fromUnsignedBytes(littleEndian);
}

function root(timestamp: BigInt): Domain {
  const id = rootId();
  let value = Domain.load(id);
  if (value == null) {
    value = new Domain(id);
    value.name = "rns";
    value.labelhash = changetype<Bytes>(crypto.keccak256(Bytes.fromUTF8("rns")));
    value.subdomainCount = 0;
    value.isMigrated = false;
    value.createdAt = timestamp;
    value.owner = accountFromString(ZERO_ADDRESS).id;
    value.storedOffchain = false;
    value.resolvedWithWildcard = false;
    value.save();
  }
  return value;
}

function domain(label: string, timestamp: BigInt): Domain {
  const id = domainId(label);
  let value = Domain.load(id);
  if (value == null) {
    const parent = root(timestamp);
    value = new Domain(id);
    value.parent = parent.id;
    value.subdomainCount = 0;
    value.isMigrated = false;
    value.createdAt = timestamp;
    value.owner = accountFromString(ZERO_ADDRESS).id;
    value.storedOffchain = false;
    value.resolvedWithWildcard = false;
    parent.subdomainCount = parent.subdomainCount + 1;
    parent.save();
  }
  value.name = label + ".rns";
  value.labelName = label;
  value.labelhash = changetype<Bytes>(crypto.keccak256(Bytes.fromUTF8(label)));
  value.tokenId = tokenIdFromLabel(label);
  value.save();
  let token = TokenIdToDomain.load(value.tokenId!.toString());
  if (token == null) token = new TokenIdToDomain(value.tokenId!.toString());
  token.domain = value.id;
  token.save();
  return value;
}

function eventId(event: ethereum.Event, kind: string): string {
  return event.transaction.hash.toHexString() + "-" + event.logIndex.toString() + "-" + kind;
}

function blockNumber(event: ethereum.Event): i32 {
  return event.block.number.toI32();
}

function clearPrimaryIfMatches(ownerId: string, id: string): void {
  const primary = PrimaryNameRecord.load(ownerId);
  if (primary != null && primary.domain_id == id) {
    store.remove("PrimaryNameRecord", ownerId);
  }
}

export function handleTransfer(event: TransferEvent): void {
  const key = event.params.tokenId.toString();
  let token = TokenIdToDomain.load(key);
  if (token == null) {
    // Mint Transfer precedes NameRegistered. Read-only labelOf gives its canonical label.
    // On burn, labelOf reverts, but a previously indexed token map already exists.
    const result = RNSRegistryV2.bind(event.address).try_labelOf(event.params.tokenId);
    if (result.reverted) return;
    domain(result.value, event.block.timestamp);
    token = TokenIdToDomain.load(key);
  }
  if (token == null) return;
  const value = Domain.load(token.domain);
  if (value == null) return;
  const next = account(event.params.to);
  if (!isZero(event.params.from)) clearPrimaryIfMatches(account(event.params.from).id, value.id);
  value.owner = next.id;
  value.registrant = isZero(event.params.to) ? null : next.id;
  // _update resets the resolver on every transfer/burn. Its matching event follows Transfer.
  if (!isZero(event.params.from)) value.resolvedAddress = isZero(event.params.to) ? null : next.id;
  value.save();
  const history = new Transfer(eventId(event, "transfer"));
  history.domain = value.id;
  history.blockNumber = blockNumber(event);
  history.transactionID = event.transaction.hash;
  history.owner = next.id;
  history.save();
}

export function handleNameRegistered(event: NameRegisteredEvent): void {
  const value = domain(event.params.label, event.block.timestamp);
  value.createdAt = event.block.timestamp;
  value.expiryDate = event.params.expiresAt;
  value.graceEndsAt = event.params.expiresAt.plus(GRACE_SECONDS);
  value.finalizedAt = null;
  value.save();
  let registration = Registration.load(value.id);
  if (registration == null) registration = new Registration(value.id);
  registration.domain = value.id;
  registration.registrationDate = event.block.timestamp;
  registration.expiryDate = event.params.expiresAt;
  registration.cost = event.params.pricePaid;
  registration.registrant = account(event.params.owner).id;
  registration.labelName = event.params.label;
  registration.freeClaimed = false;
  registration.save();
  const history = new NameRegistered(eventId(event, "registered"));
  history.registration = registration.id;
  history.blockNumber = blockNumber(event);
  history.transactionID = event.transaction.hash;
  history.registrant = registration.registrant;
  history.expiryDate = event.params.expiresAt;
  history.save();
}

export function handleNameRenewed(event: NameRenewedEvent): void {
  const value = domain(event.params.label, event.block.timestamp);
  value.expiryDate = event.params.expiresAt;
  value.graceEndsAt = event.params.expiresAt.plus(GRACE_SECONDS);
  value.save();
  const registration = Registration.load(value.id);
  if (registration == null) return;
  registration.expiryDate = event.params.expiresAt;
  registration.save();
  const history = new NameRenewed(eventId(event, "renewed"));
  history.registration = registration.id;
  history.blockNumber = blockNumber(event);
  history.transactionID = event.transaction.hash;
  history.expiryDate = event.params.expiresAt;
  history.save();
}

export function handleNameExpiredFinalized(event: NameExpiredFinalizedEvent): void {
  const value = domain(event.params.label, event.block.timestamp);
  clearPrimaryIfMatches(account(event.params.previousOwner).id, value.id);
  value.owner = accountFromString(ZERO_ADDRESS).id;
  value.registrant = null;
  value.resolvedAddress = null;
  value.finalizedAt = event.block.timestamp;
  value.save();
  const history = new ExpiryFinalizedIndexed(eventId(event, "finalized"));
  history.domain = value.id;
  history.previousOwner = account(event.params.previousOwner).id;
  history.expiryDate = event.params.expiresAt;
  history.blockNumber = blockNumber(event);
  history.transactionID = event.transaction.hash;
  history.save();
}

export function handleNameTransferred(event: NameTransferredEvent): void {
  const value = domain(event.params.label, event.block.timestamp);
  const registration = Registration.load(value.id);
  if (registration == null) return;
  // ERC-721 Transfer, not this convenience event, remains authoritative for Domain.owner.
  registration.registrant = account(event.params.newOwner).id;
  registration.save();
  const history = new NameTransferred(eventId(event, "name-transferred"));
  history.registration = registration.id;
  history.blockNumber = blockNumber(event);
  history.transactionID = event.transaction.hash;
  history.newOwner = registration.registrant;
  history.save();
}

export function handleResolvedAddressUpdated(event: ResolvedAddressUpdatedEvent): void {
  const value = domain(event.params.label, event.block.timestamp);
  const id = REGISTRY_ADDRESS + "-" + value.id;
  let resolver = Resolver.load(id);
  if (resolver == null) {
    resolver = new Resolver(id);
    resolver.domain = value.id;
    resolver.address = Address.fromString(REGISTRY_ADDRESS);
    resolver.texts = [];
    resolver.coinTypes = [];
  }
  const resolved = account(event.params.resolvedAddress);
  resolver.addr = isZero(event.params.resolvedAddress) ? null : resolved.id;
  resolver.save();
  value.resolver = resolver.id;
  value.resolvedAddress = isZero(event.params.resolvedAddress) ? null : resolved.id;
  value.save();
  const history = new AddrChanged(eventId(event, "addr-changed"));
  history.resolver = resolver.id;
  history.blockNumber = blockNumber(event);
  history.transactionID = event.transaction.hash;
  history.addr = resolved.id;
  history.save();
}

export function handlePrimaryNameSet(event: PrimaryNameSetEvent): void {
  const value = domain(event.params.label, event.block.timestamp);
  const owner = account(event.params.owner);
  let primary = PrimaryNameRecord.load(owner.id);
  if (primary == null) primary = new PrimaryNameRecord(owner.id);
  primary.resolved_address = owner.id;
  primary.domain_id = value.id;
  primary.domain_name = value.name;
  primary.save();
  const history = new PrimaryChangedIndexed(eventId(event, "primary-set"));
  history.domain = value.id;
  history.account = owner.id;
  history.isSet = true;
  history.blockNumber = blockNumber(event);
  history.transactionID = event.transaction.hash;
  history.save();
}

export function handlePrimaryNameCleared(event: PrimaryNameClearedEvent): void {
  const value = domain(event.params.label, event.block.timestamp);
  const owner = account(event.params.owner);
  clearPrimaryIfMatches(owner.id, value.id);
  const history = new PrimaryChangedIndexed(eventId(event, "primary-cleared"));
  history.domain = value.id;
  history.account = owner.id;
  history.isSet = false;
  history.blockNumber = blockNumber(event);
  history.transactionID = event.transaction.hash;
  history.save();
}

export function handleFreeNameClaimed(event: FreeNameClaimedEvent): void {
  const value = domain(event.params.label, event.block.timestamp);
  const registration = Registration.load(value.id);
  if (registration != null) {
    registration.freeClaimed = true;
    registration.save();
  }
  const history = new FreeClaimIndexed(eventId(event, "free-claim"));
  history.domain = value.id;
  history.claimant = account(event.params.claimant).id;
  history.freeClaimsUsed = event.params.freeClaimsUsed;
  history.blockNumber = blockNumber(event);
  history.transactionID = event.transaction.hash;
  history.save();
}
