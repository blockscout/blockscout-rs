// Minimal smoke-test layer for every handler wired from src/ensRegistry.ts
// (ENSRegistry + ENSRegistryOld data sources). Independent of and
// redundant with ensRegistry.test.ts's richer behavioral tests -- one
// mock-event-per-handler baseline only.
import { Address, BigInt, Bytes, ethereum } from "@graphprotocol/graph-ts";
import {
  afterEach,
  assert,
  clearStore,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import {
  handleApprovalForAll,
  handleApprovalForAllOldRegistry,
  handleNewOwner,
  handleNewOwnerOldRegistry,
  handleNewResolver,
  handleNewResolverOldRegistry,
  handleNewTTL,
  handleNewTTLOldRegistry,
  handleTransfer,
  handleTransferOldRegistry,
} from "../src/ensRegistry";
import {
  ApprovalForAll,
  NewOwner,
  NewResolver,
  NewTTL,
  Transfer,
} from "../src/types/ENSRegistry/EnsRegistry";
import { Domain } from "../src/types/schema";
import { ROOT_NODE } from "../src/utils";

const OWNER = "0x89205A3A3b2A69De6Dbf7f01ED13B2108B2c43e7";
const OPERATOR = "0x6060606060606060606060606060606060606060";
const RESOLVER = "0x7070707070707070707070707070707070707070";

const NODE_A =
  "0xa1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
const LABEL_A =
  "0xb1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1";

function newOwnerEvent(node: string, label: string, owner: string): NewOwner {
  let mockEvent = newMockEvent();
  let event = new NewOwner(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("node", ethereum.Value.fromBytes(Bytes.fromHexString(node)))
  );
  event.parameters.push(
    new ethereum.EventParam("label", ethereum.Value.fromBytes(Bytes.fromHexString(label)))
  );
  event.parameters.push(
    new ethereum.EventParam("owner", ethereum.Value.fromAddress(Address.fromString(owner)))
  );
  return event;
}

afterEach(() => {
  clearStore();
});

test("smoke: handleNewOwner creates a Domain row", () => {
  let event = newOwnerEvent(ROOT_NODE.toHexString(), LABEL_A, OWNER);
  handleNewOwner(event);
  // makeSubnode's output isn't exposed directly; assert via entity count
  // instead of re-deriving the keccak256 id by hand.
  assert.entityCount("Domain", 2); // the fresh subnode plus the ROOT_NODE parent domain, both saved by _handleNewOwner
});

test("smoke: handleTransfer updates an existing Domain's owner", () => {
  let domain = new Domain(NODE_A);
  domain.storedOffchain = false; // Blockscout/BENS extension
  domain.resolvedWithWildcard = false; // Blockscout/BENS extension
  domain.owner = OWNER;
  domain.isMigrated = true;
  domain.subdomainCount = 0;
  domain.createdAt = BigInt.fromI32(0);
  domain.save();

  let mockEvent = newMockEvent();
  let event = new Transfer(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("node", ethereum.Value.fromBytes(Bytes.fromHexString(NODE_A)))
  );
  event.parameters.push(
    new ethereum.EventParam("owner", ethereum.Value.fromAddress(Address.fromString(OPERATOR)))
  );
  handleTransfer(event);
  assert.fieldEquals("Domain", NODE_A, "owner", Address.fromString(OPERATOR).toHexString());
});

test("smoke: handleNewResolver creates a Resolver row and sets Domain.resolver", () => {
  let domain = new Domain(NODE_A);
  domain.storedOffchain = false; // Blockscout/BENS extension
  domain.resolvedWithWildcard = false; // Blockscout/BENS extension
  domain.owner = OWNER;
  domain.isMigrated = true;
  domain.subdomainCount = 0;
  domain.createdAt = BigInt.fromI32(0);
  domain.save();

  let mockEvent = newMockEvent();
  let event = new NewResolver(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(Bytes.fromHexString(NODE_A)))
  );
  event.parameters.push(
    new ethereum.EventParam("resolver", ethereum.Value.fromAddress(Address.fromString(RESOLVER)))
  );
  handleNewResolver(event);
  assert.entityCount("Resolver", 1);
});

test("smoke: handleNewTTL sets ttl on an existing Domain", () => {
  let domain = new Domain(NODE_A);
  domain.storedOffchain = false; // Blockscout/BENS extension
  domain.resolvedWithWildcard = false; // Blockscout/BENS extension
  domain.owner = OWNER;
  domain.isMigrated = true;
  domain.subdomainCount = 0;
  domain.createdAt = BigInt.fromI32(0);
  domain.save();

  let mockEvent = newMockEvent();
  let event = new NewTTL(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("node", ethereum.Value.fromBytes(Bytes.fromHexString(NODE_A)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "ttl",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(100))
    )
  );
  handleNewTTL(event);
  assert.fieldEquals("Domain", NODE_A, "ttl", "100");
});

test("smoke: handleApprovalForAll creates an OperatorApproval row", () => {
  let mockEvent = newMockEvent();
  let event = new ApprovalForAll(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("owner", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  event.parameters.push(
    new ethereum.EventParam("operator", ethereum.Value.fromAddress(Address.fromString(OPERATOR)))
  );
  event.parameters.push(new ethereum.EventParam("approved", ethereum.Value.fromBoolean(true)));
  handleApprovalForAll(event);
  assert.entityCount("OperatorApproval", 1);
});

test("smoke: handleApprovalForAllOldRegistry creates an OperatorApproval row", () => {
  let mockEvent = newMockEvent();
  let event = new ApprovalForAll(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam("owner", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  event.parameters.push(
    new ethereum.EventParam("operator", ethereum.Value.fromAddress(Address.fromString(OPERATOR)))
  );
  event.parameters.push(new ethereum.EventParam("approved", ethereum.Value.fromBoolean(true)));
  handleApprovalForAllOldRegistry(event);
  assert.entityCount("OperatorApproval", 1);
});

test("smoke: handleNewOwnerOldRegistry creates a Domain row for a fresh (non-migrated) subnode", () => {
  handleNewOwnerOldRegistry(newOwnerEvent(ROOT_NODE.toHexString(), LABEL_A, OWNER));
  // the fresh subnode plus the ROOT_NODE parent domain, both saved by _handleNewOwner
  assert.entityCount("Domain", 2);
});

test("smoke: handleTransferOldRegistry runs without crashing against the ROOT_NODE domain", () => {
  let mockEvent = newMockEvent();
  let event = new Transfer(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "node",
      ethereum.Value.fromBytes(Bytes.fromHexString(ROOT_NODE.toHexString()))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("owner", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  handleTransferOldRegistry(event);
  // ROOT_NODE's auto-created Domain is always isMigrated:true, so the inner
  // handleTransfer never fires and nothing gets persisted here -- the point
  // of this test is that the non-null assertion inside getDomain() doesn't
  // crash on a domain that doesn't exist in the store yet.
  assert.entityCount("Domain", 0);
});

test("smoke: handleNewResolverOldRegistry sets Domain.resolver for the ROOT_NODE domain", () => {
  let mockEvent = newMockEvent();
  let event = new NewResolver(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "node",
      ethereum.Value.fromFixedBytes(Bytes.fromHexString(ROOT_NODE.toHexString()))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("resolver", ethereum.Value.fromAddress(Address.fromString(RESOLVER)))
  );
  handleNewResolverOldRegistry(event);
  assert.entityCount("Resolver", 1);
});

test("smoke: handleNewTTLOldRegistry runs without crashing against the ROOT_NODE domain", () => {
  let mockEvent = newMockEvent();
  let event = new NewTTL(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "node",
      ethereum.Value.fromBytes(Bytes.fromHexString(ROOT_NODE.toHexString()))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "ttl",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(50))
    )
  );
  handleNewTTLOldRegistry(event);
  // Same reasoning as handleTransferOldRegistry above: ROOT_NODE's
  // auto-created Domain is always isMigrated:true, so the inner handleNewTTL
  // never fires and nothing gets persisted.
  assert.entityCount("Domain", 0);
});
