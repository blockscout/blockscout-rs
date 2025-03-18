import {
  MintedDomain as MintedDomainEvent,
  RenewedDomain as RenewedDomainEvent,
  TransferredDomain as TransferEvent,
} from "../generated/ZNSRegistry/ZNSRegistry";
import {
  Domain,
  Account,
  Transfer,
  Registration,
  ExpiryExtended,
} from "../generated/schema";
import { crypto, Bytes } from "@graphprotocol/graph-ts";
import {domainNameIsCorrect, hashByName} from "./utils";

export function handleMintedDomain(event: MintedDomainEvent): void {
  // let domain = new Domain(hashByName(event.params.domainName).toHex());
  let domain = Domain.load(hashByName(event.params.domainName).toHex());
  if (!domain) {
    domain = new Domain(hashByName(event.params.domainName).toHex());
  }
  domain.name = event.params.domainName;
  domain.createdAt = event.block.timestamp;
  domain.expiryDate = event.params.expiry;
  domain.labelName = event.params.domainName.split(".")[0];
  domain.labelhash = Bytes.fromByteArray(
    crypto.keccak256(Bytes.fromUTF8(domain.labelName!))
  );
  domain.resolvedAddress = event.params.owner.toHex();
  domain.registrant = event.params.owner.toHex();
  domain.subdomainCount = 0;
  domain.isMigrated = true;

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

export function handleRenewedDomain(event: RenewedDomainEvent): void {
  let domain = Domain.load(hashByName(event.params.domainName).toHex());
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
  if (!domainNameIsCorrect(event.params.domainName)) {
    return;
  }
  let domain = Domain.load(hashByName(event.params.domainName).toHex());
  if (!domain) {
    domain = new Domain(hashByName(event.params.domainName).toHex());
    domain.name = event.params.domainName;
    domain.createdAt = event.block.timestamp;
    domain.labelName = event.params.domainName.split(".")[0];
    domain.labelhash = Bytes.fromByteArray(
      crypto.keccak256(Bytes.fromUTF8(domain.labelName!))
    );
    domain.subdomainCount = 0;
    domain.isMigrated = true;
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
  domain.resolvedAddress = toOwner.id;
  transfer.save();
  domain.save();
  toOwner.save();
}
