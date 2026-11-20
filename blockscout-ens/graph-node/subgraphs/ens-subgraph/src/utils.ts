import { BigInt, ByteArray, Bytes, ethereum, log } from "@graphprotocol/graph-ts";
import { Account, Domain } from "./types/schema";

// No delimiter is needed because both values use fixed-width 32-byte encoding.
export function createEventID(event: ethereum.Event): Bytes {
  return Bytes.fromByteArray(
    concat(
      uint256ToByteArray(event.block.number),
      uint256ToByteArray(event.logIndex)
    )
  );
}

// Legacy event entities use string IDs; ENSv2 event entities use Bytes.
export function createLegacyEventID(event: ethereum.Event): string {
  return event.block.number
    .toString()
    .concat("-")
    .concat(event.logIndex.toString());
}

// These nodes remain Bytes for hashing and ENSv2 namehash comparisons.
export const ETH_NODE = Bytes.fromHexString(
  "0x93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae"
);
export const ROOT_NODE = Bytes.fromHexString(
  "0x0000000000000000000000000000000000000000000000000000000000000000"
);
// Legacy relations use string IDs; raw event parameters use byte arrays.
export const EMPTY_ADDRESS = "0x0000000000000000000000000000000000000000";

// Largest expiry written to the legacy Domain / Registration / WrappedDomain
// expiryDate fields. ENSv2 uses uint64 max (2^64-1) as "never expires" (root
// TLDs, the reverse mirror, migrated emancipated subnames with
// CAN_EXTEND_EXPIRY, and Graveyard-parked v1 names renewed to the cap), and
// consumers that turn expiryDate into a timestamp cannot represent that:
// JavaScript Date overflows past 8.64e15 ms, PostgreSQL to_timestamp fails
// with "timestamp out of range", and Rust's chrono stops at year 262142
// (v2.1.0's clamp, the JavaScript Date maximum in year 275760, still
// panicked Rust consumers such as Blockscout's BENS). 253402300799 s is
// 9999-12-31T23:59:59Z, the last instant of a four-digit ISO 8601 date, which
// every mainstream date type can hold, so a never-expiring name reads as the
// furthest future instant consumers can agree on. Native ENSv2 entities and
// the ENSv1 event entities keep the raw on-chain value.
export const MAX_LEGACY_EXPIRY = BigInt.fromString("253402300799");

export function clampLegacyExpiry(expiry: BigInt): BigInt {
  return expiry.gt(MAX_LEGACY_EXPIRY) ? MAX_LEGACY_EXPIRY : expiry;
}
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

// Four-byte big-endian encoding for counters used in composite IDs.
export function i32ToBytes(i: i32): ByteArray {
  let out = new Uint8Array(4);
  out[0] = ((i >> 24) & 0xff) as u8;
  out[1] = ((i >> 16) & 0xff) as u8;
  out[2] = ((i >> 8) & 0xff) as u8;
  out[3] = (i & 0xff) as u8;
  return changetype<ByteArray>(out);
}

// Account uses a string ID while ENSv2 callers hold addresses as Bytes.
export function createOrLoadAccount(address: Bytes): Account {
  let id = address.toHexString();
  let account = Account.load(id);
  if (account == null) {
    account = new Account(id);
    account.save();
  }

  return account;
}

export function createOrLoadDomain(node: Bytes): Domain {
  let id = node.toHexString();
  let domain = Domain.load(id);
  if (domain == null) {
    domain = new Domain(id);
    domain.storedOffchain = false; // Blockscout/BENS extension
    domain.resolvedWithWildcard = false; // Blockscout/BENS extension
    domain.save();
  }

  return domain;
}

export function checkValidLabel(name: string | null): boolean {
  if (name == null) {
    return false;
  }
  // for compiler
  name = name!;
  for (let i = 0; i < name.length; i++) {
    let charCode = name.charCodeAt(i);
    if (charCode === 0) {
      // 0 = null byte
      log.warning("Invalid label '{}' contained null byte. Skipping.", [name]);
      return false;
    } else if (charCode === 46) {
      // 46 = .
      log.warning(
        "Invalid label '{}' contained separator char '.'. Skipping.",
        [name]
      );
      return false;
    } else if (charCode === 91) {
      // 91 = [
      log.warning("Invalid label '{}' contained char '['. Skipping.", [name]);
      return false;
    } else if (charCode === 93) {
      // 93 = ]
      log.warning("Invalid label '{}' contained char ']'. Skipping.", [name]);
      return false;
    }
  }

  return true;
}
