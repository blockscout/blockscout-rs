import { BigInt, ByteArray, Bytes, crypto, ens } from "@graphprotocol/graph-ts";

import {
    NameRegistered as ControllerNameRegisteredEvent,
    NameRenewed as ControllerNameRenewedEvent,
  } from "../generated/{{ cookiecutter.controller_name }}/{{ cookiecutter.controller_name }}";
  
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
    setNamePreimage(
      event.params.name,
      event.params.label,
      event.params.baseCost.plus(event.params.premium)
    );
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

  let domain = Domain.load(crypto.keccak256(concat(rootNode, label)).toHex())!;
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