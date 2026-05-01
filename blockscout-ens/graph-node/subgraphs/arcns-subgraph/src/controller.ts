import { BigInt, Bytes, crypto, ByteArray } from "@graphprotocol/graph-ts";
import {
  NameRegistered as NameRegisteredEvent,
  NameRenewed as NameRenewedEvent,
} from "../generated/ArcController/Controller";
import {
  Domain,
  Registration,
  Account,
  NewOwner,
  NameRegistered as NameRegisteredEntity,
  NameRenewed as NameRenewedEntity,
} from "../generated/schema";
import {
  namehash,
  getOrCreateAccount,
  getOrCreateTldDomain,
} from "./utils";

// ─── Registration handler ─────────────────────────────────────────────────────

function handleRegistration(event: NameRegisteredEvent, tld: string): void {
  let labelName = event.params.name;
  if (labelName.length == 0) return;

  let fullName = labelName + "." + tld;
  let nodeBytes = namehash(fullName);
  let nodeHex = nodeBytes.toHexString();

  let labelhash = Bytes.fromByteArray(
    crypto.keccak256(ByteArray.fromUTF8(labelName))
  );
  let labelhashHex = labelhash.toHexString();

  // Ensure TLD parent domain exists
  let tldDomain = getOrCreateTldDomain(tld);

  // Registrant account
  let registrantAccount = getOrCreateAccount(event.params.owner);

  // Create or update Domain
  let domain = Domain.load(nodeHex);
  let isNew = domain == null;
  if (!domain) {
    domain = new Domain(nodeHex);
    domain.createdAt = event.block.timestamp;
    domain.subdomainCount = 0;
    domain.storedOffchain = false;
    domain.resolvedWithWildcard = false;
  }
  domain.name = fullName;
  domain.labelName = labelName;
  domain.labelhash = labelhash;
  domain.parent = tldDomain.id;
  domain.owner = registrantAccount.id;
  domain.registrant = registrantAccount.id;
  domain.expiryDate = event.params.expires;
  domain.isMigrated = true;
  // tokenId = uint256(labelhash) — store as BigInt
  domain.tokenId = BigInt.fromByteArray(
    ByteArray.fromHexString(labelhashHex)
  );
  domain.save();

  // Increment parent subdomainCount on new registration
  if (isNew) {
    tldDomain.subdomainCount = tldDomain.subdomainCount + 1;
    tldDomain.save();
  }

  // Create or update Registration (keyed by labelhash hex — ENS convention)
  let reg = Registration.load(labelhashHex);
  if (!reg) {
    reg = new Registration(labelhashHex);
    reg.domain = nodeHex;
    reg.registrationDate = event.block.timestamp;
  }
  reg.expiryDate = event.params.expires;
  reg.cost = event.params.cost;
  reg.registrant = registrantAccount.id;
  reg.labelName = labelName;
  reg.save();

  // NewOwner domain event
  let newOwnerId =
    event.transaction.hash.toHexString() +
    "-" +
    event.logIndex.toString() +
    "-newowner";
  let newOwnerEvent = new NewOwner(newOwnerId);
  newOwnerEvent.parentDomain = tldDomain.id;
  newOwnerEvent.domain = nodeHex;
  newOwnerEvent.owner = registrantAccount.id;
  newOwnerEvent.blockNumber = event.block.number.toI32();
  newOwnerEvent.transactionID = event.transaction.hash;
  newOwnerEvent.save();

  // NameRegistered registration event
  let nameRegId =
    event.transaction.hash.toHexString() +
    "-" +
    event.logIndex.toString() +
    "-namereg";
  let nameRegEvent = new NameRegisteredEntity(nameRegId);
  nameRegEvent.registration = labelhashHex;
  nameRegEvent.registrant = registrantAccount.id;
  nameRegEvent.expiryDate = event.params.expires;
  nameRegEvent.blockNumber = event.block.number.toI32();
  nameRegEvent.transactionID = event.transaction.hash;
  nameRegEvent.save();
}

// ─── Renewal handler ──────────────────────────────────────────────────────────

function handleRenewal(event: NameRenewedEvent, tld: string): void {
  let labelName = event.params.name;
  if (labelName.length == 0) return;

  let fullName = labelName + "." + tld;
  let nodeBytes = namehash(fullName);
  let nodeHex = nodeBytes.toHexString();

  let labelhash = Bytes.fromByteArray(
    crypto.keccak256(ByteArray.fromUTF8(labelName))
  );
  let labelhashHex = labelhash.toHexString();

  // Update Domain expiry
  let domain = Domain.load(nodeHex);
  if (domain) {
    domain.expiryDate = event.params.expires;
    domain.save();
  }

  // Update Registration expiry
  let reg = Registration.load(labelhashHex);
  if (reg) {
    reg.expiryDate = event.params.expires;
    reg.cost = event.params.cost;
    reg.save();
  }

  // NameRenewed registration event
  let nameRenewId =
    event.transaction.hash.toHexString() +
    "-" +
    event.logIndex.toString() +
    "-namerenew";
  let nameRenewEvent = new NameRenewedEntity(nameRenewId);
  nameRenewEvent.registration = labelhashHex;
  nameRenewEvent.expiryDate = event.params.expires;
  nameRenewEvent.blockNumber = event.block.number.toI32();
  nameRenewEvent.transactionID = event.transaction.hash;
  nameRenewEvent.save();
}

// ─── Arc handlers ─────────────────────────────────────────────────────────────

export function handleArcNameRegistered(event: NameRegisteredEvent): void {
  handleRegistration(event, "arc");
}

export function handleArcNameRenewed(event: NameRenewedEvent): void {
  handleRenewal(event, "arc");
}

// ─── Circle handlers ──────────────────────────────────────────────────────────

export function handleCircleNameRegistered(event: NameRegisteredEvent): void {
  handleRegistration(event, "circle");
}

export function handleCircleNameRenewed(event: NameRenewedEvent): void {
  handleRenewal(event, "circle");
}
