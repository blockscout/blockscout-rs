import { BigInt, ByteArray, Bytes, crypto, ens } from "@graphprotocol/graph-ts";

import {
  NameRegistered as ControllerNameRegisteredEvent,
  NameRenewed as ControllerNameRenewedEvent,
} from "../generated/RegistrarController/RegistrarController";

import {
  checkValidLabel,
  concat,
  byteArrayFromHex,
  BASE_NODE_HASH,
  BASE_NODE,
} from "./utils";

// Import entity types generated from the GraphQL schema
import {
  Account,
  Domain,
  NameRegistered,
  NameRenewed,
  NameTransferred,
  Registration,
} from "../generated/schema";

var rootNode: ByteArray = byteArrayFromHex(BASE_NODE_HASH);

export function handleNameRegisteredByController(
  event: ControllerNameRegisteredEvent
): void {
  let account = new Account(event.params.owner.toHex());
  account.save();

  let domainId = crypto.keccak256(concat(rootNode, event.params.label)).toHex();
  let domain = Domain.load(domainId);
  if (domain == null) {
    domain = new Domain(domainId);
    domain.createdAt = event.block.timestamp;
    domain.subdomainCount = 0;
    domain.storedOffchain = false;
    domain.resolvedWithWildcard = false;
    domain.owner = account.id;
    domain.registrant = account.id;
    domain.isMigrated = true;
  }

  if (checkValidLabel(event.params.name)) {
    domain.labelName = event.params.name;
    domain.labelhash = event.params.label;
    domain.name = event.params.name + BASE_NODE;
  }
  domain.save();

  let registration = Registration.load(event.params.label.toHex());
  if (registration == null) {
    registration = new Registration(event.params.label.toHex());
    registration.domain = domain.id;
    registration.registrationDate = event.block.timestamp;
    registration.registrant = account.id;
  }
  if (checkValidLabel(event.params.name)) {
    registration.labelName = event.params.name;
  }
  registration.cost = event.params.baseCost;
  registration.expiryDate = event.params.expires;
  registration.save();
}

export function handleNameRenewedByController(
  event: ControllerNameRenewedEvent
): void {
  setNamePreimage(event.params.name, event.params.label, event.params.cost);
}

function setNamePreimage(name: string, label: Bytes, cost: BigInt): void {
  if (!checkValidLabel(name)) {
    return;
  }

  let domainId = crypto.keccak256(concat(rootNode, label)).toHex();
  let domain = Domain.load(domainId);
  if (domain == null) {
    return;
  }
  if (domain.labelName !== name) {
    domain.labelName = name;
    domain.name = name + BASE_NODE;
    domain.save();
  }

  let registration = Registration.load(label.toHex());
  if (registration == null) return;
  registration.labelName = name;
  registration.cost = cost;
  registration.save();
}
