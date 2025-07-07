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
import { createDomain } from "./Registry";

var rootNode: ByteArray = byteArrayFromHex(BASE_NODE_HASH);

export function handleNameRegisteredByController(
  event: ControllerNameRegisteredEvent
): void {
  setNamePreimage(event.params.name, event.params.label, event.params.baseCost, event.block.timestamp);
}

export function handleNameRenewedByController(
  event: ControllerNameRenewedEvent
): void {
  setNamePreimage(event.params.name, event.params.label, event.params.cost, event.block.timestamp);
}

function setNamePreimage(name: string, label: Bytes, cost: BigInt, timestamp: BigInt): void {
  if (!checkValidLabel(name)) {
    return;
  }

  let domainId = crypto.keccak256(concat(rootNode, label)).toHex();
  let domain = Domain.load(domainId);
  if (domain == null) {
    domain = createDomain(domainId, timestamp);
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
