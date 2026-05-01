import { BigInt, Bytes, crypto, ByteArray, Address } from "@graphprotocol/graph-ts";
import { Account, Domain, Resolver } from "../generated/schema";

// ─── Constants ────────────────────────────────────────────────────────────────

// namehash("addr.reverse") — the root node for all reverse records
export const ADDR_REVERSE_NODE =
  "0x91d1777781884d03a6757a803996e38de2a42967fb37eeaca72729271025a9e2";

// namehash("arc")
export const ARC_NODE =
  "0x9a7ad1c5d8b1c60ef156c6723dbf462681d6462768a9e60c53665d7fc1337bae";

// namehash("circle")
export const CIRCLE_NODE =
  "0xb3f3947bd9b363b1955fa597e342731ea6bde24d057527feb2cdfdeb807c2084";

export const ZERO_ADDRESS = "0x0000000000000000000000000000000000000000";

// ─── Namehash ─────────────────────────────────────────────────────────────────

export function namehash(name: string): Bytes {
  let node = Bytes.fromHexString(
    "0x0000000000000000000000000000000000000000000000000000000000000000"
  );
  if (name == "") return node;
  let labels = name.split(".");
  for (let i = labels.length - 1; i >= 0; i--) {
    let labelHash = crypto.keccak256(ByteArray.fromUTF8(labels[i]));
    let nodeArr = ByteArray.fromHexString(node.toHexString());
    let combined = new ByteArray(64);
    for (let j = 0; j < 32; j++) combined[j] = nodeArr[j];
    for (let j = 0; j < 32; j++) combined[32 + j] = labelHash[j];
    node = Bytes.fromByteArray(crypto.keccak256(combined));
  }
  return node;
}

// ─── Account ──────────────────────────────────────────────────────────────────

export function getOrCreateAccount(addr: Bytes): Account {
  let id = addr.toHexString().toLowerCase();
  let account = Account.load(id);
  if (!account) {
    account = new Account(id);
    account.save();
  }
  return account;
}

// ─── Domain ───────────────────────────────────────────────────────────────────

/**
 * Returns the Domain entity for a TLD root node (e.g. "arc" or "circle"),
 * creating it if it does not yet exist.
 * The TLD root Domain has no parent, no registrant, and no expiry.
 */
export function getOrCreateTldDomain(tld: string): Domain {
  let nodeBytes = namehash(tld);
  let nodeHex = nodeBytes.toHexString();

  let domain = Domain.load(nodeHex);
  if (!domain) {
    domain = new Domain(nodeHex);
    domain.name = tld;
    domain.labelName = tld;
    domain.labelhash = Bytes.fromByteArray(
      crypto.keccak256(ByteArray.fromUTF8(tld))
    );
    domain.subdomainCount = 0;
    domain.isMigrated = true;
    domain.storedOffchain = false;
    domain.resolvedWithWildcard = false;
    domain.createdAt = BigInt.fromI32(0);
    // TLD root is owned by the zero address (registry-level ownership)
    let zeroAccount = getOrCreateAccount(
      Bytes.fromHexString(ZERO_ADDRESS)
    );
    domain.owner = zeroAccount.id;
    domain.save();
  }
  return domain;
}

/**
 * Returns the Domain entity for the addr.reverse root node,
 * creating it if it does not yet exist.
 */
export function getOrCreateAddrReverseDomain(): Domain {
  let nodeHex = ADDR_REVERSE_NODE;
  let domain = Domain.load(nodeHex);
  if (!domain) {
    domain = new Domain(nodeHex);
    domain.name = "addr.reverse";
    domain.labelName = "addr";
    domain.labelhash = Bytes.fromByteArray(
      crypto.keccak256(ByteArray.fromUTF8("addr"))
    );
    domain.subdomainCount = 0;
    domain.isMigrated = true;
    domain.storedOffchain = false;
    domain.resolvedWithWildcard = false;
    domain.createdAt = BigInt.fromI32(0);
    let zeroAccount = getOrCreateAccount(
      Bytes.fromHexString(ZERO_ADDRESS)
    );
    domain.owner = zeroAccount.id;
    domain.save();
  }
  return domain;
}

// ─── Resolver ─────────────────────────────────────────────────────────────────

/**
 * Returns the Resolver entity for a given resolver contract address and domain node.
 * BENS Resolver id convention: "{resolverAddress}-{nodeHex}"
 */
export function getOrCreateResolver(
  resolverAddress: Bytes,
  nodeHex: string
): Resolver {
  let id = resolverAddress.toHexString().toLowerCase() + "-" + nodeHex;
  let resolver = Resolver.load(id);
  if (!resolver) {
    resolver = new Resolver(id);
    resolver.address = resolverAddress;
    resolver.domain = nodeHex;
    resolver.save();
  }
  return resolver;
}

// ─── TokenId ──────────────────────────────────────────────────────────────────

/**
 * Converts a BigInt tokenId to a zero-padded 32-byte hex string (0x-prefixed).
 * Used to look up Registration entities keyed by labelhash hex.
 */
export function tokenIdToLabelhashHex(tokenId: BigInt): string {
  let raw = tokenId.toHexString().slice(2); // strip 0x
  while (raw.length < 64) raw = "0" + raw;
  return "0x" + raw;
}
