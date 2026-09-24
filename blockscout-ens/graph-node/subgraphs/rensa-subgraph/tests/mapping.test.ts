import { Address, BigInt, Bytes, crypto, ethereum } from "@graphprotocol/graph-ts";
import { assert, clearStore, createMockedFunction, newMockEvent, test } from "matchstick-as/assembly/index";
import {
  FreeNameClaimed, NameExpiredFinalized, NameRegistered, NameRenewed, NameTransferred,
  PrimaryNameCleared, PrimaryNameSet, ResolvedAddressUpdated, Transfer,
} from "../generated/RNSRegistryV2/RNSRegistryV2";
import {
  handleFreeNameClaimed, handleNameExpiredFinalized, handleNameRegistered, handleNameRenewed,
  handleNameTransferred, handlePrimaryNameCleared, handlePrimaryNameSet,
  handleResolvedAddressUpdated, handleTransfer,
} from "../src/mapping";

const REGISTRY = Address.fromString("0x08ed77b2ec313c7ad5ce23747b07d483071485f9");
const ZERO = Address.fromString("0x0000000000000000000000000000000000000000");
const ALICE = Address.fromString("0x1111111111111111111111111111111111111111");
const BOB = Address.fromString("0x2222222222222222222222222222222222222222");
const EXPIRY = BigInt.fromI32(2000000000);

function labelHash(label: string): Bytes {
  return changetype<Bytes>(crypto.keccak256(Bytes.fromUTF8(label)));
}

function node(parent: Bytes, label: string): Bytes {
  return changetype<Bytes>(crypto.keccak256(changetype<Bytes>(parent.concat(labelHash(label)))));
}

function rootId(): string {
  return node(Bytes.fromHexString("0x0000000000000000000000000000000000000000000000000000000000000000"), "rns").toHexString().toLowerCase();
}

function id(label: string): string {
  return node(Bytes.fromHexString(rootId()), label).toHexString().toLowerCase();
}

function tokenId(label: string): BigInt {
  const hash = labelHash(label);
  const little = new Bytes(32);
  for (let i = 0; i < 32; i++) little[i] = hash[31 - i];
  return BigInt.fromUnsignedBytes(little);
}

function mock<T>(params: Array<ethereum.EventParam>, index: i32): T {
  const event = newMockEvent();
  event.address = REGISTRY;
  event.parameters = params;
  event.logIndex = BigInt.fromI32(index);
  event.block.timestamp = BigInt.fromI32(1900000000);
  return changetype<T>(event);
}

function mint(label: string, owner: Address, index: i32): void {
  const tid = tokenId(label);
  createMockedFunction(REGISTRY, "labelOf", "labelOf(uint256):(string)")
    .withArgs([ethereum.Value.fromUnsignedBigInt(tid)])
    .returns([ethereum.Value.fromString(label)]);
  handleTransfer(mock<Transfer>([
    new ethereum.EventParam("from", ethereum.Value.fromAddress(ZERO)),
    new ethereum.EventParam("to", ethereum.Value.fromAddress(owner)),
    new ethereum.EventParam("tokenId", ethereum.Value.fromUnsignedBigInt(tid)),
  ], index));
}

function resolver(label: string, address: Address, index: i32): void {
  handleResolvedAddressUpdated(mock<ResolvedAddressUpdated>([
    new ethereum.EventParam("labelHash", ethereum.Value.fromFixedBytes(labelHash(label))),
    new ethereum.EventParam("label", ethereum.Value.fromString(label)),
    new ethereum.EventParam("resolvedAddress", ethereum.Value.fromAddress(address)),
  ], index));
}

function register(label: string, owner: Address, price: BigInt, index: i32): void {
  mint(label, owner, index);
  resolver(label, owner, index + 1);
  handleNameRegistered(mock<NameRegistered>([
    new ethereum.EventParam("labelHash", ethereum.Value.fromFixedBytes(labelHash(label))),
    new ethereum.EventParam("label", ethereum.Value.fromString(label)),
    new ethereum.EventParam("owner", ethereum.Value.fromAddress(owner)),
    new ethereum.EventParam("expiresAt", ethereum.Value.fromUnsignedBigInt(EXPIRY)),
    new ethereum.EventParam("pricePaid", ethereum.Value.fromUnsignedBigInt(price)),
  ], index + 2));
}

function setPrimary(label: string, owner: Address, index: i32): void {
  handlePrimaryNameSet(mock<PrimaryNameSet>([
    new ethereum.EventParam("owner", ethereum.Value.fromAddress(owner)),
    new ethereum.EventParam("labelHash", ethereum.Value.fromFixedBytes(labelHash(label))),
    new ethereum.EventParam("label", ethereum.Value.fromString(label)),
  ], index));
}

test("registration builds human-readable direct-child BENS record from ERC-721 mint", () => {
  clearStore();
  register("rensa", ALICE, BigInt.fromI32(50), 1);
  assert.fieldEquals("Domain", id("rensa"), "name", "rensa.rns");
  assert.fieldEquals("Domain", id("rensa"), "labelName", "rensa");
  assert.fieldEquals("Domain", id("rensa"), "parent", rootId());
  assert.fieldEquals("Domain", id("rensa"), "owner", ALICE.toHexString().toLowerCase());
  assert.fieldEquals("Domain", id("rensa"), "resolvedAddress", ALICE.toHexString().toLowerCase());
  assert.fieldEquals("Domain", id("rensa"), "expiryDate", EXPIRY.toString());
  assert.fieldEquals("Domain", id("rensa"), "tokenId", tokenId("rensa").toString());
  assert.fieldEquals("Domain", rootId(), "subdomainCount", "1");
  assert.fieldEquals("Registration", id("rensa"), "freeClaimed", "false");
  assert.entityCount("Domain", 2); // .rns root + one direct child; no invented subdomains.
});

test("free claim is an ordinary registration plus explicit free-claim audit event", () => {
  clearStore();
  register("claimable", ALICE, BigInt.zero(), 10);
  handleFreeNameClaimed(mock<FreeNameClaimed>([
    new ethereum.EventParam("labelHash", ethereum.Value.fromFixedBytes(labelHash("claimable"))),
    new ethereum.EventParam("label", ethereum.Value.fromString("claimable")),
    new ethereum.EventParam("claimant", ethereum.Value.fromAddress(ALICE)),
    new ethereum.EventParam("expiresAt", ethereum.Value.fromUnsignedBigInt(EXPIRY)),
    new ethereum.EventParam("freeClaimsUsed", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(1))),
  ], 13));
  assert.fieldEquals("Registration", id("claimable"), "freeClaimed", "true");
  assert.entityCount("FreeClaimIndexed", 1);
});

test("resolver update changes only the forward record, not ERC-721 ownership", () => {
  clearStore();
  register("resolve-me", ALICE, BigInt.fromI32(10), 15);
  resolver("resolve-me", BOB, 18);
  assert.fieldEquals("Domain", id("resolve-me"), "owner", ALICE.toHexString().toLowerCase());
  assert.fieldEquals("Domain", id("resolve-me"), "resolvedAddress", BOB.toHexString().toLowerCase());
  assert.entityCount("AddrChanged", 2);
});

test("transfer changes canonical owner, resets resolver, preserves expiry and invalidates old primary", () => {
  clearStore();
  register("transferable", ALICE, BigInt.fromI32(10), 20);
  setPrimary("transferable", ALICE, 23);
  assert.fieldEquals("PrimaryNameRecord", ALICE.toHexString().toLowerCase(), "domain_name", "transferable.rns");
  handleTransfer(mock<Transfer>([
    new ethereum.EventParam("from", ethereum.Value.fromAddress(ALICE)),
    new ethereum.EventParam("to", ethereum.Value.fromAddress(BOB)),
    new ethereum.EventParam("tokenId", ethereum.Value.fromUnsignedBigInt(tokenId("transferable"))),
  ], 24));
  resolver("transferable", BOB, 25);
  handleNameTransferred(mock<NameTransferred>([
    new ethereum.EventParam("labelHash", ethereum.Value.fromFixedBytes(labelHash("transferable"))),
    new ethereum.EventParam("label", ethereum.Value.fromString("transferable")),
    new ethereum.EventParam("previousOwner", ethereum.Value.fromAddress(ALICE)),
    new ethereum.EventParam("newOwner", ethereum.Value.fromAddress(BOB)),
  ], 26));
  assert.fieldEquals("Domain", id("transferable"), "owner", BOB.toHexString().toLowerCase());
  assert.fieldEquals("Domain", id("transferable"), "resolvedAddress", BOB.toHexString().toLowerCase());
  assert.fieldEquals("Domain", id("transferable"), "expiryDate", EXPIRY.toString());
  assert.notInStore("PrimaryNameRecord", ALICE.toHexString().toLowerCase());
  setPrimary("transferable", BOB, 27);
  assert.fieldEquals("PrimaryNameRecord", BOB.toHexString().toLowerCase(), "domain_id", id("transferable"));
});

test("renewal updates expiry and explicit primary clear removes reverse record", () => {
  clearStore();
  register("renewable", ALICE, BigInt.fromI32(10), 30);
  setPrimary("renewable", ALICE, 33);
  const later = EXPIRY.plus(BigInt.fromI32(31536000));
  handleNameRenewed(mock<NameRenewed>([
    new ethereum.EventParam("labelHash", ethereum.Value.fromFixedBytes(labelHash("renewable"))),
    new ethereum.EventParam("label", ethereum.Value.fromString("renewable")),
    new ethereum.EventParam("expiresAt", ethereum.Value.fromUnsignedBigInt(later)),
    new ethereum.EventParam("pricePaid", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(10))),
  ], 34));
  assert.fieldEquals("Domain", id("renewable"), "expiryDate", later.toString());
  assert.fieldEquals("Registration", id("renewable"), "expiryDate", later.toString());
  handlePrimaryNameCleared(mock<PrimaryNameCleared>([
    new ethereum.EventParam("owner", ethereum.Value.fromAddress(ALICE)),
    new ethereum.EventParam("labelHash", ethereum.Value.fromFixedBytes(labelHash("renewable"))),
    new ethereum.EventParam("label", ethereum.Value.fromString("renewable")),
  ], 35));
  assert.notInStore("PrimaryNameRecord", ALICE.toHexString().toLowerCase());
});

test("burn/finalize clears ownership and re-registration reuses deterministic NFT and domain IDs", () => {
  clearStore();
  register("reusable", ALICE, BigInt.fromI32(10), 40);
  setPrimary("reusable", ALICE, 43);
  handleTransfer(mock<Transfer>([
    new ethereum.EventParam("from", ethereum.Value.fromAddress(ALICE)),
    new ethereum.EventParam("to", ethereum.Value.fromAddress(ZERO)),
    new ethereum.EventParam("tokenId", ethereum.Value.fromUnsignedBigInt(tokenId("reusable"))),
  ], 44));
  resolver("reusable", ZERO, 45);
  handleNameExpiredFinalized(mock<NameExpiredFinalized>([
    new ethereum.EventParam("labelHash", ethereum.Value.fromFixedBytes(labelHash("reusable"))),
    new ethereum.EventParam("label", ethereum.Value.fromString("reusable")),
    new ethereum.EventParam("previousOwner", ethereum.Value.fromAddress(ALICE)),
    new ethereum.EventParam("expiresAt", ethereum.Value.fromUnsignedBigInt(EXPIRY)),
  ], 46));
  assert.fieldEquals("Domain", id("reusable"), "owner", ZERO.toHexString().toLowerCase());
  assert.notInStore("PrimaryNameRecord", ALICE.toHexString().toLowerCase());
  assert.entityCount("ExpiryFinalizedIndexed", 1);
  register("reusable", BOB, BigInt.fromI32(20), 47);
  assert.fieldEquals("Domain", id("reusable"), "owner", BOB.toHexString().toLowerCase());
  assert.fieldEquals("Domain", id("reusable"), "resolvedAddress", BOB.toHexString().toLowerCase());
  assert.fieldEquals("Domain", id("reusable"), "name", "reusable.rns");
  assert.fieldEquals("Domain", rootId(), "subdomainCount", "1");
  assert.entityCount("Domain", 2);
});
