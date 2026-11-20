import {
  NameRegistered as NameRegisteredEvent,
  NameRenewed as NameRenewedEvent,
} from "./types/RONRegistrarController/RONRegistrarController";
import {
  Domain,
  NameRegistered,
  NameRenewed,
  Registration,
} from "./types/schema";
import { createEventID, createOrLoadAccount, isValidLabel, nodeId } from "./utils";

// Both events come after RNSUnified's own RecordUpdated in the same
// transaction (RONRegistrarController._register mints first, renew() calls
// RNSUnified.renew first), so the domain is already in the store. Registration
// ids are domain ids.

export function handleNameRegistered(event: NameRegisteredEvent): void {
  let id = nodeId(event.params.id);
  if (Domain.load(id) == null) return;

  let registrant = createOrLoadAccount(event.params.owner.toHexString());
  let registration = new Registration(id);
  registration.domain = id;
  registration.registrationDate = event.block.timestamp;
  registration.expiryDate = event.params.expires;
  registration.cost = event.params.ronPrice;
  registration.registrant = registrant.id;
  let label = event.params.name;
  registration.labelName = isValidLabel(label) ? label : null;
  registration.save();

  let registrationEvent = new NameRegistered(createEventID(event));
  registrationEvent.registration = registration.id;
  registrationEvent.blockNumber = event.block.number.toI32();
  registrationEvent.transactionID = event.transaction.hash;
  registrationEvent.registrant = registrant.id;
  registrationEvent.expiryDate = event.params.expires;
  registrationEvent.save();
}

export function handleNameRenewed(event: NameRenewedEvent): void {
  let id = nodeId(event.params.id);
  let domain = Domain.load(id);
  if (domain == null) return;

  let registration = Registration.load(id);
  if (registration == null) {
    // Registered before the L2 genesis, so no NameRegistered was indexed.
    registration = new Registration(id);
    registration.domain = id;
    registration.registrationDate = domain.createdAt;
    registration.registrant = domain.owner;
    let label = event.params.name;
    registration.labelName = isValidLabel(label) ? label : null;
  }
  registration.expiryDate = event.params.expires;
  registration.cost = event.params.cost;
  registration.save();

  let registrationEvent = new NameRenewed(createEventID(event));
  registrationEvent.registration = registration.id;
  registrationEvent.blockNumber = event.block.number.toI32();
  registrationEvent.transactionID = event.transaction.hash;
  registrationEvent.expiryDate = event.params.expires;
  registrationEvent.save();
}
