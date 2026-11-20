// ETHRegistrar enrichment. NameRegistered/NameRenewed
// are emitted by ETHRegistrar, a different contract from ETHRegistry — the
// registry these events enrich is always the canonical ETHRegistry
// (getEthRegistryAddress()), never event.address (that's ETHRegistrar's own
// address). Easy to get backwards, worth this comment.
//
// ENSv2Registration.id = the same id ENSv2NameSlot uses for this tokenId
// (nameSlotId(ETHRegistry, toSlotId(tokenId))) rather than a new tx-based
// scheme — this is what makes correlating registry/registrar events by
// tokenId order-independent "for free": the id needs nothing from the
// other event, so it doesn't matter which arrives first — "correlate by
// transaction hash plus token ID, do not rely on log order."
import { log } from "@graphprotocol/graph-ts";
import {
  checkValidLabel,
  clampLegacyExpiry,
  createOrLoadAccount,
  ETH_NODE,
} from "./utils";
import { getEthRegistryAddress } from "./ensv2Constants";
import { nameSlotId, pathNamehash, toSlotId } from "./ensv2Utils";
import {
  ENSv2NamePath,
  ENSv2NameSlot,
  ENSv2Registration,
} from "./types/schema";
import {
  NameRegistered,
  NameRenewed,
  OwnershipTransferred,
} from "./types/ETHRegistrar/ETHRegistrar";
import { processOwnershipTransferred } from "./accessControl";

export function syncRegistrationFromSlot(slot: ENSv2NameSlot): void {
  if (!slot.registry.equals(getEthRegistryAddress())) {
    return;
  }
  let registration = ENSv2Registration.load(slot.id);
  if (registration == null) {
    return;
  }
  let expiryDate = slot.expiryDate;
  if (expiryDate) {
    registration.expiryDate = clampLegacyExpiry(expiryDate);
  }
  let resource = slot.currentResource;
  if (resource) {
    registration.resource = resource;
  }
  let path = ENSv2NamePath.load(pathNamehash(ETH_NODE, slot.labelhash));
  if (path != null) {
    registration.path = path.id;
  }
  registration.save();
}

export function handleNameRegistered(event: NameRegistered): void {
  let registryId = getEthRegistryAddress();
  let slotId = toSlotId(event.params.tokenId);
  let id = nameSlotId(registryId, slotId);

  let registration = ENSv2Registration.load(id);
  if (registration == null) {
    registration = new ENSv2Registration(id);
    registration.slot = id;
  }

  // This row is slot-keyed and therefore reused across incarnations. Every
  // NameRegistered begins a new incarnation, so registrationDate and all
  // purchase metadata must be replaced even when the entity already exists.
  registration.registrationDate = event.block.timestamp;
  let slot = ENSv2NameSlot.load(id);
  if (slot != null && slot.expiryDate) {
    registration.expiryDate = clampLegacyExpiry(slot.expiryDate!);
  } else {
    // The registry event later reconciles this fallback to the canonical
    // absolute expiry through syncRegistrationFromSlot.
    registration.expiryDate = event.block.timestamp.plus(event.params.duration);
  }

  if (checkValidLabel(event.params.label)) {
    registration.label = event.params.label;
  }
  registration.owner = createOrLoadAccount(event.params.owner).id;
  registration.duration = event.params.duration;
  registration.paymentToken = event.params.paymentToken;
  registration.referrer = event.params.referrer;
  registration.base = event.params.base;
  registration.premium = event.params.premium;
  registration.transactionID = event.transaction.hash;
  registration.logIndex = event.logIndex;
  registration.save();
  if (slot != null) {
    syncRegistrationFromSlot(slot);
  }
}

export function handleNameRenewed(event: NameRenewed): void {
  let registryId = getEthRegistryAddress();
  let slotId = toSlotId(event.params.tokenId);
  let id = nameSlotId(registryId, slotId);

  // Refresh enrichment only if a registration already exists — a renewal
  // without a prior registration is nonsensical, so never create one here.
  let registration = ENSv2Registration.load(id);
  if (registration == null) {
    log.warning("handleNameRenewed: no ENSv2Registration for {}, skipping", [
      id.toHexString(),
    ]);
    return;
  }
  registration.duration = event.params.duration;
  registration.paymentToken = event.params.paymentToken;
  registration.referrer = event.params.referrer;
  // newExpiry is the contract's absolute expiry, not a duration delta.
  registration.expiryDate = clampLegacyExpiry(event.params.newExpiry);
  registration.save();
}

export function handleETHRegistrarOwnershipTransferred(
  event: OwnershipTransferred
): void {
  processOwnershipTransferred(event.address, event.params.newOwner, event.block);
}
