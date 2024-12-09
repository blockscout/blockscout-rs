import { Address, BigInt, Bytes, ethereum } from "@graphprotocol/graph-ts";

import {
  SLDMintedForOrder as SLDMintedForOrderEvent,
  SLDRenewed as SLDRenewedEvent,
  SLDMinted as SLDMintedEvent,
  Transfer as TransferEvent
} from "../generated/Registry/Registry"
import { Domain, Account } from "../generated/schema"
import { EMPTY_ADDRESS, ROOT_NODE, hashByName, keccakFromStr } from "./utils";

const BIG_INT_ZERO = BigInt.fromI32(0);

function createDomain(node: string, timestamp: BigInt): Domain {
  let domain = new Domain(node);
  if (node == ROOT_NODE) {
    domain = new Domain(node);
    domain.owner = EMPTY_ADDRESS;
    domain.isMigrated = true;
    domain.createdAt = timestamp;
    domain.subdomainCount = 0;
    domain.storedOffchain = false;
    domain.resolvedWithWildcard = false;
  }
  return domain;
}

function getDomain(
  node: string,
  timestamp: BigInt = BIG_INT_ZERO
): Domain | null {
  let domain = Domain.load(node);
  if (domain === null && node == ROOT_NODE) {
    return createDomain(node, timestamp);
  } else {
    return domain;
  }
}


export function handleSLDMinted(event: SLDMintedEvent): void {
  _handleNewDomain(event.params.tokenId, event.params.to, event.params.label, event.params.tld, event.params.expiration, event.block);
}

export function handleSLDMintedForOrder(event: SLDMintedForOrderEvent): void {
  _handleNewDomain(event.params.tokenId, event.params.to, event.params.label, event.params.tld, event.params.expiration, event.block);
}

export function handleSLDRenewed(event: SLDRenewedEvent): void {
  _handleRenewed(event.params.tokenId, event.params.expiration);
}


export function handleTransfer(event: TransferEvent): void {
  _handleTransfer(event.params.tokenId, event.params.to, event.params.from);
}

function _handleNewDomain(tokenId: BigInt, to: Address, label: string, tld: string, expiration: BigInt, block: ethereum.Block): void {
  let account = new Account(to.toHexString());
  account.save();

  let node = hashByName(tld);
  let subnode = hashByName(label + "." + tld);

  let domain = getDomain(subnode.toHexString());
  let parent = getDomain(node.toHexString());

  if (domain === null) {
    domain = new Domain(subnode.toHexString());
    domain.createdAt = block.timestamp;
    domain.subdomainCount = 0;
    domain.storedOffchain = false;
    domain.resolvedWithWildcard = false;
  }

  if (domain.parent === null && parent !== null) {
    parent.subdomainCount = parent.subdomainCount + 1;
    parent.save();
  }

  if (domain.name == null) {
    domain.labelName = label;
    domain.name = label + "." + tld;
  }

  // if (domain.resolvedAddress == null) {
  //   domain.resolvedAddress = to.toHexString();
  // }

  domain.owner = to.toHexString();
  domain.parent = node.toHexString();
  domain.labelhash = Bytes.fromByteArray(keccakFromStr(label));
  domain.isMigrated = true;
  domain.save();
}

function _handleRenewed(tokenId: BigInt, expiration: BigInt): void {
    let domain = getDomain(tokenId.toHexString());
   
    if (domain) {
      domain.expiryDate = expiration;
      domain.save();
    }    
}

function _handleTransfer(tokenId: BigInt, to: Address, from: Address): void {
  let domain = getDomain(tokenId.toHexString());
  if (domain) {
    domain.owner = to.toHexString();
    domain.save();
  }
}

// // Handler for Transfer events
// export function handleTransfer(event: TransferEvent): void {
//   let node = event.params.node.toHexString();

//   let account = new Account(event.params.owner.toHexString());
//   account.save();

//   // Update the domain owner
//   let domain = getDomain(node)!;

//   domain.owner = event.params.owner.toHexString();
//   saveDomain(domain);

//   let domainEvent = new Transfer(createEventID(event));
//   domainEvent.blockNumber = event.block.number.toI32();
//   domainEvent.transactionID = event.transaction.hash;
//   domainEvent.domain = node;
//   domainEvent.owner = event.params.owner.toHexString();
//   domainEvent.save();
// }

// // Handler for NewResolver events
// export function handleNewResolver(event: NewResolverEvent): void {
//   let id: string | null;

//   // if resolver is set to 0x0, set id to null
//   // we don't want to create a resolver entity for 0x0
//   if (event.params.resolver.equals(EMPTY_ADDRESS_BYTEARRAY)) {
//     id = null;
//   } else {
//     id = event.params.resolver
//       .toHexString()
//       .concat("-")
//       .concat(event.params.node.toHexString());
//   }

//   let node = event.params.node.toHexString();
//   let domain = getDomain(node)!;
//   domain.resolver = id;

//   if (id) {
//     let resolver = Resolver.load(id);
//     if (resolver == null) {
//       resolver = new Resolver(id);
//       resolver.domain = event.params.node.toHexString();
//       resolver.address = event.params.resolver;
//       resolver.save();
//       // since this is a new resolver entity, there can't be a resolved address yet so set to null
//       domain.resolvedAddress = null;
//     } else {
//       domain.resolvedAddress = resolver.addr;
//     }
//   } else {
//     domain.resolvedAddress = null;
//   }
//   saveDomain(domain);

//   let domainEvent = new NewResolver(createEventID(event));
//   domainEvent.blockNumber = event.block.number.toI32();
//   domainEvent.transactionID = event.transaction.hash;
//   domainEvent.domain = node;
//   domainEvent.resolver = id ? id : EMPTY_ADDRESS;
//   domainEvent.save();
// }

// // Handler for NewTTL events
// export function handleNewTTL(event: NewTTLEvent): void {
//   let node = event.params.node.toHexString();
//   let domain = getDomain(node);
//   // For the edge case that a domain's owner and resolver are set to empty
//   // in the same transaction as setting TTL
//   if (domain) {
//     domain.ttl = event.params.ttl;
//     domain.save();
//   }

//   let domainEvent = new NewTTL(createEventID(event));
//   domainEvent.blockNumber = event.block.number.toI32();
//   domainEvent.transactionID = event.transaction.hash;
//   domainEvent.domain = node;
//   domainEvent.ttl = event.params.ttl;
//   domainEvent.save();
// }

// export function handleNewOwner(event: NewOwnerEvent): void {
//   _handleNewOwner(event, true);
// }
