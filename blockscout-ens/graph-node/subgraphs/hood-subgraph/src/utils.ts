// Import types and APIs from graph-ts
import { BigInt, ByteArray, ethereum, log } from "@graphprotocol/graph-ts";
import { Account, Domain } from "../generated/schema";

export function createEventID(event: ethereum.Event): string {
  return event.block.number
    .toString()
    .concat("-")
    .concat(event.logIndex.toString());
}

/** namehash("hood") — NOT eth */
export const ETH_NODE =
  "0x17f79377132793bf63f8c99a522a617a401dc4826aa34aa9cc11e97310c22e5d";
export const ROOT_NODE =
  "0x0000000000000000000000000000000000000000000000000000000000000000";
export const EMPTY_ADDRESS = "0x0000000000000000000000000000000000000000";
export const EMPTY_ADDRESS_BYTEARRAY = new ByteArray(20);

export function concat(a: ByteArray, b: ByteArray): ByteArray {
  let out = new Uint8Array(a.length + b.length);
  for (let i = 0; i < a.length; i++) {
    out[i] = a[i];
  }
  for (let j = 0; j < b.length; j++) {
    out[a.length + j] = b[j];
  }
  return changetype<ByteArray>(out);
}

export function byteArrayFromHex(s: string): ByteArray {
  if (s.length % 2 !== 0) {
    throw new TypeError("Hex string must have an even number of characters");
  }
  let out = new Uint8Array(s.length / 2);
  for (var i = 0; i < s.length; i += 2) {
    out[i / 2] = parseInt(s.substring(i, i + 2), 16) as u32;
  }
  return changetype<ByteArray>(out);
}

export function uint256ToByteArray(i: BigInt): ByteArray {
  let hex = i.toHex().slice(2).padStart(64, "0");
  return byteArrayFromHex(hex);
}

export function createOrLoadAccount(address: string): Account {
  let account = Account.load(address);
  if (account == null) {
    account = new Account(address);
    account.save();
  }
  return account;
}

export function createOrLoadDomain(node: string): Domain {
  let domain = Domain.load(node);
  if (domain == null) {
    domain = new Domain(node);
    domain.save();
  }
  return domain;
}

export function checkValidLabel(name: string | null): boolean {
  if (name == null) {
    return false;
  }
  let label = name as string;
  if (label.includes(".")) {
    log.warning("Invalid label '{}': has a '.'", [label]);
    return false;
  }
  for (let i = 0; i < label.length; i++) {
    let c = label.charCodeAt(i);
    if (c == 0) {
      log.warning("Invalid label '{}': null byte", [label]);
      return false;
    }
  }
  return true;
}
