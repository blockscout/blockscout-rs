import {
  MintedDomain as MintedDomainEvent,
  PrimaryDomainSet as PrimaryDomainSetEvent,
  RenewedDomain as RenewedDomainEvent,
  Transfer as TransferEvent,
} from "../generated/ZNSRegistry/ZNSRegistry";
import {
  Domain,
  Account,
  Transfer,
  Registration,
  ExpiryExtended,
} from "../generated/schema";
import { crypto, Bytes } from "@graphprotocol/graph-ts";

export function handleMintedDomain(event: MintedDomainEvent): void {
  let domain = new Domain(event.params.tokenId.toString());
  domain.name = event.params.domainName;
  domain.createdAt = event.block.timestamp;
  domain.expiryDate = event.params.expiry;
  domain.labelName = event.params.domainName;
  domain.labelhash = Bytes.fromByteArray(
    crypto.keccak256(Bytes.fromUTF8(domain.labelName!))
  );
  domain.resolvedAddress = event.params.owner.toHex();
  domain.registrant = event.params.owner.toHex();
  let ownerId = event.params.owner.toHex();
  let owner = Account.load(ownerId);
  if (!owner) {
    owner = new Account(ownerId);
  }
  owner.save();
  domain.owner = owner.id;
  domain.save();
  let mintedEvent = new Registration(event.params.tokenId.toString());
  mintedEvent.registrant = event.params.owner.toHex();
  mintedEvent.domain = domain.id;
  mintedEvent.registrationDate = event.block.timestamp;
  mintedEvent.expiryDate = event.params.expiry;
  mintedEvent.cost = event.transaction.value;
  mintedEvent.save();
}

export function handlePrimaryDomainSet(event: PrimaryDomainSetEvent): void {
  let domain = Domain.load(event.params.tokenId.toString());
  if (!domain) {
    return;
  }
  let ownerId = event.params.owner.toHex();
  let owner = Account.load(ownerId);
  if (!owner) {
    owner = new Account(ownerId);
  }
  domain.owner = owner.id;
  domain.save();
  owner.save();
}

export function handleRenewedDomain(event: RenewedDomainEvent): void {
  let domain = Domain.load(event.params.tokenId.toString());
  if (!domain) {
    return;
  }

  domain.expiryDate = event.params.expiry;
  domain.save();

  let expiryExtendedId = event.transaction.hash
    .toHexString()
    .concat("-")
    .concat(event.logIndex.toString());
  let expiryExtended = ExpiryExtended.load(expiryExtendedId);
  if (!expiryExtended) {
    expiryExtended = new ExpiryExtended(expiryExtendedId);
    expiryExtended.domain = domain.id;
    expiryExtended.blockNumber = event.block.number.toI32();
    expiryExtended.transactionID = event.transaction.hash;
    expiryExtended.expiryDate = event.params.expiry;
    expiryExtended.save();
  }
}

export function handleTransfer(event: TransferEvent): void {
  let domain = Domain.load(event.params.tokenId.toHex());
  if (!domain) {
    return;
  }
  let transfer = new Transfer(event.transaction.hash.toHex());
  transfer.domain = domain.id;
  transfer.owner = event.params.to.toHex();
  transfer.blockNumber = event.block.number.toI32();
  transfer.transactionID = event.transaction.hash;
  let toOwnerId = event.params.to.toHex();
  let toOwner = Account.load(toOwnerId);
  if (!toOwner) {
    toOwner = new Account(toOwnerId);
  }
  domain.owner = toOwner.id;
  transfer.save();
  domain.save();
  toOwner.save();
}
