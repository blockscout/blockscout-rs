import { Address, ByteArray, ethereum, crypto, Bytes } from "@graphprotocol/graph-ts";

import { SetReverseMapping } from '../generated/Resolver/Resolver';
import { Domain, NameChanged } from '../generated/schema';
import { hashByName } from "./utils";

const ADDR_REVERSE_NODE = "0x91d1777781884d03a6757a803996e38de2a42967fb37eeaca72729271025a9e2";

export function handleSetReverseMapping(event: SetReverseMapping): void {
  let name = `${event.params.wallet.toHexString().slice(2)}.addr.reverse`;
  let subnode = hashByName(name);
  let subnodeStr = subnode.toHexString();
  let domain = Domain.load(subnodeStr);
  let resolverId = createResolverID(subnode, event.address);

  if (domain == null) {
    domain = new Domain(subnodeStr);
    domain.createdAt = event.block.timestamp;
    domain.parent = ADDR_REVERSE_NODE;
    domain.name = name;

    domain.resolver = resolverId

    domain.subdomainCount = 0;
    domain.isMigrated = true;
    domain.owner = event.transaction.from.toHexString();
    domain.storedOffchain = false;
    domain.resolvedWithWildcard = false;

    domain.save();
  }

  let resolvedDomainNode = hashByName(event.params.name);
  let resolvedDomain = Domain.load(resolvedDomainNode.toHexString());
  if (resolvedDomain == null) {
    resolvedDomain = new Domain(resolvedDomainNode.toHexString());
    resolvedDomain.createdAt = event.block.timestamp;
    resolvedDomain.name = event.params.name;

    resolvedDomain.subdomainCount = 0;
    resolvedDomain.isMigrated = true;
    resolvedDomain.owner = event.params.wallet.toHexString();
    resolvedDomain.storedOffchain = false;
    resolvedDomain.resolvedWithWildcard = false;
  }

  resolvedDomain.resolver = resolverId;
  resolvedDomain.resolvedAddress = event.params.wallet.toHexString();
  resolvedDomain.save();

  let resolverEvent = new NameChanged(createEventID(event));
  resolverEvent.resolver = resolverId
  resolverEvent.blockNumber = event.block.number.toI32();
  resolverEvent.transactionID = event.transaction.hash;
  resolverEvent.name = event.params.name;
  resolverEvent.save();
}

function createEventID(event: ethereum.Event): string {
    return event.block.number
        .toString()
        .concat("-")
        .concat(event.logIndex.toString());
}

function createResolverID(node: ByteArray, resolver: Address): string {
    return resolver
      .toHexString()
      .concat("-")
      .concat(node.toHexString());
  }
