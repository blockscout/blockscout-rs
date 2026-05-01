import { BigInt, Bytes } from "@graphprotocol/graph-ts";
import {
  Transfer as TransferEvent,
} from "../generated/ArcRegistrar/BaseRegistrar";
import {
  Domain,
  Account,
  Registration,
  Transfer as TransferEntity,
  NameTransferred,
} from "../generated/schema";
import { getOrCreateAccount, tokenIdToLabelhashHex } from "./utils";

// ─── ERC-721 Transfer handler ─────────────────────────────────────────────────
//
// BaseRegistrar.Transfer(from, to, tokenId) fires on every ERC-721 transfer.
// tokenId == uint256(keccak256(label)) — the labelhash as a uint256.
//
// We look up the domain via the Registration entity (keyed by labelhash hex),
// which was written at registration time by the Controller handler.
//
// Mints (from == zero address) are skipped — the Controller NameRegistered
// handler already sets the initial owner and registrant on the Domain entity.

const ZERO_ADDRESS = "0x0000000000000000000000000000000000000000";

function handleTransfer(event: TransferEvent, tld: string): void {
  let from = event.params.from.toHexString().toLowerCase();

  // Skip mints — already handled by Controller NameRegistered
  if (from == ZERO_ADDRESS) return;

  // Derive labelhash hex from tokenId
  let labelhashHex = tokenIdToLabelhashHex(event.params.tokenId);

  // Look up domain via Registration (keyed by labelhash hex)
  let reg = Registration.load(labelhashHex);
  if (!reg) return; // not registered through our controller — skip

  let nodeHex = reg.domain;
  let domain = Domain.load(nodeHex);
  if (!domain) return;

  let newOwnerAccount = getOrCreateAccount(event.params.to);

  // Update both owner (registry-level) and registrant (ERC-721 owner)
  domain.owner = newOwnerAccount.id;
  domain.registrant = newOwnerAccount.id;
  domain.save();

  // Transfer domain event
  let transferId =
    event.transaction.hash.toHexString() +
    "-" +
    event.logIndex.toString() +
    "-transfer";
  let transferEvent = new TransferEntity(transferId);
  transferEvent.domain = nodeHex;
  transferEvent.owner = newOwnerAccount.id;
  transferEvent.blockNumber = event.block.number.toI32();
  transferEvent.transactionID = event.transaction.hash;
  transferEvent.save();

  // NameTransferred registration event
  let nameTransferredId =
    event.transaction.hash.toHexString() +
    "-" +
    event.logIndex.toString() +
    "-nametransferred";
  let nameTransferredEvent = new NameTransferred(nameTransferredId);
  nameTransferredEvent.registration = labelhashHex;
  nameTransferredEvent.newOwner = newOwnerAccount.id;
  nameTransferredEvent.blockNumber = event.block.number.toI32();
  nameTransferredEvent.transactionID = event.transaction.hash;
  nameTransferredEvent.save();
}

// ─── Arc handlers ─────────────────────────────────────────────────────────────

export function handleArcTransfer(event: TransferEvent): void {
  handleTransfer(event, "arc");
}

// ─── Circle handlers ──────────────────────────────────────────────────────────

export function handleCircleTransfer(event: TransferEvent): void {
  handleTransfer(event, "circle");
}
