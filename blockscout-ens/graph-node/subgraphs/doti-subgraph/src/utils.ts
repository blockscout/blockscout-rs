import { BigInt, ByteArray, Bytes, crypto, ethereum, log } from "@graphprotocol/graph-ts";
import { Account, Domain } from "../generated/schema";

export const BASE_NODE_HASH = "96b16e885d568c078028f8feef27a718c0e2a3cf42145b2100806cb1f07f4bb7";
export const BASE_NODE = ".i";
export const COIN_TYPE_ETH_BIGINT = BigInt.fromI32(60);
export const COIN_TYPE_ARBITRUM_BIGINT = BigInt.fromI64(2147525809); // Arbitrum One (0x80000000 | 42161)
export const COIN_TYPE_OP_BIGINT = BigInt.fromI64(2147483658); // OP Mainnet (0x80000000 | 10)
export const COIN_TYPE_BIGINT = BigInt.fromI64(2147525809); // Arbitrum One
export const COIN_TYPE = 2147525809;

/**
 * Checks if the coin type matches supported network address coin types (Ethereum 60, Arbitrum One, or Optimism).
 * Safely compares BigInt values without conversion to avoid integer overflow assertion errors.
 * @param coinType The coin type as a BigInt.
 * @returns True if the coin type matches supported network address types.
 */
export function isNetworkCoinType(coinType: BigInt): boolean {
  return (
    coinType.equals(COIN_TYPE_ETH_BIGINT) ||
    coinType.equals(COIN_TYPE_ARBITRUM_BIGINT) ||
    coinType.equals(COIN_TYPE_OP_BIGINT)
  );
}

/**
 * Validates and converts an address byte array into a hex string.
 * @param address The raw address bytes.
 * @returns A formatted 20-byte hex string or the empty address placeholder.
 */
export function safeAddress(address: Bytes): string {
  if (address.length === 20) {
    return address.toHexString();
  } else {
    return EMPTY_ADDRESS;
  }
}

export const ROOT_NODE =
  "0x0000000000000000000000000000000000000000000000000000000000000000";
export const EMPTY_ADDRESS = "0x0000000000000000000000000000000000000000";
export const EMPTY_ADDRESS_BYTEARRAY = new ByteArray(20);

/**
 * Generates a unique event identifier based on block, transaction, and log indexes.
 * @param event The blockchain event.
 * @returns A formatted event ID string.
 */
export function createEventID(event: ethereum.Event): string {
  return event.block.number
    .toString()
    .concat("-")
    .concat(event.transaction.index.toString())
    .concat("-")
    .concat(event.logIndex.toString())
    .concat("-")
    .concat(event.transactionLogIndex.toString());
}

/**
 * Concatenates two byte arrays into a single byte array.
 * @param a The first byte array.
 * @param b The second byte array.
 * @returns The combined ByteArray.
 */
export function concat(a: ByteArray, b: ByteArray): ByteArray {
  let out = new Uint8Array(a.length + b.length);
  for (let i = 0; i < a.length; i++) {
    out[i] = a[i];
  }
  for (let j = 0; j < b.length; j++) {
    out[a.length + j] = b[j];
  }
  // return out as ByteArray
  return changetype<ByteArray>(out);
}

/**
 * Parses a hex string into a ByteArray representation.
 * @param s Hex string with an even number of characters.
 * @returns Parsed ByteArray.
 */
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

/**
 * Converts a 256-bit unsigned integer to a 32-byte ByteArray.
 * @param i BigInt integer.
 * @returns 32-byte padded ByteArray.
 */
export function uint256ToByteArray(i: BigInt): ByteArray {
  let hex = i
    .toHex()
    .slice(2)
    .padStart(64, "0");
  return byteArrayFromHex(hex);
}

/**
 * Loads an existing Account entity or creates and saves a new one.
 * @param address The hex string account address.
 * @returns The Account entity.
 */
export function createOrLoadAccount(address: string): Account {
  let account = Account.load(address);
  if (account == null) {
    account = new Account(address);
    account.save();
  }

  return account;
}

/**
 * Loads an existing Domain entity or creates and saves a default one.
 * @param node The node hash identifier.
 * @returns The Domain entity.
 */
export function createOrLoadDomain(node: string): Domain {
  let domain = Domain.load(node);
  if (domain == null) {
    domain = new Domain(node);
    domain.storedOffchain = false;
    domain.resolvedWithWildcard = false;
    domain.save();
  }

  return domain;
}

/**
 * Validates that a domain label does not contain null bytes or separator characters.
 * @param name The label string to validate.
 * @returns True if valid, false otherwise.
 */
export function checkValidLabel(name: string): boolean {
  for (let i = 0; i < name.length; i++) {
    let c = name.charCodeAt(i);
    if (c === 0) {
      log.warning("Invalid label '{}' contained null byte. Skipping.", [name]);
      return false;
    } else if (c === 46) {
      log.warning(
        "Invalid label '{}' contained separator char '.'. Skipping.",
        [name]
      );
      return false;
    }
  }

  return true;
}

/**
 * Updates domain label and name records for an existing loaded domain.
 * @param name The domain full name.
 */
export function maybeSaveDomainName(name: string): void {
  const nodehash = hashByName(name);
  const domain = Domain.load(nodehash.toHex());
  if (domain != null) {
    const label = labelFromName(name);
    domain.labelName = label;
    domain.labelhash = Bytes.fromByteArray(keccakFromStr(label));
    domain.name = name;
    domain.save();
  }
}

/**
 * Computes the recursive namehash for a given domain string according to EIP-137.
 * @param name The full domain name.
 * @returns The computed ByteArray namehash.
 */
export function hashByName(name: string): ByteArray {
  if (name === BASE_NODE.slice(1)) {
    return byteArrayFromHex(BASE_NODE_HASH);
  } else if (!name) {
    return byteArrayFromHex(ROOT_NODE.slice(2));
  } else {
    const partition = splitStringOnce(name, '.');
    const label = partition[0];
    const remainder = partition[1];

    return crypto.keccak256(
      concat(
        hashByName(remainder),
        keccakFromStr(label)
      )
    );
  }
}

/**
 * Splits a string into two parts at the first occurrence of the separator.
 * @param input The string to partition.
 * @param separator The character separator.
 * @returns An array containing [head, tail].
 */
function splitStringOnce(input: string, separator: string): string[] {
  let index = input.indexOf(separator);
  if (index >= 0) {
    return [input.slice(0, index), input.slice(index + 1)];
  } else {
    return [input, ''];
  }
}

/**
 * Extracts the primary leftmost label from a domain name.
 * @param name Full domain name.
 * @returns First label component.
 */
function labelFromName(name: string): string {
  const labels = splitStringOnce(name, '.');
  return labels[0];
}

/**
 * Computes the Keccak-256 hash of a UTF-8 string.
 * @param s Input string.
 * @returns Keccak-256 digest ByteArray.
 */
function keccakFromStr(s: string): ByteArray {
  return crypto.keccak256(Bytes.fromUTF8(s));
}
