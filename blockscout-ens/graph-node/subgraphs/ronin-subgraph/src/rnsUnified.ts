import { Address, BigInt } from "@graphprotocol/graph-ts";

import {
  RecordUpdated as RecordUpdatedEvent,
  Transfer as TransferEvent,
} from "./types/RNSUnified/RNSUnified";
import {
  Domain,
  ExpiryExtended,
  NameTransferred,
  NewOwner,
  NewResolver,
  Registration,
  Transfer,
} from "./types/schema";
import {
  applyLabel,
  createChild,
  loadDomain,
  loadRoot,
  refreshFromChain,
  saveDomain,
  setOwner,
  setResolver,
} from "./domain";
import {
  createEventID,
  createSubEventID,
  EMPTY_ADDRESS,
  EXPIRY_FIELD,
  hasField,
  IMMUTABLE_FIELDS,
  nodeId,
  RESOLVER_FIELD,
} from "./utils";

// mint() emits RecordUpdated with every field set, the label included.
function mintedDomain(event: RecordUpdatedEvent): Domain | null {
  let node = event.params.node;
  let record = event.params.record;
  let parent = loadDomain(record.immut.parentId, event.address, event.block);
  if (parent == null) return null;

  let domain = createChild(nodeId(node), node, parent, event.block.timestamp);
  applyLabel(domain, parent, record.immut.label);
  setOwner(domain, record.mut.owner.toHexString());

  let newOwner = new NewOwner(createEventID(event));
  newOwner.blockNumber = event.block.number.toI32();
  newOwner.transactionID = event.transaction.hash;
  newOwner.parentDomain = parent.id;
  newOwner.domain = domain.id;
  newOwner.owner = domain.owner;
  newOwner.save();
  return domain;
}

export function handleRecordUpdated(event: RecordUpdatedEvent): void {
  let node = event.params.node;
  if (node.isZero()) {
    // initialize() records the root token.
    loadRoot(event.block.timestamp);
    return;
  }

  let indicator = event.params.indicator;
  let record = event.params.record;
  let isMint = hasField(indicator, IMMUTABLE_FIELDS);
  let domain = isMint
    ? mintedDomain(event)
    : loadDomain(node, event.address, event.block);
  if (domain == null) return;

  if (hasField(indicator, RESOLVER_FIELD)) {
    setResolver(domain, record.mut.resolver);
    if (!isMint || domain.resolver !== null) {
      let newResolver = new NewResolver(createSubEventID(event, "resolver"));
      newResolver.blockNumber = event.block.number.toI32();
      newResolver.transactionID = event.transaction.hash;
      newResolver.domain = domain.id;
      let resolver = domain.resolver;
      newResolver.resolver = resolver !== null ? resolver : EMPTY_ADDRESS;
      newResolver.save();
    }
  }

  if (hasField(indicator, EXPIRY_FIELD)) {
    domain.expiryDate = record.mut.expiry;
    refreshFromChain(domain, node, event.address);
    if (!isMint) {
      let extended = new ExpiryExtended(createSubEventID(event, "expiry"));
      extended.blockNumber = event.block.number.toI32();
      extended.transactionID = event.transaction.hash;
      extended.domain = domain.id;
      extended.expiryDate = domain.expiryDate!;
      extended.save();
    }
  }

  // The owner bit is not read here: setRecord() emits a zero placeholder owner
  // before it transfers the token, and every ownership change also emits the
  // ERC-721 Transfer that handleTransfer applies.
  saveDomain(domain);
}

export function handleTransfer(event: TransferEvent): void {
  let node = event.params.tokenId;
  // The root token belongs to the admin and is not a name.
  if (node.isZero()) return;

  // A first mint: the RecordUpdated that follows carries the whole record.
  if (
    event.params.from.equals(Address.zero()) &&
    Domain.load(nodeId(node)) == null
  ) {
    return;
  }

  let domain = loadDomain(node, event.address, event.block);
  if (domain == null) return;

  let to = event.params.to.toHexString();
  setOwner(domain, to);
  if (to == EMPTY_ADDRESS) {
    // _burn() deletes the mutable record; a re-mint in the same transaction
    // sets it again.
    domain.expiryDate = BigInt.zero();
  } else {
    refreshFromChain(domain, node, event.address);
  }
  saveDomain(domain);

  let transfer = new Transfer(createEventID(event));
  transfer.blockNumber = event.block.number.toI32();
  transfer.transactionID = event.transaction.hash;
  transfer.domain = domain.id;
  transfer.owner = domain.owner;
  transfer.save();

  let registration = Registration.load(domain.id);
  if (registration != null && to != EMPTY_ADDRESS) {
    registration.registrant = domain.owner;
    registration.save();

    let transferred = new NameTransferred(createEventID(event));
    transferred.blockNumber = event.block.number.toI32();
    transferred.transactionID = event.transaction.hash;
    transferred.registration = registration.id;
    transferred.newOwner = domain.owner;
    transferred.save();
  }
}
