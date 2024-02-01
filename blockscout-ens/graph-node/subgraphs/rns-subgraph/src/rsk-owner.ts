import {
    ExpirationChanged as ExpirationChangedEvent,
  } from "../generated/RSKOwner/RSKOwner"
  import { ByteArray, crypto } from "@graphprotocol/graph-ts";
import { Domain, ExpiryExtended } from "../generated/schema"

import {
    RSK_NODE,
    byteArrayFromHex,
    concat,
    createEventID,
    uint256ToByteArray,
    EMPTY_ADDRESS,
  } from "./utils";


var rskNode: ByteArray = byteArrayFromHex(RSK_NODE.slice(2));

export function handleExpirationChanged(event: ExpirationChangedEvent): void {
    let label = uint256ToByteArray(event.params.tokenId);
    const node = crypto.keccak256(concat(rskNode, label)).toHex();
    let domain = Domain.load(node);
    if (domain === null) {
        // handle case that sometimes for some reason 
        // expiration changed goes BEFORE creation of domain...
        domain = new Domain(node);
        domain.createdAt = event.block.timestamp;
        domain.isMigrated = true;
        domain.subdomainCount = 0;
        domain.owner = EMPTY_ADDRESS;
    }
    domain.expiryDate = event.params.expirationTime;
    domain.save();
    let domainEvent = new ExpiryExtended(createEventID(event));
    domainEvent.blockNumber = event.block.number.toI32();
    domainEvent.transactionID = event.transaction.hash;
    domainEvent.domain = domain.id;
    domainEvent.expiryDate =  event.params.expirationTime;
    domainEvent.save();
  }