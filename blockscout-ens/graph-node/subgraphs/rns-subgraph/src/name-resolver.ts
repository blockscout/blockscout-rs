import { Address, Bytes } from "@graphprotocol/graph-ts";
import { NameChanged as NameChangedEvent } from "../generated/NameResolver/NameResolver"
import { Domain, NameChanged } from "../generated/schema"
import { createEventID, createOrLoadDomain, maybeSaveDomainName } from "./utils";

export function handleNameChanged(event: NameChangedEvent): void {
  const name = event.params.name;
  maybeSaveDomainName(name);

  let resolverEvent = new NameChanged(createEventID(event));
  resolverEvent.resolver = createResolverID(event.params.node, event.address);
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.name = event.params.name;
  resolverEvent.save();
}

function createResolverID(node: Bytes, resolver: Address): string {
  return resolver
    .toHexString()
    .concat("-")
    .concat(node.toHexString());
}