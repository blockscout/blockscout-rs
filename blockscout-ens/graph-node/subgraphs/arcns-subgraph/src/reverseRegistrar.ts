import { BigInt, Bytes } from "@graphprotocol/graph-ts";
import {
  ReverseClaimed as ReverseClaimedEvent,
} from "../generated/ReverseRegistrar/ReverseRegistrar";
import { Domain, NewOwner } from "../generated/schema";
import {
  getOrCreateAccount,
  getOrCreateAddrReverseDomain,
  ADDR_REVERSE_NODE,
} from "./utils";

// ─── ReverseClaimed ───────────────────────────────────────────────────────────
// ReverseRegistrar.ReverseClaimed(addr, node) — emitted when an address claims
// its reverse node under addr.reverse.
//
// In the BENS model, reverse resolution works by querying Domain entities whose
// parent.id == ADDR_REVERSE_NODE. BENS reads Domain.name (set later by
// NameChanged on the Resolver) to get the primary name for an address.
//
// This handler creates the reverse node Domain entity under addr.reverse,
// setting its owner to the claiming address. The Domain.name field will be
// populated later when NameChanged fires on the Resolver.

export function handleReverseClaimed(event: ReverseClaimedEvent): void {
  let addrId = event.params.addr.toHexString().toLowerCase();
  let nodeHex = event.params.node.toHexString();

  // Ensure addr.reverse parent domain exists
  let addrReverseDomain = getOrCreateAddrReverseDomain();

  // Ensure Account exists for the claiming address
  let ownerAccount = getOrCreateAccount(event.params.addr);

  // Create or update the reverse node Domain
  let domain = Domain.load(nodeHex);
  if (!domain) {
    domain = new Domain(nodeHex);
    domain.createdAt = event.block.timestamp;
    domain.subdomainCount = 0;
    domain.storedOffchain = false;
    domain.resolvedWithWildcard = false;
    domain.isMigrated = true;
    // name will be set by NameChanged handler when the primary name is written
    domain.name = addrId + ".addr.reverse";
    domain.labelName = addrId;

    // Increment parent subdomainCount
    addrReverseDomain.subdomainCount = addrReverseDomain.subdomainCount + 1;
    addrReverseDomain.save();
  }

  domain.parent = addrReverseDomain.id;
  domain.owner = ownerAccount.id;
  domain.save();

  // NewOwner domain event (reverse node claimed under addr.reverse)
  let newOwnerId =
    event.transaction.hash.toHexString() +
    "-" +
    event.logIndex.toString() +
    "-reverse-newowner";
  let newOwnerEvent = new NewOwner(newOwnerId);
  newOwnerEvent.parentDomain = addrReverseDomain.id;
  newOwnerEvent.domain = nodeHex;
  newOwnerEvent.owner = ownerAccount.id;
  newOwnerEvent.blockNumber = event.block.number.toI32();
  newOwnerEvent.transactionID = event.transaction.hash;
  newOwnerEvent.save();
}
