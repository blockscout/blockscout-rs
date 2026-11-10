import { ByteArray, Bytes, crypto, store } from "@graphprotocol/graph-ts";

import { NameForAddrChanged as NameForAddrChangedEvent } from "./types/DefaultReverseRegistrar/DefaultReverseRegistrar";
import { ReverseClaimed as ReverseClaimedEvent } from "./types/ReverseRegistrar/ReverseRegistrar";
import {
  Domain,
  PrimaryNameClaim,
  PrimaryNameRecord,
  Resolver,
  ReverseClaim,
} from "./types/schema";
import { concat, ROOT_NODE, byteArrayFromHex } from "./utils";

// ENS namehash of a dot-separated name (eg. "alice.etn")
function namehash(name: string): string {
  let node = byteArrayFromHex(ROOT_NODE.slice(2));
  if (name.length > 0) {
    let labels = name.split(".");
    for (let i = labels.length - 1; i >= 0; i--) {
      let labelhash = crypto.keccak256(Bytes.fromUTF8(labels[i]));
      node = crypto.keccak256(concat(node, labelhash));
    }
  }
  return node.toHexString();
}

// Empty names clear the record; names with NUL bytes cannot be stored.
function isUsableName(name: string | null): boolean {
  if (name === null) return false;
  return name.length > 0 && name.indexOf("\u0000") == -1;
}

function clearPrimaryNameRecord(address: string): void {
  if (PrimaryNameRecord.load(address) != null) {
    store.remove("PrimaryNameRecord", address);
  }
}

// Rebuilds the primary-name record BENS reads via its `primary_name_record`
// address-resolve technique from the names claimed for `address`.
//
// ENSIP-19 precedence: the chain-specific (classic `<addr>.addr.reverse`)
// name wins when set, otherwise the default reverse registrar name is used.
// Only a name whose indexed domain forward-resolves to the claiming address
// is recorded (ENSIP-19 requires clients to verify the forward match; doing
// it at index time keeps spoofed claims out of the table). The claim itself
// is kept regardless, so the record can be (re)built when the forward
// record catches up later -- users routinely set the primary name before
// the addr record, or move the addr record around afterwards.
export function refreshPrimaryNameRecord(address: string): void {
  const claim = PrimaryNameClaim.load(address);
  if (claim == null) {
    clearPrimaryNameRecord(address);
    return;
  }

  let name: string | null = null;
  if (isUsableName(claim.name)) {
    name = claim.name;
  } else if (isUsableName(claim.defaultName)) {
    name = claim.defaultName;
  }
  if (name === null) {
    clearPrimaryNameRecord(address);
    return;
  }

  const domainId = namehash(name!);
  const domain = Domain.load(domainId);
  if (domain == null || domain.resolvedAddress != address) {
    clearPrimaryNameRecord(address);
    return;
  }

  let record = PrimaryNameRecord.load(address);
  if (record == null) {
    record = new PrimaryNameRecord(address);
  }
  record.resolved_address = address;
  record.domain_id = domainId;
  record.domain_name = name!;
  record.save();
}

function loadOrCreateClaim(address: string): PrimaryNameClaim {
  let claim = PrimaryNameClaim.load(address);
  if (claim == null) {
    claim = new PrimaryNameClaim(address);
  }
  return claim;
}

// Re-reads the classic primary name of a claimed `<addr>.addr.reverse` node
// from the resolver currently set on that node and rebuilds the address's
// primary-name record.
//
// Resolver events are indexed from any contract (the Resolver data source has
// no fixed address), so the name is only ever taken from the resolver the
// reverse node actually points to; a NameChanged emitted by any other contract
// is ignored. Called whenever the name, the node's resolver or the claim
// changes, so switching resolvers picks up the new resolver's name (or clears
// it) right away.
export function syncClassicPrimaryName(node: string): void {
  let reverseClaim = ReverseClaim.load(node);
  if (reverseClaim == null) return;

  let name: string | null = null;
  let domain = Domain.load(node);
  if (domain != null && domain.resolver != null) {
    let resolver = Resolver.load(domain.resolver!);
    if (resolver != null) {
      name = resolver.name;
    }
  }

  let claim = loadOrCreateClaim(reverseClaim.address);
  claim.name = name;
  claim.save();
  refreshPrimaryNameRecord(reverseClaim.address);
}

// Records the ENSIP-19 default reverse registrar name claimed for an address
// and rebuilds its primary-name record.
//
// Anyone can set an arbitrary string as their default name, and Postgres
// rejects NUL bytes in text columns, so storing such a name would halt
// indexing. It could never forward-resolve anyway, so it clears the claim.
export function setDefaultPrimaryNameRecord(
  address: string,
  name: string,
): void {
  let claim = loadOrCreateClaim(address);
  claim.defaultName = name.indexOf("\u0000") == -1 ? name : null;
  claim.save();
  refreshPrimaryNameRecord(address);
}

// Called whenever a domain's forward-resolved address changes so the
// primary-name records of both the previous and the new address are
// re-verified against the forward record.
export function onResolvedAddressChanged(
  previous: string | null,
  current: string | null,
): void {
  if (previous !== null && previous != current) {
    refreshPrimaryNameRecord(previous);
  }
  if (current !== null) {
    refreshPrimaryNameRecord(current);
  }
}

// ENSIP-19 default reverse registrar: the event carries address and name
// directly.
export function handleNameForAddrChanged(event: NameForAddrChangedEvent): void {
  setDefaultPrimaryNameRecord(
    event.params.addr.toHexString(),
    event.params.name,
  );
}

// Classic reverse registrar: ReverseClaimed maps <addr>.addr.reverse nodes to
// their address; the name itself lives in the name() record of the node's
// resolver (see syncClassicPrimaryName). The resolver may already hold a name
// at claim time, so sync immediately.
export function handleReverseClaimed(event: ReverseClaimedEvent): void {
  let node = event.params.node.toHexString();
  let claim = new ReverseClaim(node);
  claim.address = event.params.addr.toHexString();
  claim.save();
  syncClassicPrimaryName(node);
}
