// ENSv2 ID and helper functions. Reuse src/utils.ts for anything not
// specific to the registry-scoped ENSv2 entity model (concat,
// checkValidLabel, createEventID, uint256ToByteArray, i32ToBytes,
// createOrLoadAccount, createOrLoadDomain) — do not duplicate those here.
//
// Every composite ID below is a fixed-width Bytes concatenation. Each
// component is either fixed-width already or made fixed-width via
// uint256ToByteArray (32-byte big-endian BigInt) / i32ToBytes (4-byte
// big-endian counter), so no delimiter is needed.
import { Address, BigInt, Bytes, crypto } from "@graphprotocol/graph-ts";
import { concat, i32ToBytes, uint256ToByteArray } from "./utils";
import {
  ENSv2Namespace,
  ENSv2Registry,
  ENSv2RegistryNamespaceIndex,
} from "./types/schema";

// 2^32, used to zero the lower 32 bits of a BigInt without relying on
// bitwise helpers that are not part of the mapping runtime API.
const TWO_POW_32 = BigInt.fromI64(4294967296);

export function nameSlotId(registry: Bytes, slotId: BigInt): Bytes {
  return Bytes.fromByteArray(concat(registry, uint256ToByteArray(slotId)));
}

export function resourceId(registry: Bytes, resource: BigInt): Bytes {
  return Bytes.fromByteArray(concat(registry, uint256ToByteArray(resource)));
}

export function tokenEntityId(registry: Bytes, tokenId: BigInt): Bytes {
  return Bytes.fromByteArray(concat(registry, uint256ToByteArray(tokenId)));
}

export function resolverRecordId(resolver: Bytes, recordId: BigInt): Bytes {
  return Bytes.fromByteArray(concat(resolver, uint256ToByteArray(recordId)));
}

export function resolverLinkId(resolver: Bytes, node: Bytes): Bytes {
  return Bytes.fromByteArray(concat(resolver, node));
}

export function resolverMembershipId(record: Bytes, node: Bytes): Bytes {
  return Bytes.fromByteArray(concat(record, node));
}

export function resolverLinkIndexId(record: Bytes, index: i32): Bytes {
  return Bytes.fromByteArray(concat(record, i32ToBytes(index)));
}

export function resolverAddressId(record: Bytes, coinType: BigInt): Bytes {
  return Bytes.fromByteArray(concat(record, uint256ToByteArray(coinType)));
}

export function resolverAddressIndexId(record: Bytes, index: i32): Bytes {
  return Bytes.fromByteArray(concat(record, i32ToBytes(index)));
}

export function resolverTextId(record: Bytes, keyHash: Bytes): Bytes {
  return Bytes.fromByteArray(concat(record, keyHash));
}

export function resolverTextIndexId(record: Bytes, index: i32): Bytes {
  return Bytes.fromByteArray(concat(record, i32ToBytes(index)));
}

// Port of LibLabel.withVersion(anyId, 0) from contracts-v2's
// contracts/src/utils/LibLabel.sol:
//   withVersion(anyId, versionId) = anyId ^ uint32(anyId) ^ versionId
// For versionId = 0 this reduces to `anyId ^ uint32(anyId)`, which zeroes
// the lower 32 bits of anyId (XOR-ing a value with its own low 32 bits
// clears them; higher bits are untouched since the low-32 value
// zero-extends). Equivalent to `anyId - (anyId mod 2^32)`.
export function toSlotId(anyId: BigInt): BigInt {
  return anyId.minus(anyId.mod(TWO_POW_32));
}

export function isZeroAddress(a: Address): boolean {
  return a.equals(Address.zero());
}

export function slotPathIndexId(slotId: Bytes, index: i32): Bytes {
  return Bytes.fromByteArray(concat(slotId, i32ToBytes(index)));
}

export function registryNamespaceIndexId(
  registryId: Bytes,
  index: i32
): Bytes {
  return Bytes.fromByteArray(concat(registryId, i32ToBytes(index)));
}

export function pathNamespaceIndexId(pathId: Bytes, index: i32): Bytes {
  return Bytes.fromByteArray(concat(pathId, i32ToBytes(index)));
}

// Param named nsId, not namespaceId, to avoid shadowing this file's own
// namespaceId() function above.
export function namespacePathIndexId(nsId: Bytes, index: i32): Bytes {
  return Bytes.fromByteArray(concat(nsId, i32ToBytes(index)));
}

export function namespaceId(registryId: Bytes, baseNamehash: Bytes): Bytes {
  return Bytes.fromByteArray(concat(registryId, baseNamehash));
}

export function namespaceLinkId(
  parentRegistryId: Bytes,
  parentSlotId: BigInt,
  childAddress: Bytes
): Bytes {
  return Bytes.fromByteArray(
    concat(
      concat(parentRegistryId, uint256ToByteArray(parentSlotId)),
      childAddress
    )
  );
}

// ENSv2NamePath.id = namehash hex (schema's own ID comment), computed the
// same way as ensRegistry.ts::makeSubnode's keccak256(concat(parentNode,
// labelHash)) — here parentNode is a namespace's baseNamehash. Lives here
// (not ensv2Paths.ts, where it originated) because ensv2Domain.ts also
// needs it to recover a slot's Domain ID at transfer time, and
// ensv2Paths.ts already imports from ensv2Domain.ts — putting it in this
// dependency-free utils file avoids a circular import either way.
export function pathNamehash(baseNamehash: Bytes, labelhash: Bytes): Bytes {
  return Bytes.fromByteArray(crypto.keccak256(concat(baseNamehash, labelhash)));
}

// Shared "append index row, bump counter" sequence — used both when a
// registry's root namespace is bootstrapped (ensv2Discovery.ts) and when a
// namespace is created/found via a subregistry link (ensv2Paths.ts). Used to
// live here to avoid duplication and circular imports.
export function appendRegistryNamespaceIndex(
  registry: ENSv2Registry,
  namespace: ENSv2Namespace
): void {
  let index = new ENSv2RegistryNamespaceIndex(
    registryNamespaceIndexId(registry.id, registry.namespaceCount)
  );
  index.registry = registry.id;
  index.index = registry.namespaceCount;
  index.namespace = namespace.id;
  index.save();

  registry.namespaceCount = registry.namespaceCount + 1;
  registry.save();
}
