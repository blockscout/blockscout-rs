import { BigInt, ethereum } from "@graphprotocol/graph-ts";

export const EMPTY_ADDRESS = "0x0000000000000000000000000000000000000000";
export const GWEI_NODE =
  "0xcca9c7f2dbe2808af0de2982fc84314bfa68a82a6a60ad5cd757f91a233d7d7f";

export function tokenIdToDomainID(tokenId: BigInt): string {
  let value = tokenId.toHexString().slice(2);
  return "0x".concat("0".repeat(64 - value.length)).concat(value);
}

export function createEventID(event: ethereum.Event): string {
  return event.block.number
    .toString()
    .concat("-")
    .concat(event.transaction.index.toString())
    .concat("-")
    .concat(event.logIndex.toString());
}

export function createResolverID(
  event: ethereum.Event,
  domainID: string
): string {
  // A fresh resolver entity per registration prevents versioned resolver data
  // from a previous registration leaking into a re-registered name.
  return event.address
    .toHexString()
    .concat("-")
    .concat(domainID)
    .concat("-")
    .concat(event.transaction.hash.toHexString())
    .concat("-")
    .concat(event.logIndex.toString());
}
