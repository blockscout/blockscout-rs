import { Address, BigInt, Bytes, ethereum } from "@graphprotocol/graph-ts";
import { newMockEvent } from "matchstick-as/assembly/index";

import {
  AddrChanged,
  NameRegistered,
  PrimaryNameSet,
  SubdomainRegistered,
  Transfer,
} from "../generated/NameNFT/NameNFT";
import { tokenIdToDomainID } from "../src/utils";

export function nameRegisteredEvent(
  tokenId: BigInt,
  label: string,
  owner: Address,
  expiresAt: BigInt
): NameRegistered {
  let event = changetype<NameRegistered>(newMockEvent());
  event.parameters = [
    new ethereum.EventParam(
      "tokenId",
      ethereum.Value.fromUnsignedBigInt(tokenId)
    ),
    new ethereum.EventParam("label", ethereum.Value.fromString(label)),
    new ethereum.EventParam("owner", ethereum.Value.fromAddress(owner)),
    new ethereum.EventParam(
      "expiresAt",
      ethereum.Value.fromUnsignedBigInt(expiresAt)
    ),
  ];
  return event;
}

export function subdomainRegisteredEvent(
  tokenId: BigInt,
  parentId: BigInt,
  label: string
): SubdomainRegistered {
  let event = changetype<SubdomainRegistered>(newMockEvent());
  event.parameters = [
    new ethereum.EventParam(
      "tokenId",
      ethereum.Value.fromUnsignedBigInt(tokenId)
    ),
    new ethereum.EventParam(
      "parentId",
      ethereum.Value.fromUnsignedBigInt(parentId)
    ),
    new ethereum.EventParam("label", ethereum.Value.fromString(label)),
  ];
  return event;
}

export function transferEvent(
  from: Address,
  to: Address,
  tokenId: BigInt
): Transfer {
  let event = changetype<Transfer>(newMockEvent());
  event.parameters = [
    new ethereum.EventParam("from", ethereum.Value.fromAddress(from)),
    new ethereum.EventParam("to", ethereum.Value.fromAddress(to)),
    new ethereum.EventParam(
      "id",
      ethereum.Value.fromUnsignedBigInt(tokenId)
    ),
  ];
  return event;
}

export function addrChangedEvent(tokenId: BigInt, addr: Address): AddrChanged {
  let event = changetype<AddrChanged>(newMockEvent());
  event.parameters = [
    new ethereum.EventParam(
      "node",
      ethereum.Value.fromFixedBytes(
        Bytes.fromHexString(tokenIdToDomainID(tokenId))
      )
    ),
    new ethereum.EventParam("addr", ethereum.Value.fromAddress(addr)),
  ];
  return event;
}

export function primaryNameSetEvent(
  addr: Address,
  tokenId: BigInt
): PrimaryNameSet {
  let event = changetype<PrimaryNameSet>(newMockEvent());
  event.parameters = [
    new ethereum.EventParam("addr", ethereum.Value.fromAddress(addr)),
    new ethereum.EventParam(
      "tokenId",
      ethereum.Value.fromUnsignedBigInt(tokenId)
    ),
  ];
  return event;
}
