// Import types and APIs from graph-ts
import {
  BigInt,
  Bytes,
  crypto,
  ens,
  log,
  store,
} from "@graphprotocol/graph-ts";

import {
  checkValidLabel,
  clampLegacyExpiry,
  concat,
  createLegacyEventID,
  ETH_NODE,
  uint256ToByteArray,
} from "./utils";

// Import event types from the registry contract ABI
import {
  ApprovalForAll as BaseRegistrarApprovalForAllEvent,
  ControllerAdded as ControllerAddedEvent,
  ControllerRemoved as ControllerRemovedEvent,
  NameRegistered as NameRegisteredEvent,
  NameRenewed as NameRenewedEvent,
  OwnershipTransferred as BaseRegistrarOwnershipTransferredEvent,
  Transfer as TransferEvent,
} from "./types/BaseRegistrar/BaseRegistrar";

import {
  NameRegistered as LegacyEthRegistrarController_NameRegistered,
  NameRenewed as LegacyEthRegistrarController_NameRenewed,
  OwnershipTransferred as LegacyEthRegistrarController_OwnershipTransferred,
} from "./types/LegacyEthRegistrarController/LegacyEthRegistrarController";
import {
  NameRegistered as UnwrappedEthRegistrarController_NameRegistered,
  NameRenewed as UnwrappedEthRegistrarController_NameRenewed,
  OwnershipTransferred as UnwrappedEthRegistrarController_OwnershipTransferred,
} from "./types/UnwrappedEthRegistrarController/UnwrappedEthRegistrarController";
import {
  NameRegistered as WrappedEthRegistrarController_NameRegistered,
  OwnershipTransferred as WrappedEthRegistrarController_OwnershipTransferred,
} from "./types/WrappedEthRegistrarController/WrappedEthRegistrarController";

import {
  processApprovalForAll,
  processControllerStatus,
  processOwnershipTransferred,
} from "./accessControl";

// Import entity types generated from the GraphQL schema
import {
  Account,
  Domain,
  NameRegistered,
  NameRenewed,
  NameTransferred,
  Registration,
  WrappedDomain,
} from "./types/schema";

const GRACE_PERIOD_SECONDS = BigInt.fromI32(7776000); // 90 days

let rootNode: Bytes = ETH_NODE;

export function handleNameRegistered(event: NameRegisteredEvent): void {
  let account = new Account(event.params.owner.toHexString());
  account.save();

  let label = uint256ToByteArray(event.params.id);
  let labelHex = Bytes.fromByteArray(label).toHexString();
  let domainId = Bytes.fromByteArray(crypto.keccak256(concat(rootNode, label))).toHexString();
  let domain = Domain.load(domainId);
  if (domain == null) {
    // Expected to always exist by the time BaseRegistrar's own
    // NameRegistered fires (ENSRegistry's NewOwner, which creates it, is
    // emitted earlier in the same tx) — but testnets can be reset/redeployed
    // without every historical NewOwner being indexed, so guard rather than
    // crash the whole subgraph on a single irregular name.
    log.warning("handleNameRegistered: no Domain for {}, skipping", [domainId]);
    return;
  }

  let registration = new Registration(labelHex);
  registration.domain = domain.id;
  registration.registrationDate = event.block.timestamp;
  registration.expiryDate = clampLegacyExpiry(event.params.expires);
  registration.registrant = account.id;

  domain.registrant = account.id;
  domain.expiryDate = clampLegacyExpiry(
    event.params.expires.plus(GRACE_PERIOD_SECONDS)
  );

  // A fresh .eth registration means any previous wrapped registration has lapsed
  // (a name leaving the NameWrapper by expiry emits no NameUnwrapped event, so the
  // wrapped state would otherwise go stale). Clear it here. If this is a
  // register-and-wrap in the same tx, the BaseRegistrar NameRegistered event fires
  // before the NameWrapper NameWrapped/TransferSingle events, which recreate the
  // WrappedDomain with the correct new owner.
  domain.wrappedOwner = null;
  if (WrappedDomain.load(domain.id) != null) {
    store.remove("WrappedDomain", domain.id);
  }

  let labelName = ens.nameByHash(label.toHexString());
  if (checkValidLabel(labelName)) {
    domain.labelName = labelName;
    domain.name = labelName! + ".eth";
    registration.labelName = labelName;
  }
  domain.save();
  registration.save();

  let registrationEvent = new NameRegistered(createLegacyEventID(event));
  registrationEvent.registration = registration.id;
  registrationEvent.blockNumber = event.block.number.toI32();
  registrationEvent.transactionID = event.transaction.hash;
  registrationEvent.registrant = account.id;
  registrationEvent.expiryDate = event.params.expires;
  registrationEvent.save();
}

// Legacy controller

export function handleNameRegisteredByLegacyController(
  event: LegacyEthRegistrarController_NameRegistered
): void {
  setNamePreimage(event.params.name, event.params.label, event.params.cost);
}

export function handleNameRenewedByLegacyController(
  event: LegacyEthRegistrarController_NameRenewed
): void {
  setNamePreimage(event.params.name, event.params.label, event.params.cost);
}

// Wrapped controller (reuses same renew event as legacy controller)

export function handleNameRegisteredByWrappedController(
  event: WrappedEthRegistrarController_NameRegistered
): void {
  setNamePreimage(
    event.params.name,
    event.params.label,
    event.params.baseCost.plus(event.params.premium)
  );
}

// Unwrapped controller

export function handleNameRegisteredByUnwrappedController(
  event: UnwrappedEthRegistrarController_NameRegistered
): void {
  setNamePreimage(
    event.params.label,
    event.params.labelhash,
    event.params.baseCost.plus(event.params.premium)
  );
}

export function handleNameRenewedByUnwrappedController(
  event: UnwrappedEthRegistrarController_NameRenewed
): void {
  setNamePreimage(
    event.params.label,
    event.params.labelhash,
    event.params.cost
  );
}

function setNamePreimage(name: string, label: Bytes, cost: BigInt): void {
  if (!checkValidLabel(name)) {
    return;
  }

  let domainId = Bytes.fromByteArray(crypto.keccak256(concat(rootNode, label))).toHexString();
  let domain = Domain.load(domainId);
  if (domain == null) {
    log.warning("setNamePreimage: no Domain for {}, skipping", [domainId]);
    return;
  }
  if (domain.labelName != name) {
    domain.labelName = name;
    domain.name = name + ".eth";
    domain.save();
  }

  let registration = Registration.load(Bytes.fromByteArray(label).toHexString());
  if (registration == null) return;
  registration.labelName = name;
  registration.cost = cost;
  registration.save();
}

export function handleNameRenewed(event: NameRenewedEvent): void {
  let label = uint256ToByteArray(event.params.id);
  let labelHex = Bytes.fromByteArray(label).toHexString();
  let domainId = Bytes.fromByteArray(crypto.keccak256(concat(rootNode, label))).toHexString();
  let registration = Registration.load(labelHex);
  if (registration == null) {
    log.warning("handleNameRenewed: no Registration for {}, skipping", [labelHex]);
    return;
  }
  let domain = Domain.load(domainId);
  if (domain == null) {
    log.warning("handleNameRenewed: no Domain for {}, skipping", [domainId]);
    return;
  }

  // Graveyard-parked migrated names are renewed up to uint64 max minus the
  // grace period; clamp both legacy fields (event rows keep the raw value).
  registration.expiryDate = clampLegacyExpiry(event.params.expires);
  domain.expiryDate = clampLegacyExpiry(
    event.params.expires.plus(GRACE_PERIOD_SECONDS)
  );

  registration.save();
  domain.save();

  // A wrapped .eth 2LD renewed through the BaseRegistrar emits no
  // NameWrapper ExpiryExtended event: the wrapper normalises its expiry on
  // read (registrar expiry + GRACE_PERIOD, see NameWrapper.getData). Mirror
  // that here so WrappedDomain.expiryDate does not go stale (#89).
  let wrappedDomain = WrappedDomain.load(domainId);
  if (wrappedDomain != null) {
    wrappedDomain.expiryDate = domain.expiryDate!;
    wrappedDomain.save();
  }

  let registrationEvent = new NameRenewed(createLegacyEventID(event));
  registrationEvent.registration = registration.id;
  registrationEvent.blockNumber = event.block.number.toI32();
  registrationEvent.transactionID = event.transaction.hash;
  registrationEvent.expiryDate = event.params.expires;
  registrationEvent.save();
}

export function handleNameTransferred(event: TransferEvent): void {
  let account = new Account(event.params.to.toHexString());
  account.save();

  let label = uint256ToByteArray(event.params.tokenId);
  let labelHex = Bytes.fromByteArray(label).toHexString();
  let registration = Registration.load(labelHex);
  if (registration == null) return;

  let domainId = Bytes.fromByteArray(crypto.keccak256(concat(rootNode, label))).toHexString();
  let domain = Domain.load(domainId);
  if (domain == null) {
    log.warning("handleNameTransferred: no Domain for {}, skipping", [domainId]);
    return;
  }

  registration.registrant = account.id;
  domain.registrant = account.id;

  domain.save();
  registration.save();

  let transferEvent = new NameTransferred(createLegacyEventID(event));
  transferEvent.registration = labelHex;
  transferEvent.blockNumber = event.block.number.toI32();
  transferEvent.transactionID = event.transaction.hash;
  transferEvent.newOwner = account.id;
  transferEvent.save();
}

export function handleBaseRegistrarApprovalForAll(
  event: BaseRegistrarApprovalForAllEvent
): void {
  processApprovalForAll(
    event.address,
    event.params.owner,
    event.params.operator,
    event.params.approved,
    event.block
  );
}

export function handleBaseRegistrarOwnershipTransferred(
  event: BaseRegistrarOwnershipTransferredEvent
): void {
  processOwnershipTransferred(event.address, event.params.newOwner, event.block);
}

export function handleControllerAdded(event: ControllerAddedEvent): void {
  processControllerStatus(
    event.address,
    event.params.controller,
    true,
    event.block
  );
}

export function handleControllerRemoved(event: ControllerRemovedEvent): void {
  processControllerStatus(
    event.address,
    event.params.controller,
    false,
    event.block
  );
}

export function handleLegacyControllerOwnershipTransferred(
  event: LegacyEthRegistrarController_OwnershipTransferred
): void {
  processOwnershipTransferred(event.address, event.params.newOwner, event.block);
}

export function handleWrappedControllerOwnershipTransferred(
  event: WrappedEthRegistrarController_OwnershipTransferred
): void {
  processOwnershipTransferred(event.address, event.params.newOwner, event.block);
}

export function handleUnwrappedControllerOwnershipTransferred(
  event: UnwrappedEthRegistrarController_OwnershipTransferred
): void {
  processOwnershipTransferred(event.address, event.params.newOwner, event.block);
}
