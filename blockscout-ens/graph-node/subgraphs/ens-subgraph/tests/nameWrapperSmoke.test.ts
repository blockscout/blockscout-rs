// Minimal smoke-test layer for every handler wired from src/nameWrapper.ts
// (NameWrapper data source). Independent of and redundant with
// nameWrapper.test.ts's richer behavioral tests -- one mock-event-per-handler
// baseline only.
import { Address, BigInt, Bytes, ethereum } from "@graphprotocol/graph-ts";
import {
  afterEach,
  assert,
  beforeAll,
  beforeEach,
  clearStore,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import {
  handleControllerChanged,
  handleExpiryExtended,
  handleFusesSet,
  handleNameUnwrapped,
  handleNameWrapped,
  handleNameWrapperApprovalForAll,
  handleNameWrapperOwnershipTransferred,
  handleTransferBatch,
  handleTransferSingle,
} from "../src/nameWrapper";
import {
  ApprovalForAll,
  ControllerChanged,
  ExpiryExtended,
  FusesSet,
  NameUnwrapped,
  NameWrapped,
  OwnershipTransferred,
  TransferBatch,
  TransferSingle,
} from "../src/types/NameWrapper/NameWrapper";
import { Domain } from "../src/types/schema";
import { setEthOwner } from "./testUtils";

const CONTRACT = "0xD4416b13d2b3a9aBae7AcD5D6C2BbDBE25686401";
const OWNER = "0x89205A3A3b2A69De6Dbf7f01ED13B2108B2c43e7";
const OTHER_OWNER = "0x1313131313131313131313131313131313131313";
const SMOKE_NAME_DNS_WIRE = "0x05736d6f6b650365746800";
const NODE =
  "0xe1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1";

beforeAll(() => {
  setEthOwner();
});

// createOrLoadDomain (src/utils.ts) creates a bare Domain and saves it
// immediately without setting the schema's non-nullable subdomainCount --
// never a problem in production or in the existing suite because a
// NameWrapper event always fires on a node that already passed through
// ENSRegistry's NewOwner first. Match that real ordering here rather than
// papering over the gap by pre-populating the field ourselves.
function preCreateDomainForNode(): void {
  let domain = new Domain(NODE);
  domain.storedOffchain = false; // Blockscout/BENS extension
  domain.resolvedWithWildcard = false; // Blockscout/BENS extension
  domain.owner = OWNER;
  domain.isMigrated = true;
  domain.subdomainCount = 0;
  domain.createdAt = BigInt.fromI32(0);
  domain.save();
}

beforeEach(() => {
  preCreateDomainForNode();
});

afterEach(() => {
  clearStore();
});

test("smoke: handleNameWrapped creates a WrappedDomain row", () => {
  let mockEvent = newMockEvent();
  let event = new NameWrapped(
    Address.fromString(CONTRACT),
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
    new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(Bytes.fromHexString(NODE)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "name",
      ethereum.Value.fromBytes(Bytes.fromHexString(SMOKE_NAME_DNS_WIRE))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("owner", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  event.parameters.push(
    new ethereum.EventParam("fuses", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(0)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "expiry",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2000000000))
    )
  );
  handleNameWrapped(event);
  assert.entityCount("WrappedDomain", 1);
});

test("smoke: handleNameUnwrapped clears WrappedDomain and removes the row", () => {
  handleNameWrapped(
    (function (): NameWrapped {
      let mockEvent = newMockEvent();
      let event = new NameWrapped(
        Address.fromString(CONTRACT),
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
        new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(Bytes.fromHexString(NODE)))
      );
      event.parameters.push(
        new ethereum.EventParam(
          "name",
          ethereum.Value.fromBytes(Bytes.fromHexString(SMOKE_NAME_DNS_WIRE))
        )
      );
      event.parameters.push(
        new ethereum.EventParam("owner", ethereum.Value.fromAddress(Address.fromString(OWNER)))
      );
      event.parameters.push(
        new ethereum.EventParam("fuses", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(0)))
      );
      event.parameters.push(
        new ethereum.EventParam(
          "expiry",
          ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2000000000))
        )
      );
      return event;
    })()
  );
  assert.entityCount("WrappedDomain", 1);

  let mockEvent = newMockEvent();
  let event = new NameUnwrapped(
    Address.fromString(CONTRACT),
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
    new ethereum.EventParam("node", ethereum.Value.fromBytes(Bytes.fromHexString(NODE)))
  );
  event.parameters.push(
    new ethereum.EventParam("owner", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  handleNameUnwrapped(event);
  assert.entityCount("WrappedDomain", 0);
});

test("smoke: handleFusesSet updates an existing WrappedDomain's fuses", () => {
  let wrapEvent = newMockEvent();
  let nameWrapped = new NameWrapped(
    Address.fromString(CONTRACT),
    wrapEvent.logIndex,
    wrapEvent.transactionLogIndex,
    wrapEvent.logType,
    wrapEvent.block,
    wrapEvent.transaction,
    wrapEvent.parameters,
    wrapEvent.receipt
  );
  nameWrapped.parameters = new Array();
  nameWrapped.parameters.push(
    new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(Bytes.fromHexString(NODE)))
  );
  nameWrapped.parameters.push(
    new ethereum.EventParam(
      "name",
      ethereum.Value.fromBytes(Bytes.fromHexString(SMOKE_NAME_DNS_WIRE))
    )
  );
  nameWrapped.parameters.push(
    new ethereum.EventParam("owner", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  nameWrapped.parameters.push(
    new ethereum.EventParam("fuses", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(0)))
  );
  nameWrapped.parameters.push(
    new ethereum.EventParam(
      "expiry",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2000000000))
    )
  );
  handleNameWrapped(nameWrapped);

  let mockEvent = newMockEvent();
  let event = new FusesSet(
    Address.fromString(CONTRACT),
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
    new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(Bytes.fromHexString(NODE)))
  );
  event.parameters.push(
    new ethereum.EventParam("fuses", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(65536)))
  );
  handleFusesSet(event);
  assert.fieldEquals("WrappedDomain", Bytes.fromHexString(NODE).toHexString(), "fuses", "65536");
});

test("smoke: handleExpiryExtended updates an existing WrappedDomain's expiryDate", () => {
  let wrapEvent = newMockEvent();
  let nameWrapped = new NameWrapped(
    Address.fromString(CONTRACT),
    wrapEvent.logIndex,
    wrapEvent.transactionLogIndex,
    wrapEvent.logType,
    wrapEvent.block,
    wrapEvent.transaction,
    wrapEvent.parameters,
    wrapEvent.receipt
  );
  nameWrapped.parameters = new Array();
  nameWrapped.parameters.push(
    new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(Bytes.fromHexString(NODE)))
  );
  nameWrapped.parameters.push(
    new ethereum.EventParam(
      "name",
      ethereum.Value.fromBytes(Bytes.fromHexString(SMOKE_NAME_DNS_WIRE))
    )
  );
  nameWrapped.parameters.push(
    new ethereum.EventParam("owner", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  nameWrapped.parameters.push(
    new ethereum.EventParam("fuses", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(0)))
  );
  nameWrapped.parameters.push(
    new ethereum.EventParam(
      "expiry",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(2000000000))
    )
  );
  handleNameWrapped(nameWrapped);

  let mockEvent = newMockEvent();
  let event = new ExpiryExtended(
    Address.fromString(CONTRACT),
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
    new ethereum.EventParam("node", ethereum.Value.fromFixedBytes(Bytes.fromHexString(NODE)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "expiry",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromString("2200000000"))
    )
  );
  handleExpiryExtended(event);
  assert.fieldEquals(
    "WrappedDomain",
    Bytes.fromHexString(NODE).toHexString(),
    "expiryDate",
    "2200000000"
  );
});

test("smoke: handleTransferSingle creates a WrappedDomain placeholder and a WrappedTransfer", () => {
  // makeWrappedTransfer derives its own namehash from the tokenId
  // (uint256ToByteArray(node)), not from the shared NODE constant -- use a
  // small tokenId and pre-create the Domain at its derived node, mirroring
  // nameWrapper.test.ts's own proven pattern for this exact handler.
  let tokenId = BigInt.fromI32(222333444);
  let node = "0x000000000000000000000000000000000000000000000000000000000d408a04";
  let domain = new Domain(node);
  domain.storedOffchain = false; // Blockscout/BENS extension
  domain.resolvedWithWildcard = false; // Blockscout/BENS extension
  domain.owner = OWNER;
  domain.isMigrated = true;
  domain.subdomainCount = 0;
  domain.createdAt = BigInt.fromI32(0);
  domain.save();

  let mockEvent = newMockEvent();
  let event = new TransferSingle(
    Address.fromString(CONTRACT),
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
    new ethereum.EventParam("operator", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  event.parameters.push(
    new ethereum.EventParam("from", ethereum.Value.fromAddress(Address.zero()))
  );
  event.parameters.push(
    new ethereum.EventParam("to", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  event.parameters.push(
    new ethereum.EventParam("id", ethereum.Value.fromUnsignedBigInt(tokenId))
  );
  event.parameters.push(
    new ethereum.EventParam("value", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(1)))
  );
  handleTransferSingle(event);
  assert.entityCount("WrappedTransfer", 1);
});

test("smoke: handleTransferBatch creates a WrappedTransfer per id in the batch", () => {
  let tokenId = BigInt.fromI32(333444555);
  let node = "0x0000000000000000000000000000000000000000000000000000000013dff5cb";
  let domain = new Domain(node);
  domain.storedOffchain = false; // Blockscout/BENS extension
  domain.resolvedWithWildcard = false; // Blockscout/BENS extension
  domain.owner = OWNER;
  domain.isMigrated = true;
  domain.subdomainCount = 0;
  domain.createdAt = BigInt.fromI32(0);
  domain.save();

  let mockEvent = newMockEvent();
  let event = new TransferBatch(
    Address.fromString(CONTRACT),
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
    new ethereum.EventParam("operator", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  event.parameters.push(
    new ethereum.EventParam("from", ethereum.Value.fromAddress(Address.zero()))
  );
  event.parameters.push(
    new ethereum.EventParam("to", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  let ids = new Array<BigInt>();
  ids.push(tokenId);
  event.parameters.push(
    new ethereum.EventParam("ids", ethereum.Value.fromUnsignedBigIntArray(ids))
  );
  let values = new Array<BigInt>();
  values.push(BigInt.fromI32(1));
  event.parameters.push(
    new ethereum.EventParam("values", ethereum.Value.fromUnsignedBigIntArray(values))
  );
  handleTransferBatch(event);
  assert.entityCount("WrappedTransfer", 1);
});

test("smoke: handleNameWrapperApprovalForAll creates an OperatorApproval row", () => {
  let mockEvent = newMockEvent();
  let event = new ApprovalForAll(
    Address.fromString(CONTRACT),
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
    new ethereum.EventParam("account", ethereum.Value.fromAddress(Address.fromString(OWNER)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "operator",
      ethereum.Value.fromAddress(Address.fromString(OTHER_OWNER))
    )
  );
  event.parameters.push(new ethereum.EventParam("approved", ethereum.Value.fromBoolean(true)));
  handleNameWrapperApprovalForAll(event);
  assert.entityCount("OperatorApproval", 1);
});

test("smoke: handleNameWrapperOwnershipTransferred sets ContractOwnership.owner", () => {
  let mockEvent = newMockEvent();
  let event = new OwnershipTransferred(
    Address.fromString(CONTRACT),
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
      "previousOwner",
      ethereum.Value.fromAddress(Address.fromString(OWNER))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "newOwner",
      ethereum.Value.fromAddress(Address.fromString(OTHER_OWNER))
    )
  );
  handleNameWrapperOwnershipTransferred(event);
  assert.fieldEquals(
    "ContractOwnership",
    Address.fromString(CONTRACT).toHexString(),
    "owner",
    Address.fromString(OTHER_OWNER).toHexString()
  );
});

test("smoke: handleControllerChanged flips RegistrarController.active", () => {
  let mockEvent = newMockEvent();
  let event = new ControllerChanged(
    Address.fromString(CONTRACT),
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
      "controller",
      ethereum.Value.fromAddress(Address.fromString(OTHER_OWNER))
    )
  );
  event.parameters.push(new ethereum.EventParam("active", ethereum.Value.fromBoolean(true)));
  handleControllerChanged(event);
  let id = Address.fromString(CONTRACT)
    .toHexString()
    .concat(Address.fromString(OTHER_OWNER).toHexString().slice(2));
  assert.fieldEquals("RegistrarController", id, "active", "true");
});
