// Import types and APIs from graph-ts
import { BigInt, ByteArray, crypto, ens } from "@graphprotocol/graph-ts";

import {
  byteArrayFromHex,
  checkValidLabel,
  concat,
  createEventID,
  BASE_NODE,
  uint256ToByteArray,
  BASE_NODE_HASH,
} from "./utils";

// Import event types from the registry contract ABI
import {
  NameRegistered as NameRegisteredEvent,
  NameRenewed as NameRenewedEvent,
  Transfer as TransferEvent,
} from "../generated/Base/Base";

// Import entity types generated from the GraphQL schema
import {
  Account,
  Domain,
  NameRegistered,
  NameRenewed,
  NameTransferred,
  Registration,
} from "../generated/schema";

const GRACE_PERIOD_SECONDS = BigInt.fromI32(7776000); // 90 days

var rootNode: ByteArray = byteArrayFromHex(BASE_NODE_HASH);

/**
 * Handles NameRegistered registrar events to index domain registrations.
 * @param event The NameRegistered blockchain event.
 */
export function handleNameRegistered(event: NameRegisteredEvent): void {
  let account = new Account(event.params.owner.toHex());
  account.save();

  let label = uint256ToByteArray(event.params.id);
  let registration = new Registration(label.toHex());

  let domainId = crypto.keccak256(concat(rootNode, label)).toHex();
  let domain = Domain.load(domainId);

  if (domain == null) {
    domain = new Domain(domainId);
    domain.createdAt = event.block.timestamp;
    domain.subdomainCount = 0;
    domain.storedOffchain = false;
    domain.resolvedWithWildcard = false;
    domain.owner = account.id;
    domain.isMigrated = true;
  }

  registration.domain = domain.id;
  registration.registrationDate = event.block.timestamp;
  registration.expiryDate = event.params.expires;
  registration.registrant = account.id;

  domain.registrant = account.id;
  domain.expiryDate = event.params.expires;

  let labelName = ens.nameByHash(label.toHexString());
  if (labelName != null) {
    domain.labelName = labelName;
    domain.name = labelName! + BASE_NODE;
  }
  domain.save();
  registration.save();

  let registrationEvent = new NameRegistered(createEventID(event));
  registrationEvent.registration = registration.id;
  registrationEvent.blockNumber = event.block.number.toI32();
  registrationEvent.transactionID = event.transaction.hash;
  registrationEvent.registrant = account.id;
  registrationEvent.expiryDate = event.params.expires;
  registrationEvent.save();
}

/**
 * Handles NameRenewed registrar events to extend registration expiry.
 * @param event The NameRenewed blockchain event.
 */
export function handleNameRenewed(event: NameRenewedEvent): void {
  let label = uint256ToByteArray(event.params.id);
  let registration = Registration.load(label.toHex());

  let domainId = crypto.keccak256(concat(rootNode, label)).toHex();
  let domain = Domain.load(domainId);

  if (registration != null) {
    registration.expiryDate = event.params.expires;
    registration.save();
  }

  if (domain != null) {
    domain.expiryDate = event.params.expires;
    domain.save();
  }

  let registrationEvent = new NameRenewed(createEventID(event));
  registrationEvent.registration = label.toHex();
  registrationEvent.blockNumber = event.block.number.toI32();
  registrationEvent.transactionID = event.transaction.hash;
  registrationEvent.expiryDate = event.params.expires;
  registrationEvent.save();
}

/**
 * Handles Transfer registrar events for token ownership changes.
 * @param event The Transfer blockchain event.
 */
export function handleNameTransferred(event: TransferEvent): void {
  let account = new Account(event.params.to.toHex());
  account.save();

  let label = uint256ToByteArray(event.params.tokenId);
  let registration = Registration.load(label.toHex());
  if (registration != null) {
    registration.registrant = account.id;
    registration.save();
  }

  let domainId = crypto.keccak256(concat(rootNode, label)).toHex();
  let domain = Domain.load(domainId);
  if (domain != null) {
    domain.registrant = account.id;
    domain.save();
  }

  let eventEntity = new NameTransferred(createEventID(event));
  eventEntity.registration = label.toHex();
  eventEntity.blockNumber = event.block.number.toI32();
  eventEntity.transactionID = event.transaction.hash;
  eventEntity.newOwner = account.id;
  eventEntity.save();
}
