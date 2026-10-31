// Import types and APIs from graph-ts
import { BigInt, ByteArray, Bytes, store } from "@graphprotocol/graph-ts";
// Import event types from the registry contract ABI
import {
  ApprovalForAll as ApprovalForAllEvent,
  ControllerChanged as ControllerChangedEvent,
  ExpiryExtended as ExpiryExtendedEvent,
  FusesSet as FusesSetEvent,
  NameUnwrapped as NameUnwrappedEvent,
  NameWrapped as NameWrappedEvent,
  OwnershipTransferred as OwnershipTransferredEvent,
  TransferBatch as TransferBatchEvent,
  TransferSingle as TransferSingleEvent,
} from "./types/NameWrapper/NameWrapper";
import {
  processApprovalForAll,
  processControllerStatus,
  processOwnershipTransferred,
} from "./accessControl";
// Import entity types generated from the GraphQL schema
import {
  ExpiryExtended,
  FusesSet,
  NameUnwrapped,
  NameWrapped,
  WrappedDomain,
  WrappedTransfer,
} from "./types/schema";
import {
  checkValidLabel,
  clampLegacyExpiry,
  concat,
  createLegacyEventID,
  createOrLoadAccount,
  createOrLoadDomain,
  EMPTY_ADDRESS_BYTEARRAY,
  ETH_NODE,
  uint256ToByteArray,
} from "./utils";

export function decodeName(buf: Bytes): Array<string> | null {
  let offset = 0;
  let list = new ByteArray(0);
  let dot = Bytes.fromHexString("2e");
  let len = buf[offset++];
  let hex = buf.toHexString();
  let firstLabel = "";
  if (len == 0) {
    return [firstLabel, "."];
  }

  while (len) {
    let label = hex.slice((offset + 1) * 2, (offset + 1 + len) * 2);
    let labelBytes = Bytes.fromHexString(label);

    if (!checkValidLabel(labelBytes.toString())) {
      return null;
    }

    if (offset > 1) {
      list = concat(list, dot);
    } else {
      firstLabel = labelBytes.toString();
    }
    list = concat(list, labelBytes);
    offset += len;
    len = buf[offset++];
  }
  return [firstLabel, list.toString()];
}

const PARENT_CANNOT_CONTROL: i32 = 65536;

function checkPccBurned(fuses: i32): boolean {
  return (fuses & PARENT_CANNOT_CONTROL) == PARENT_CANNOT_CONTROL;
}

export function handleNameWrapped(event: NameWrappedEvent): void {
  let decoded = decodeName(event.params.name);
  let label: string | null = null;
  let name: string | null = null;
  if (decoded != null) {
    label = decoded[0];
    name = decoded[1];
  }
  let node = event.params.node;
  let expiryDate = event.params.expiry;
  // Legacy entity fields hold the clamped value; the NameWrapped event row
  // below keeps the raw on-chain expiry.
  let legacyExpiryDate = clampLegacyExpiry(expiryDate);
  let fuses = event.params.fuses.toI32();
  let blockNumber = event.block.number.toI32();
  let transactionID = event.transaction.hash;
  let owner = createOrLoadAccount(event.params.owner);
  let domain = createOrLoadDomain(node);

  if (!domain.labelName && label) {
    domain.labelName = label;
    domain.name = name;
  }
  if (
    checkPccBurned(fuses) &&
    (!domain.expiryDate || legacyExpiryDate > domain.expiryDate!)
  ) {
    domain.expiryDate = legacyExpiryDate;
  }
  domain.wrappedOwner = owner.id;
  domain.save();

  let wrappedDomain = new WrappedDomain(node.toHexString());
  wrappedDomain.domain = domain.id;
  wrappedDomain.expiryDate = legacyExpiryDate;
  wrappedDomain.fuses = fuses;
  wrappedDomain.owner = owner.id;
  wrappedDomain.name = name;
  wrappedDomain.save();

  let nameWrappedEvent = new NameWrapped(createLegacyEventID(event));
  nameWrappedEvent.domain = domain.id;
  nameWrappedEvent.name = name;
  nameWrappedEvent.fuses = fuses;
  nameWrappedEvent.expiryDate = expiryDate;
  nameWrappedEvent.owner = owner.id;
  nameWrappedEvent.blockNumber = blockNumber;
  nameWrappedEvent.transactionID = transactionID;
  nameWrappedEvent.save();
}

export function handleNameUnwrapped(event: NameUnwrappedEvent): void {
  let node = event.params.node;
  let blockNumber = event.block.number.toI32();
  let transactionID = event.transaction.hash;
  let owner = createOrLoadAccount(event.params.owner);

  let domain = createOrLoadDomain(node);
  domain.wrappedOwner = null;
  // Nullable-Bytes comparison against a real value (not null) hits the same
  // compileBinaryOverload crash as comparing against null (AssemblyScript
  // compiler gotcha) — isolate the .equals() call behind a
  // truthy guard and assign the boolean to a local first, never inline.
  let parentIsEth = false;
  if (domain.parent) {
    parentIsEth = domain.parent! == ETH_NODE.toHexString();
  }
  if (domain.expiryDate && !parentIsEth) {
    domain.expiryDate = null;
  }
  domain.save();

  let nameUnwrappedEvent = new NameUnwrapped(createLegacyEventID(event));
  nameUnwrappedEvent.domain = node.toHexString();
  nameUnwrappedEvent.owner = owner.id;
  nameUnwrappedEvent.blockNumber = blockNumber;
  nameUnwrappedEvent.transactionID = transactionID;
  nameUnwrappedEvent.save();

  // Already gone when the registry Transfer that precedes NameUnwrapped
  // detached the wrapped state (ensRegistry.detachWrappedStateIfLeftWrapper).
  if (WrappedDomain.load(node.toHexString()) != null) {
    store.remove("WrappedDomain", node.toHexString());
  }
}

export function handleFusesSet(event: FusesSetEvent): void {
  let node = event.params.node;
  let fuses = event.params.fuses;
  let blockNumber = event.block.number.toI32();
  let transactionID = event.transaction.hash;
  let wrappedDomain = WrappedDomain.load(node.toHexString());
  if (wrappedDomain) {
    wrappedDomain.fuses = fuses.toI32();
    wrappedDomain.save();
    if (wrappedDomain.expiryDate && checkPccBurned(wrappedDomain.fuses)) {
      let domain = createOrLoadDomain(node);
      if (!domain.expiryDate || wrappedDomain.expiryDate > domain.expiryDate!) {
        domain.expiryDate = wrappedDomain.expiryDate;
        domain.save();
      }
    }
  }
  let fusesBurnedEvent = new FusesSet(createLegacyEventID(event));
  fusesBurnedEvent.domain = node.toHexString();
  fusesBurnedEvent.fuses = fuses.toI32();
  fusesBurnedEvent.blockNumber = blockNumber;
  fusesBurnedEvent.transactionID = transactionID;
  fusesBurnedEvent.save();
}

export function handleExpiryExtended(event: ExpiryExtendedEvent): void {
  let node = event.params.node;
  let expiry = event.params.expiry;
  // Graveyard-parked migrated names are extended to uint64 max; legacy
  // entity fields hold the clamped value, the event row keeps the raw one.
  let legacyExpiry = clampLegacyExpiry(expiry);
  let blockNumber = event.block.number.toI32();
  let transactionID = event.transaction.hash;
  let wrappedDomain = WrappedDomain.load(node.toHexString());
  if (wrappedDomain) {
    wrappedDomain.expiryDate = legacyExpiry;
    wrappedDomain.save();
    if (checkPccBurned(wrappedDomain.fuses)) {
      let domain = createOrLoadDomain(node);
      if (!domain.expiryDate || legacyExpiry > domain.expiryDate!) {
        domain.expiryDate = legacyExpiry;
        domain.save();
      }
    }
  }
  let expiryExtendedEvent = new ExpiryExtended(createLegacyEventID(event));
  expiryExtendedEvent.domain = node.toHexString();
  expiryExtendedEvent.expiryDate = expiry;
  expiryExtendedEvent.blockNumber = blockNumber;
  expiryExtendedEvent.transactionID = transactionID;
  expiryExtendedEvent.save();
}

function makeWrappedTransfer(
  blockNumber: i32,
  transactionID: Bytes,
  eventID: string,
  node: BigInt,
  from: Bytes,
  to: Bytes
): void {
  const _to = createOrLoadAccount(to);
  // Reuses uint256ToByteArray instead of the old manual
  // "0x" + node.toHex().slice(2).padStart(64, "0") string surgery — same
  // 32-byte big-endian value, no reimplementation.
  const namehash = Bytes.fromByteArray(uint256ToByteArray(node));
  const domain = createOrLoadDomain(namehash);
  let wrappedDomain = WrappedDomain.load(namehash.toHexString());
  // A transfer (not a mint from 0x0) of a token whose name carries no wrapped
  // state moves a detached ERC-1155 token: the name left the NameWrapper
  // through the registry (see ensRegistry.detachWrappedStateIfLeftWrapper) or
  // was re-registered after expiry. Keep the history row, do not revive the
  // wrapped state.
  // (.equals() kept out of compound conditions: AssemblyScript's
  // compileBinaryOverload crashes on it, see handleNameUnwrapped.)
  let isMint = from.equals(EMPTY_ADDRESS_BYTEARRAY);
  if (!isMint && wrappedDomain == null) {
    if (!domain.wrappedOwner) {
      saveWrappedTransfer(
        eventID,
        domain.id,
        blockNumber,
        transactionID,
        _to.id
      );
      return;
    }
  }
  // new registrations emit the Transfer` event before the NameWrapped event
  // so we need to create the WrappedDomain entity here
  if (wrappedDomain == null) {
    wrappedDomain = new WrappedDomain(namehash.toHexString());
    wrappedDomain.domain = domain.id;

    // placeholders until we get the NameWrapped event
    wrappedDomain.expiryDate = BigInt.fromI32(0);
    wrappedDomain.fuses = 0;
  }
  wrappedDomain.owner = _to.id;
  wrappedDomain.save();
  domain.wrappedOwner = _to.id;
  domain.save();
  saveWrappedTransfer(eventID, domain.id, blockNumber, transactionID, _to.id);
}

function saveWrappedTransfer(
  eventID: string,
  domainId: string,
  blockNumber: i32,
  transactionID: Bytes,
  ownerId: string
): void {
  const wrappedTransfer = new WrappedTransfer(eventID);
  wrappedTransfer.domain = domainId;
  wrappedTransfer.blockNumber = blockNumber;
  wrappedTransfer.transactionID = transactionID;
  wrappedTransfer.owner = ownerId;
  wrappedTransfer.save();
}

export function handleTransferSingle(event: TransferSingleEvent): void {
  makeWrappedTransfer(
    event.block.number.toI32(),
    event.transaction.hash,
    createLegacyEventID(event).concat("-0"),
    event.params.id,
    event.params.from,
    event.params.to
  );
}

export function handleTransferBatch(event: TransferBatchEvent): void {
  let blockNumber = event.block.number.toI32();
  let transactionID = event.transaction.hash;
  let ids = event.params.ids;
  let from = event.params.from;
  let to = event.params.to;
  for (let i = 0; i < ids.length; i++) {
    makeWrappedTransfer(
      blockNumber,
      transactionID,
      createLegacyEventID(event).concat("-").concat(i.toString()),
      ids[i],
      from,
      to
    );
  }
}

// NameWrapper's ApprovalForAll names the approving account `account`, not
// `owner` (unlike ENSRegistry/BaseRegistrar) — see src/accessControl.ts.
export function handleNameWrapperApprovalForAll(
  event: ApprovalForAllEvent
): void {
  processApprovalForAll(
    event.address,
    event.params.account,
    event.params.operator,
    event.params.approved,
    event.block
  );
}

export function handleNameWrapperOwnershipTransferred(
  event: OwnershipTransferredEvent
): void {
  processOwnershipTransferred(event.address, event.params.newOwner, event.block);
}

export function handleControllerChanged(event: ControllerChangedEvent): void {
  processControllerStatus(
    event.address,
    event.params.controller,
    event.params.active,
    event.block
  );
}
