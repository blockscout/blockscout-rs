import { Address, BigInt, Bytes, ethereum } from "@graphprotocol/graph-ts";
import {
  createMockedFunction,
  newMockEvent,
} from "matchstick-as/assembly/index";

import { handleRecordUpdated, handleTransfer } from "../src/rnsUnified";
import {
  NameChanged,
  VersionChanged,
} from "../src/types/PublicResolver/PublicResolver";
import {
  RecordUpdated,
  Transfer,
} from "../src/types/RNSUnified/RNSUnified";
import {
  NameRegistered,
  NameRenewed,
} from "../src/types/RONRegistrarController/RONRegistrarController";

// Test addresses are built from small numbers, never real deployments.
function hex(n: i32): string {
  let digits = "0123456789abcdef";
  let out = "";
  let value = n;
  while (value > 0) {
    out = digits.charAt(value & 15) + out;
    value = value >> 4;
  }
  return out;
}

export function addressString(n: i32): string {
  return "0x" + hex(n).padStart(40, "0");
}

export function address(n: i32): Address {
  return Address.fromString(addressString(n));
}

export const REGISTRY = 0x1001;
export const RESOLVER = 0x1002;
export const OTHER_RESOLVER = 0x1012;
export const REVERSE_REGISTRAR = 0x1003;
export const AUCTION = 0x1004;
export const CONTROLLER = 0x1005;
export const ADMIN = 0xad;
export const ALICE = 0xa11ce;
export const BOB = 0xb0b;

// namehash vectors, computed outside the subgraph code.
export const ROOT = "0x" + "0".repeat(64);
export const RON = "0xba69923fa107dbf5a25a073a10b7c9216ae39fbadc95dc891d460d9ae315d688";
export const REVERSE = "0xa097f6721ce401e757d1223a763fef49b8b5f90bb18567ddb86fd205dff71d34";
export const ADDR_REVERSE = "0x91d1777781884d03a6757a803996e38de2a42967fb37eeaca72729271025a9e2";
export const DUKE = "0x4467e296cabb66ee07d345db56cf81360336f0e6eafb97957d0c2ab9082adbd3";
export const OTHER = "0xb544dfc682af6a3111fec9fb1219ee3ace6e519a8217c8bf4e824ba77fb11835";
// namehash("<ALICE without 0x>.addr.reverse")
export const ALICE_REVERSE = "0x452734c0ca5332704485b49f7433c6bda1958449c47b77bd935e0f94c20ba2bf";
// namehash("99999.ron"): on Ronin mainnet a reserved name held by the auction.
export const RESERVED = "0x3a8f960c85622cacd7ed6be5a09af9c39232dd2911745426be1c16d8476c9054";
// namehash("a.ron") .. namehash("e.ron")
export const A = "0x0857f5e7b560d810eb91fc2c57f96199d89139d2e91d3501aa151ff41e57788e";
export const B = "0x5ed7a0650795ec5ed88d54ab216be4b5e7ec5d357e326c9988d8c5d6a755da8e";
export const C = "0xa4edc8abc6697f8e397d5e61860a8a25d0bcabd3ed6990275fdb9a9133b5a6bd";
export const D = "0x677a3e3b50500c93b9a736a5aed743156d99686a242c6e27f788d892f8720e72";
export const E = "0x6f76f4618f8c131da3937f20ef49d697b5add31f05e1094877c77dedc6b537e4";

export const EMPTY = addressString(0);
export const ALICE_LABEL = addressString(ALICE).slice(2);
export const EXPIRY = BigInt.fromI32(2000000000);
export const LATER = BigInt.fromI32(2100000000);

export const MAX_EXPIRY = BigInt.fromString("18446744073709551615");
export const ALL_FIELDS = BigInt.fromUnsignedBytes(
  Bytes.fromHexString("0x" + "ff".repeat(32))
);
export const RESOLVER_BIT = BigInt.fromI32(0x08);
export const OWNER_BIT = BigInt.fromI32(0x10);
export const EXPIRY_BIT = BigInt.fromI32(0x20);
export const USER_FIELDS = BigInt.fromI32(0x18);

export function node(id: string): BigInt {
  let bigEndian = Bytes.fromHexString(id);
  let littleEndian = new Uint8Array(bigEndian.length);
  for (let i = 0; i < bigEndian.length; i++) {
    littleEndian[i] = bigEndian[bigEndian.length - 1 - i];
  }
  return BigInt.fromUnsignedBytes(Bytes.fromUint8Array(littleEndian));
}

export function resolverId(nodeId: string, resolver: i32 = RESOLVER): string {
  return addressString(resolver) + "-" + nodeId;
}

// --- RNSUnified.getRecord ---------------------------------------------------

function recordTuple(
  depth: i32,
  parent: string,
  label: string,
  resolver: i32,
  owner: i32,
  expiry: BigInt
): ethereum.Tuple {
  let immut: Array<ethereum.Value> = [
    ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(depth)),
    ethereum.Value.fromUnsignedBigInt(node(parent)),
    ethereum.Value.fromString(label),
  ];
  let mut: Array<ethereum.Value> = [
    ethereum.Value.fromAddress(resolver == 0 ? Address.zero() : address(resolver)),
    ethereum.Value.fromAddress(owner == 0 ? Address.zero() : address(owner)),
    ethereum.Value.fromUnsignedBigInt(expiry),
    ethereum.Value.fromBoolean(false),
  ];
  let record: Array<ethereum.Value> = [
    ethereum.Value.fromTuple(changetype<ethereum.Tuple>(immut)),
    ethereum.Value.fromTuple(changetype<ethereum.Tuple>(mut)),
  ];
  return changetype<ethereum.Tuple>(record);
}

const GET_RECORD =
  "getRecord(uint256):(((uint8,uint256,string),(address,address,uint64,bool)))";

export function mockRecord(
  id: string,
  depth: i32,
  parent: string,
  label: string,
  resolver: i32,
  owner: i32,
  expiry: BigInt
): void {
  createMockedFunction(address(REGISTRY), "getRecord", GET_RECORD)
    .withArgs([ethereum.Value.fromUnsignedBigInt(node(id))])
    .returns([
      ethereum.Value.fromTuple(
        recordTuple(depth, parent, label, resolver, owner, expiry)
      ),
    ]);
}

export function mockRecordReverts(id: string): void {
  createMockedFunction(address(REGISTRY), "getRecord", GET_RECORD)
    .withArgs([ethereum.Value.fromUnsignedBigInt(node(id))])
    .reverts();
}

// The top-level nodes as they exist on chain: ron, reverse, addr.reverse.
export function mockTopLevelRecords(): void {
  mockRecord(RON, 1, ROOT, "ron", 0, ADMIN, MAX_EXPIRY);
  mockRecord(REVERSE, 1, ROOT, "reverse", 0, ADMIN, MAX_EXPIRY);
  mockRecord(ADDR_REVERSE, 2, REVERSE, "addr", 0, REVERSE_REGISTRAR, MAX_EXPIRY);
}

export function mockReverseName(id: string, name: string): void {
  createMockedFunction(address(RESOLVER), "name", "name(bytes32):(string)")
    .withArgs([ethereum.Value.fromFixedBytes(Bytes.fromHexString(id))])
    .returns([ethereum.Value.fromString(name)]);
}

export function mockGetRNSUnified(): void {
  createMockedFunction(
    address(RESOLVER),
    "getRNSUnified",
    "getRNSUnified():(address)"
  ).returns([ethereum.Value.fromAddress(address(REGISTRY))]);
}

export function mockTotalSupply(supply: i32): void {
  createMockedFunction(
    address(REGISTRY),
    "totalSupply",
    "totalSupply():(uint256)"
  ).returns([ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(supply))]);
}

export function mockTokenByIndex(index: i32, id: string): void {
  createMockedFunction(
    address(REGISTRY),
    "tokenByIndex",
    "tokenByIndex(uint256):(uint256)"
  )
    .withArgs([ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(index))])
    .returns([ethereum.Value.fromUnsignedBigInt(node(id))]);
}

// --- events -----------------------------------------------------------------

let logIndex = 0;

function nextEvent(emitter: i32): ethereum.Event {
  let event = newMockEvent();
  event.address = address(emitter);
  event.logIndex = BigInt.fromI32(logIndex);
  logIndex++;
  event.block.number = BigInt.fromI32(55577600);
  event.block.timestamp = BigInt.fromI32(1780000000);
  event.parameters = new Array();
  return event;
}

function param(name: string, value: ethereum.Value): ethereum.EventParam {
  return new ethereum.EventParam(name, value);
}

export function mintEvent(
  id: string,
  depth: i32,
  parent: string,
  label: string,
  resolver: i32,
  owner: i32,
  expiry: BigInt
): RecordUpdated {
  return recordUpdatedEvent(
    id,
    ALL_FIELDS,
    recordTuple(depth, parent, label, resolver, owner, expiry)
  );
}

// A RecordUpdated outside mint(): the immutable half is empty.
export function updateEvent(
  id: string,
  indicator: BigInt,
  resolver: i32,
  owner: i32,
  expiry: BigInt
): RecordUpdated {
  return recordUpdatedEvent(
    id,
    indicator,
    recordTuple(0, ROOT, "", resolver, owner, expiry)
  );
}

function recordUpdatedEvent(
  id: string,
  indicator: BigInt,
  record: ethereum.Tuple
): RecordUpdated {
  let event = changetype<RecordUpdated>(nextEvent(REGISTRY));
  event.parameters.push(param("node", ethereum.Value.fromUnsignedBigInt(node(id))));
  event.parameters.push(param("indicator", ethereum.Value.fromUnsignedBigInt(indicator)));
  event.parameters.push(param("record", ethereum.Value.fromTuple(record)));
  return event;
}

export function transferEvent(from: i32, to: i32, id: string): Transfer {
  let event = changetype<Transfer>(nextEvent(REGISTRY));
  event.parameters.push(
    param("from", ethereum.Value.fromAddress(from == 0 ? Address.zero() : address(from)))
  );
  event.parameters.push(
    param("to", ethereum.Value.fromAddress(to == 0 ? Address.zero() : address(to)))
  );
  event.parameters.push(param("tokenId", ethereum.Value.fromUnsignedBigInt(node(id))));
  return event;
}

export function nameChangedEvent(id: string, name: string): NameChanged {
  let event = changetype<NameChanged>(nextEvent(RESOLVER));
  event.parameters.push(
    param("node", ethereum.Value.fromFixedBytes(Bytes.fromHexString(id)))
  );
  event.parameters.push(param("name", ethereum.Value.fromString(name)));
  return event;
}

export function versionChangedEvent(id: string, version: i32): VersionChanged {
  let event = changetype<VersionChanged>(nextEvent(RESOLVER));
  event.parameters.push(
    param("node", ethereum.Value.fromFixedBytes(Bytes.fromHexString(id)))
  );
  event.parameters.push(
    param("newVersion", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(version)))
  );
  return event;
}

export function nameRegisteredEvent(
  label: string,
  id: string,
  owner: i32,
  ronPrice: i32,
  expires: BigInt
): NameRegistered {
  let event = changetype<NameRegistered>(nextEvent(CONTROLLER));
  event.parameters.push(param("name", ethereum.Value.fromString(label)));
  event.parameters.push(param("id", ethereum.Value.fromUnsignedBigInt(node(id))));
  event.parameters.push(param("owner", ethereum.Value.fromAddress(address(owner))));
  event.parameters.push(
    param("ronPrice", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(ronPrice)))
  );
  event.parameters.push(
    param("usdPrice", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(ronPrice * 2)))
  );
  event.parameters.push(param("expires", ethereum.Value.fromUnsignedBigInt(expires)));
  return event;
}

export function nameRenewedEvent(
  label: string,
  id: string,
  cost: i32,
  expires: BigInt
): NameRenewed {
  let event = changetype<NameRenewed>(nextEvent(CONTROLLER));
  event.parameters.push(param("name", ethereum.Value.fromString(label)));
  event.parameters.push(param("id", ethereum.Value.fromUnsignedBigInt(node(id))));
  event.parameters.push(
    param("cost", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(cost)))
  );
  event.parameters.push(param("expires", ethereum.Value.fromUnsignedBigInt(expires)));
  return event;
}

// A .ron name minted after the L2 genesis, as RONRegistrarController does it:
// the ERC-721 Transfer first, then RecordUpdated with every field.
export function mintName(id: string, label: string, owner: i32): void {
  mockRecord(id, 2, RON, label, RESOLVER, owner, EXPIRY);
  handleTransfer(transferEvent(0, owner, id));
  handleRecordUpdated(mintEvent(id, 2, RON, label, RESOLVER, owner, EXPIRY));
}

// RNSReverseRegistrar._claimWithResolver mints <addr>.addr.reverse to itself.
export function claimReverse(id: string, label: string): void {
  mockRecord(id, 3, ADDR_REVERSE, label, RESOLVER, REVERSE_REGISTRAR, MAX_EXPIRY);
  handleRecordUpdated(
    mintEvent(id, 3, ADDR_REVERSE, label, RESOLVER, REVERSE_REGISTRAR, MAX_EXPIRY)
  );
}

export function block(number: i32): ethereum.Block {
  let b = newMockEvent().block;
  b.number = BigInt.fromI32(number);
  b.timestamp = BigInt.fromI32(1779000000 + number);
  return b;
}
