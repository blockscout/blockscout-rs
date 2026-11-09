// Import types and APIs from graph-ts
import {
  Address,
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
import { isGraveyard } from "./ensv2Constants";

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

// The ENSv2 Graveyard retires expired ENSv1 .eth names (Graveyard.clear() in
// ensdomains/contracts-v2, run on mainnet by ENS Labs' Morticia daemon):
// BaseRegistrar.register(labelhash, graveyard, type(uint64).max - now -
// GRACE_PERIOD), after which the resolver is cleared and any subnames are
// moved to the Graveyard. On chain the BaseRegistrar token is burned and
// re-minted to the Graveyard with a practically infinite expiry, so taken
// literally the name would read as a live registration owned by the
// Graveyard forever. It is nothing of the sort: the name lapsed, nobody holds
// it, and it can be registered again (on ENSv2). Mainnet's first sweep
// covered ~946,000 names, about a fifth of all .eth registrations.
//
// So a registration to the Graveyard is treated as the retirement it is: the
// Registration and Domain keep the lapsed registrant and expiry (exactly the
// state an expired name had before the sweep, which every consumer already
// renders as expired/available), while the NameRegistered / NameTransferred
// history rows record what happened on chain. Domain.owner still follows the
// registry's NewOwner to the Graveyard, as it does for migrated names.
//
// A Graveyard registration is distinguishable from a migration: the
// migration controllers move a *live* token with a plain transfer (from the
// holder, no burn, no NameRegistered) and renew it afterwards, whereas
// clear() only registers, and BaseRegistrar.register of an expired name
// always burns the old token (Transfer to 0x0) and mints a new one (Transfer
// from 0x0) before emitting NameRegistered, all in the same transaction.
// isGraveyard() (ensv2Constants.ts) is the test used in both handlers.

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

  let registration: Registration;
  if (isGraveyard(event.params.owner)) {
    let lapsed = Registration.load(labelHex);
    if (lapsed == null) {
      // clear() requires BaseRegistrar.nameExpires(label) != 0, so the name
      // was registered on chain before; only a gap in the indexed history
      // gets here. Nothing sensible can be recorded without the lapsed
      // state (the event row needs a Registration too), so leave the name
      // as it is rather than inventing a Graveyard-owned registration.
      log.warning(
        "handleNameRegistered: Graveyard retirement of {} without a prior Registration, skipping",
        [domainId]
      );
      return;
    }
    // Lapsed registrant, registrationDate and expiryDate stay as they were.
    registration = lapsed;
  } else {
    registration = new Registration(labelHex);
    registration.domain = domain.id;
    registration.registrationDate = event.block.timestamp;
    registration.expiryDate = clampLegacyExpiry(event.params.expires);
    registration.registrant = account.id;

    domain.registrant = account.id;
    domain.expiryDate = clampLegacyExpiry(
      event.params.expires.plus(GRACE_PERIOD_SECONDS)
    );
  }

  // A fresh .eth registration means any previous wrapped registration has lapsed
  // (a name leaving the NameWrapper by expiry emits no NameUnwrapped event, so the
  // wrapped state would otherwise go stale). Clear it here. If this is a
  // register-and-wrap in the same tx, the BaseRegistrar NameRegistered event fires
  // before the NameWrapper NameWrapped/TransferSingle events, which recreate the
  // WrappedDomain with the correct new owner. A Graveyard retirement of an
  // expired wrapped name clears it for the same reason: the wrapped
  // registration lapsed (the stale ERC-1155 token stays with its last holder
  // but NameWrapper.getData reports it expired and ownerless).
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

  // BaseRegistrar burns (Transfer to 0x0) only inside _register, immediately
  // before minting the token again to the new registrant, so the burn never
  // needs to touch the registrant: the mint, or the NameRegistered that
  // follows it in the same transaction, sets the real one. Leaving it alone
  // is what lets a Graveyard retirement (see the note above
  // handleNameRegistered) keep the
  // lapsed registrant, since the mint to the Graveyard is skipped as well.
  let isBurn = event.params.to.equals(Address.zero());
  let isMintToGraveyard =
    event.params.from.equals(Address.zero()) &&
    isGraveyard(event.params.to);
  if (!isBurn && !isMintToGraveyard) {
    registration.registrant = account.id;
    domain.registrant = account.id;

    domain.save();
    registration.save();
  }

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
