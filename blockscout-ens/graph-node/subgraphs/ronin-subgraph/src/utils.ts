import { BigInt, ByteArray, Bytes, crypto, ethereum } from "@graphprotocol/graph-ts";

import { Account, Domain } from "./types/schema";

export const ROOT_NODE =
  "0x0000000000000000000000000000000000000000000000000000000000000000";
// The zero address, built rather than spelled out.
export const EMPTY_ADDRESS = "0x".padEnd(42, "0");

// namehash("reverse") and namehash("addr.reverse"). RNS uses the ENS values
// (rns-contracts src/libraries/LibRNSDomain.sol, ADDR_REVERSE_ID).
export const REVERSE_NODE =
  "0xa097f6721ce401e757d1223a763fef49b8b5f90bb18567ddb86fd205dff71d34";
export const ADDR_REVERSE_NODE =
  "0x91d1777781884d03a6757a803996e38de2a42967fb37eeaca72729271025a9e2";
export const ADDR_REVERSE_SUFFIX = ".addr.reverse";

// Bits of RNSUnified's ModifyingIndicator, in the order of the ModifyingField
// enum (rns-contracts src/libraries/LibModifyingField.sol): Depth, ParentId,
// Label, Resolver, Owner, Expiry, Protected. Only mint() sets the first three.
export const IMMUTABLE_FIELDS: i32 = 0x07;
export const RESOLVER_FIELD: i32 = 0x08;
export const EXPIRY_FIELD: i32 = 0x20;

export function hasField(indicator: BigInt, field: i32): boolean {
  return !indicator.bitAnd(BigInt.fromI32(field)).isZero();
}

// RNS token ids are uint256 namehashes; domain ids are the 32-byte hex form
// BENS computes for a name.
export function nodeId(node: BigInt): string {
  return "0x" + node.toHex().slice(2).padStart(64, "0");
}

export function createEventID(event: ethereum.Event): string {
  return event.block.number
    .toString()
    .concat("-")
    .concat(event.logIndex.toString());
}

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

export function labelhash(label: string): Bytes {
  return Bytes.fromByteArray(crypto.keccak256(ByteArray.fromUTF8(label)));
}

// ENS namehash, which RNS uses unchanged (LibRNSDomain.namehash). Like the
// contract it does not normalise: callers pass names as stored on chain.
export function namehash(name: string): string {
  let node = ByteArray.fromHexString(ROOT_NODE);
  if (name.length > 0) {
    let labels = name.split(".");
    for (let i = labels.length - 1; i >= 0; i--) {
      node = crypto.keccak256(
        concat(node, crypto.keccak256(ByteArray.fromUTF8(labels[i])))
      );
    }
  }
  return node.toHexString();
}

// The reverse node RNSReverseRegistrar claims for an address: the label is
// the address in lowercase hex without 0x (LibString.toString(address)).
export function reverseNodeOf(address: string): string {
  return namehash(address.slice(2).toLowerCase() + ADDR_REVERSE_SUFFIX);
}

// "<40 hex chars>.addr.reverse" -> "0x<40 hex chars>"; null for other names.
export function addressOfReverseName(name: string | null): string | null {
  if (name === null) return null;
  let value = name;
  if (
    value.length != 40 + ADDR_REVERSE_SUFFIX.length ||
    !value.endsWith(ADDR_REVERSE_SUFFIX)
  ) {
    return null;
  }
  return "0x" + value.slice(0, 40);
}

// Postgres rejects NUL in text columns, so a string carrying one would halt
// indexing; a dot would make the stored name ambiguous.
export function isValidLabel(label: string): boolean {
  return (
    label.length > 0 && label.indexOf("\u0000") == -1 && label.indexOf(".") == -1
  );
}

export function isUsableName(name: string | null): boolean {
  if (name === null) return false;
  return name.length > 0 && name.indexOf("\u0000") == -1;
}

// Names people own: everything below a top-level node except the reverse
// tree. The root, top-level nodes (ron, reverse), addr.reverse and the
// <addr>.addr.reverse nodes are bookkeeping.
export function isForwardName(domain: Domain): boolean {
  let parent = domain.parent;
  if (parent === null) return false;
  return (
    parent != ROOT_NODE && parent != REVERSE_NODE && parent != ADDR_REVERSE_NODE
  );
}

export function isReverseNode(domain: Domain): boolean {
  let parent = domain.parent;
  return parent !== null && parent == ADDR_REVERSE_NODE;
}

export function createOrLoadAccount(address: string): Account {
  let account = Account.load(address);
  if (account == null) {
    account = new Account(address);
    account.save();
  }
  return account;
}
