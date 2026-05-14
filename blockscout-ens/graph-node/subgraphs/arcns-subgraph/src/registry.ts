import { Bytes } from "@graphprotocol/graph-ts";
import {
  Transfer as TransferEvent,
  NewResolver as NewResolverEvent,
} from "../generated/Registry/Registry";
import {
  Domain,
  Transfer as TransferEntity,
  NewResolver as NewResolverEntity,
} from "../generated/schema";
import { getOrCreateAccount, getOrCreateResolver } from "./utils";

// ─── Registry Transfer ────────────────────────────────────────────────────────
// Registry.Transfer(node, newOwner) — registry-level ownership change.
// This fires alongside the ERC-721 Transfer from the BaseRegistrar.
// We update Domain.owner here (registry owner = BENS "owner" field).

export function handleRegistryTransfer(event: TransferEvent): void {
  let nodeHex = event.params.node.toHexString();
  let domain = Domain.load(nodeHex);
  if (!domain) return;

  let newOwnerAccount = getOrCreateAccount(event.params.owner);
  domain.owner = newOwnerAccount.id;
  domain.save();

  // Transfer domain event
  let transferId =
    event.transaction.hash.toHexString() +
    "-" +
    event.logIndex.toString() +
    "-registry-transfer";
  let transferEvent = new TransferEntity(transferId);
  transferEvent.domain = nodeHex;
  transferEvent.owner = newOwnerAccount.id;
  transferEvent.blockNumber = event.block.number.toI32();
  transferEvent.transactionID = event.transaction.hash;
  transferEvent.save();
}

// ─── NewResolver ──────────────────────────────────────────────────────────────
// Registry.NewResolver(node, resolver) — resolver address changed for a node.
// Creates or updates the Resolver entity and links it to the Domain.

export function handleNewResolver(event: NewResolverEvent): void {
  let nodeHex = event.params.node.toHexString();
  let domain = Domain.load(nodeHex);
  if (!domain) return;

  let resolverAddress = event.params.resolver;

  // Zero address means resolver was cleared
  if (
    resolverAddress.toHexString().toLowerCase() ==
    "0x0000000000000000000000000000000000000000"
  ) {
    domain.resolver = null;
    domain.save();
    return;
  }

  let resolver = getOrCreateResolver(resolverAddress, nodeHex);
  resolver.domain = nodeHex;
  resolver.save();

  domain.resolver = resolver.id;
  domain.save();

  // NewResolver domain event
  let newResolverId =
    event.transaction.hash.toHexString() +
    "-" +
    event.logIndex.toString() +
    "-newresolver";
  let newResolverEvent = new NewResolverEntity(newResolverId);
  newResolverEvent.domain = nodeHex;
  newResolverEvent.resolver = resolver.id;
  newResolverEvent.blockNumber = event.block.number.toI32();
  newResolverEvent.transactionID = event.transaction.hash;
  newResolverEvent.save();
}
