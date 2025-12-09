import { ByteArray, Bytes, crypto } from "@graphprotocol/graph-ts";

// InfinityName uses keccak256(domain + suffix) instead of ENS namehash
// The suffix is ".blue" based on the contract
export const SUFFIX = ".blue";

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
 * InfinityName uses keccak256(domain + suffix) instead of ENS namehash
 * @param domain The full domain name (e.g., "example.blue")
 * @returns The hash as ByteArray
 */
export function hashByDomainName(domain: string): ByteArray {
  if (!domain) {
    return byteArrayFromHex("0".repeat(64));
  }
  // InfinityName calculates hash as keccak256(domain + suffix)
  // The domain parameter already includes the suffix (e.g., "example.blue")
  return crypto.keccak256(Bytes.fromUTF8(domain));
}

/**
 * Get the label name from a full domain name
 * @param domain The full domain name (e.g., "example.blue")
 * @returns The label name (e.g., "example")
 */
export function getLabelName(domain: string): string {
  const parts = domain.split(".");
  if (parts.length > 0) {
    return parts[0];
  }
  return domain;
}

/**
 * Check if a domain name is valid
 * @param domain The domain name to check
 * @returns True if valid, false otherwise
 */
export function isValidDomain(domain: string): boolean {
  if (!domain || domain.length === 0) {
    return false;
  }
  // Check if domain contains the suffix
  return domain.endsWith(SUFFIX);
}

