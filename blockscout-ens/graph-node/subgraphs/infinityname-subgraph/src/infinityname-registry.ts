import {
  DomainRegistered as DomainRegisteredEvent,
  PrimaryDomainSet as PrimaryDomainSetEvent,
  PrimaryDomainReset as PrimaryDomainResetEvent,
  Transfer as TransferEvent,
  TokenSeized as TokenSeizedEvent,
} from "../generated/InfinityNameUpgradeable/InfinityNameUpgradeable";
import {
  Domain,
  Account,
  Transfer,
  DomainRegistered,
  PrimaryDomainSet,
  PrimaryDomainReset,
  TokenSeized,
  Registration,
  NameRegistered,
  TokenIdToDomain,
  PrimaryNameRecord,
} from "../generated/schema";
import { BigInt, Bytes, crypto, ethereum } from "@graphprotocol/graph-ts";
import { hashByDomainName, getLabelName, isValidDomain } from "./utils";

const EMPTY_ADDRESS = "0x0000000000000000000000000000000000000000";

function createOrLoadAccount(address: string): Account {
  let account = Account.load(address);
  if (account == null) {
    account = new Account(address);
    account.save();
  }
  return account;
}

function createOrLoadDomain(domainHash: string, domainName: string, timestamp: BigInt): Domain {
  let domain = Domain.load(domainHash);
  if (domain == null) {
    domain = new Domain(domainHash);
    domain.name = domainName;
    domain.createdAt = timestamp;
    domain.subdomainCount = 0;
    domain.isMigrated = true;
    domain.storedOffchain = false;
    domain.resolvedWithWildcard = false;
    domain.isPrimary = false;
    domain.labelName = getLabelName(domainName);
    domain.labelhash = Bytes.fromByteArray(
      crypto.keccak256(Bytes.fromUTF8(domain.labelName!))
    );
  }
  return domain;
}

function createEventID(event: ethereum.Event): string {
  return event.block.number
    .toString()
    .concat("-")
    .concat(event.transaction.index.toString())
    .concat("-")
    .concat(event.logIndex.toString());
}

function getDomainByTokenId(tokenId: BigInt): Domain | null {
  let tokenMapping = TokenIdToDomain.load(tokenId.toString());
  if (tokenMapping == null) {
    return null;
  }
  return Domain.load(tokenMapping.domain);
}

export function handleDomainRegistered(event: DomainRegisteredEvent): void {
  let domainName = event.params.domain;
  let tokenId = event.params.tokenId;
  let owner = event.params.owner;

  // Create domain hash using keccak256(domain + suffix)
  // The domain parameter already includes the suffix (e.g., "example.blue")
  let domainHash = hashByDomainName(domainName).toHex();

  // Create or load domain
  let domain = createOrLoadDomain(domainHash, domainName, event.block.timestamp);
  
  // Create or load owner account
  let ownerAccount = createOrLoadAccount(owner.toHexString());
  
  // Update domain properties
  domain.owner = ownerAccount.id;
  domain.registrant = ownerAccount.id;
  domain.resolvedAddress = ownerAccount.id; // Owner is the resolved address
  domain.tokenId = tokenId;
  domain.isPrimary = false; // Will be set when PrimaryDomainSet is called
  domain.save();

  // Create DomainRegistered event
  let domainRegisteredEventId = createEventID(event);
  let domainRegisteredEvent = new DomainRegistered(domainRegisteredEventId);
  domainRegisteredEvent.domain = domain.id;
  domainRegisteredEvent.blockNumber = event.block.number.toI32();
  domainRegisteredEvent.transactionID = event.transaction.hash;
  domainRegisteredEvent.owner = ownerAccount.id;
  domainRegisteredEvent.domainName = domainName;
  domainRegisteredEvent.tokenId = tokenId;
  domainRegisteredEvent.save();

  // Create Registration entity
  let registration = new Registration(tokenId.toString());
  registration.domain = domain.id;
  registration.registrant = ownerAccount.id;
  registration.registrationDate = event.block.timestamp;
  registration.cost = event.transaction.value;
  registration.labelName = getLabelName(domainName);
  registration.save();

  // Create NameRegistered event
  let nameRegisteredEventId = event.transaction.hash
    .toHexString()
    .concat("-")
    .concat(event.logIndex.toString())
    .concat("-registered");
  let nameRegisteredEvent = new NameRegistered(nameRegisteredEventId);
  nameRegisteredEvent.registration = registration.id;
  nameRegisteredEvent.blockNumber = event.block.number.toI32();
  nameRegisteredEvent.transactionID = event.transaction.hash;
  nameRegisteredEvent.registrant = ownerAccount.id;
  nameRegisteredEvent.save();

  // Create tokenId -> domain mapping
  let tokenMapping = new TokenIdToDomain(tokenId.toString());
  tokenMapping.domain = domain.id;
  tokenMapping.save();
}

export function handlePrimaryDomainSet(event: PrimaryDomainSetEvent): void {
  let owner = event.params.owner;
  let tokenId = event.params.tokenId;
  let domainName = event.params.domain;

  // Create domain hash
  let domainHash = hashByDomainName(domainName).toHex();

  // Load domain (should exist from DomainRegistered)
  let domain = Domain.load(domainHash);
  if (domain == null) {
    // Domain might not exist if this is called before registration
    // Create it anyway
    domain = createOrLoadDomain(domainHash, domainName, event.block.timestamp);
  }

  // Create or load owner account
  let ownerAccount = createOrLoadAccount(owner.toHexString());

  // Update domain to be primary
  domain.isPrimary = true;
  // The resolved address for a domain is always its owner
  domain.resolvedAddress = ownerAccount.id;
  domain.save();

  // Update account's primary domain
  // When primaryDomain is set, that domain becomes the resolved address for the account
  ownerAccount.primaryDomain = domain.id;
  ownerAccount.save();

  // Update PrimaryNameRecord table - write primary name to PrimaryNameRecord
  let resolvedAddress = owner.toHexString().toLowerCase();
  let primaryNameRecordId = resolvedAddress;
  let primaryNameRecord = PrimaryNameRecord.load(primaryNameRecordId);
  if (primaryNameRecord == null) {
    primaryNameRecord = new PrimaryNameRecord(primaryNameRecordId);
  }
  primaryNameRecord.resolved_address = resolvedAddress;
  primaryNameRecord.domain_id = domain.id;
  primaryNameRecord.domain_name = domainName;
  primaryNameRecord.save();

  // Create PrimaryDomainSet event
  let primaryDomainSetEventId = createEventID(event);
  let primaryDomainSetEvent = new PrimaryDomainSet(primaryDomainSetEventId);
  primaryDomainSetEvent.domain = domain.id;
  primaryDomainSetEvent.blockNumber = event.block.number.toI32();
  primaryDomainSetEvent.transactionID = event.transaction.hash;
  primaryDomainSetEvent.owner = ownerAccount.id;
  primaryDomainSetEvent.tokenId = tokenId;
  primaryDomainSetEvent.domainName = domainName;
  primaryDomainSetEvent.save();
}

export function handlePrimaryDomainReset(event: PrimaryDomainResetEvent): void {
  let owner = event.params.owner;
  let tokenId = event.params.tokenId;

  // Create or load owner account
  let ownerAccount = createOrLoadAccount(owner.toHexString());

  // Find domain by tokenId
  let domain = getDomainByTokenId(tokenId);
  if (domain == null) {
    // Domain not found - skip
    return;
  }

  // Update domain to not be primary
  domain.isPrimary = false;
  // The resolved address for a domain is always its owner
  domain.resolvedAddress = domain.owner;
  domain.save();

  // Update account's primary domain to null
  ownerAccount.primaryDomain = null;
  ownerAccount.save();

  // Update PrimaryNameRecord table - clear the primary name record
  let resolvedAddress = owner.toHexString().toLowerCase();
  let primaryNameRecordId = resolvedAddress;
  let primaryNameRecord = PrimaryNameRecord.load(primaryNameRecordId);
  if (primaryNameRecord != null) {
    primaryNameRecord.resolved_address = resolvedAddress;
    primaryNameRecord.domain_id = null;
    primaryNameRecord.domain_name = null;
    primaryNameRecord.save();
  }

  // Create PrimaryDomainReset event
  let primaryDomainResetEventId = createEventID(event);
  let primaryDomainResetEvent = new PrimaryDomainReset(primaryDomainResetEventId);
  primaryDomainResetEvent.domain = domain.id;
  primaryDomainResetEvent.blockNumber = event.block.number.toI32();
  primaryDomainResetEvent.transactionID = event.transaction.hash;
  primaryDomainResetEvent.owner = ownerAccount.id;
  primaryDomainResetEvent.tokenId = tokenId;
  primaryDomainResetEvent.save();
}

export function handleTransfer(event: TransferEvent): void {
  let from = event.params.from;
  let to = event.params.to;
  let tokenId = event.params.tokenId;

  // Create or load accounts
  let fromAccount = createOrLoadAccount(from.toHexString());
  let toAccount = createOrLoadAccount(to.toHexString());

  // Find domain by tokenId
  let domain = getDomainByTokenId(tokenId);
  if (domain == null) {
    // Domain not found - might be a mint (from == zero address)
    // In that case, we'll skip since DomainRegistered should handle it
    // But if it's a regular transfer, domain should exist
    // For now, skip if domain not found
    return;
  }

  // Update domain owner
  domain.owner = toAccount.id;
  
  // The resolved address for a domain is always its owner
  domain.resolvedAddress = toAccount.id;
  
  // If this was the primary domain for the old owner, reset it
  if (domain.isPrimary && from.toHexString() != EMPTY_ADDRESS) {
    domain.isPrimary = false;
    fromAccount.primaryDomain = null;
    fromAccount.save();
    
    // Clear PrimaryNameRecord for the old owner
    let oldOwnerResolvedAddress = from.toHexString().toLowerCase();
    let oldOwnerPrimaryNameRecordId = oldOwnerResolvedAddress;
    let oldOwnerPrimaryNameRecord = PrimaryNameRecord.load(oldOwnerPrimaryNameRecordId);
    if (oldOwnerPrimaryNameRecord != null) {
      oldOwnerPrimaryNameRecord.resolved_address = oldOwnerResolvedAddress;
      oldOwnerPrimaryNameRecord.domain_id = null;
      oldOwnerPrimaryNameRecord.domain_name = null;
      oldOwnerPrimaryNameRecord.save();
    }
  }
  
  domain.save();

  // Create Transfer event
  let transferEventId = createEventID(event);
  let transferEvent = new Transfer(transferEventId);
  transferEvent.domain = domain.id;
  transferEvent.blockNumber = event.block.number.toI32();
  transferEvent.transactionID = event.transaction.hash;
  transferEvent.from = fromAccount.id;
  transferEvent.to = toAccount.id;
  transferEvent.owner = toAccount.id;
  transferEvent.save();

  toAccount.save();
  fromAccount.save();
}

export function handleTokenSeized(event: TokenSeizedEvent): void {
  let from = event.params.from;
  let to = event.params.to;
  let tokenId = event.params.tokenId;

  // Create or load accounts
  let fromAccount = createOrLoadAccount(from.toHexString());
  let toAccount = createOrLoadAccount(to.toHexString());

  // Find domain by tokenId
  let domain = getDomainByTokenId(tokenId);
  if (domain == null) {
    // Domain not found - skip
    return;
  }

  // Update domain owner
  domain.owner = toAccount.id;
  
  // The resolved address for a domain is always its owner
  domain.resolvedAddress = toAccount.id;
  
  // If this was the primary domain for the old owner, reset it
  if (domain.isPrimary) {
    domain.isPrimary = false;
    fromAccount.primaryDomain = null;
    fromAccount.save();
    
    // Clear PrimaryNameRecord for the old owner
    let oldOwnerResolvedAddress = from.toHexString().toLowerCase();
    let oldOwnerPrimaryNameRecordId = oldOwnerResolvedAddress;
    let oldOwnerPrimaryNameRecord = PrimaryNameRecord.load(oldOwnerPrimaryNameRecordId);
    if (oldOwnerPrimaryNameRecord != null) {
      oldOwnerPrimaryNameRecord.resolved_address = oldOwnerResolvedAddress;
      oldOwnerPrimaryNameRecord.domain_id = null;
      oldOwnerPrimaryNameRecord.domain_name = null;
      oldOwnerPrimaryNameRecord.save();
    }
  }
  
  domain.save();

  // Create TokenSeized event
  let tokenSeizedEventId = createEventID(event);
  let tokenSeizedEvent = new TokenSeized(tokenSeizedEventId);
  tokenSeizedEvent.domain = domain.id;
  tokenSeizedEvent.blockNumber = event.block.number.toI32();
  tokenSeizedEvent.transactionID = event.transaction.hash;
  tokenSeizedEvent.from = fromAccount.id;
  tokenSeizedEvent.to = toAccount.id;
  tokenSeizedEvent.tokenId = tokenId;
  tokenSeizedEvent.save();

  // Update accounts
  fromAccount.save();
  toAccount.save();
}

