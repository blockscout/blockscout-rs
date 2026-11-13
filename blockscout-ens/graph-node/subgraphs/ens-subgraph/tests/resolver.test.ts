import { Address, BigInt, Bytes, ethereum } from "@graphprotocol/graph-ts";
import {
  assert,
  beforeEach,
  clearStore,
  describe,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import {
  createResolverID,
  handleABIChanged,
  handleAddrChanged,
  handleAuthorisationChanged,
  handleContentHashChanged,
  handleInterfaceChanged,
  handleMulticoinAddrChanged,
  handleNameChanged,
  handlePubkeyChanged,
  handleTextChanged,
  handleTextChangedWithValue,
  handleVersionChanged,
} from "../src/resolver";
import {
  ABIChanged,
  AddrChanged,
  AddressChanged,
  AuthorisationChanged,
  ContenthashChanged,
  InterfaceChanged,
  NameChanged,
  PubkeyChanged,
  TextChanged,
  TextChanged1 as TextChangedWithValue,
  VersionChanged,
} from "../src/types/Resolver/Resolver";
import { Resolver } from "../src/types/schema";

const namehash = Bytes.fromHexString(
  "0x7857c9824139b8a8c3cb04712b41558b4878c55fa9c1e5390e910ee3220c3cce"
);

const addressValue = ethereum.Value.fromAddress(
  Address.fromString("0x8e8db5ccef88cca9d624701db544989c996e3216")
);

const getResolver = (address: Address): Resolver | null =>
  Resolver.load(createResolverID(namehash, address));

describe("creates new resolver when resolver entity doesn't exist", () => {
  beforeEach(() => {
    clearStore();
  });
  test("handleAddrChanged", () => {
    const mockEvent = newMockEvent();
    const resolverEvent = new AddrChanged(
      mockEvent.address,
      mockEvent.logIndex,
      mockEvent.transactionLogIndex,
      mockEvent.logType,
      mockEvent.block,
      mockEvent.transaction,
      mockEvent.parameters,
      mockEvent.receipt
    );
    resolverEvent.parameters = new Array();

    let nodeParam = new ethereum.EventParam(
      "node",
      ethereum.Value.fromFixedBytes(namehash)
    );
    let aParam = new ethereum.EventParam("a", addressValue);
    resolverEvent.parameters.push(nodeParam);
    resolverEvent.parameters.push(aParam);

    let resolver = getResolver(mockEvent.address);

    assert.assertNull(resolver);

    handleAddrChanged(resolverEvent);

    resolver = getResolver(mockEvent.address);

    assert.assertNotNull(resolver);
  });
  test("handleMulticoinAddrChanged", () => {
    const mockEvent = newMockEvent();
    const resolverEvent = new AddressChanged(
      mockEvent.address,
      mockEvent.logIndex,
      mockEvent.transactionLogIndex,
      mockEvent.logType,
      mockEvent.block,
      mockEvent.transaction,
      mockEvent.parameters,
      mockEvent.receipt
    );
    resolverEvent.parameters = new Array();

    let nodeParam = new ethereum.EventParam(
      "node",
      ethereum.Value.fromFixedBytes(namehash)
    );
    let coinTypeParam = new ethereum.EventParam(
      "coinType",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(60))
    );
    let newAddressParam = new ethereum.EventParam(
      "newAddress",
      ethereum.Value.fromBytes(
        Address.fromString("0x8e8db5ccef88cca9d624701db544989c996e3216")
      )
    );
    resolverEvent.parameters.push(nodeParam);
    resolverEvent.parameters.push(coinTypeParam);
    resolverEvent.parameters.push(newAddressParam);

    let resolver = getResolver(mockEvent.address);

    assert.assertNull(resolver);

    handleMulticoinAddrChanged(resolverEvent);

    resolver = getResolver(mockEvent.address);

    assert.assertNotNull(resolver);
  });
  test("handleNameChanged", () => {
    const mockEvent = newMockEvent();
    const resolverEvent = new NameChanged(
      mockEvent.address,
      mockEvent.logIndex,
      mockEvent.transactionLogIndex,
      mockEvent.logType,
      mockEvent.block,
      mockEvent.transaction,
      mockEvent.parameters,
      mockEvent.receipt
    );
    resolverEvent.parameters = new Array();

    let nodeParam = new ethereum.EventParam(
      "node",
      ethereum.Value.fromFixedBytes(namehash)
    );
    let nameParam = new ethereum.EventParam(
      "name",
      ethereum.Value.fromString("test.eth")
    );
    resolverEvent.parameters.push(nodeParam);
    resolverEvent.parameters.push(nameParam);

    let resolver = getResolver(mockEvent.address);

    assert.assertNull(resolver);

    handleNameChanged(resolverEvent);

    resolver = getResolver(mockEvent.address);

    assert.assertNotNull(resolver);
  });
  test("handleABIChanged", () => {
    const mockEvent = newMockEvent();
    const resolverEvent = new ABIChanged(
      mockEvent.address,
      mockEvent.logIndex,
      mockEvent.transactionLogIndex,
      mockEvent.logType,
      mockEvent.block,
      mockEvent.transaction,
      mockEvent.parameters,
      mockEvent.receipt
    );
    resolverEvent.parameters = new Array();

    let nodeParam = new ethereum.EventParam(
      "node",
      ethereum.Value.fromFixedBytes(namehash)
    );
    let contentTypeParam = new ethereum.EventParam(
      "contentType",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(1))
    );
    resolverEvent.parameters.push(nodeParam);
    resolverEvent.parameters.push(contentTypeParam);

    let resolver = getResolver(mockEvent.address);

    assert.assertNull(resolver);

    handleABIChanged(resolverEvent);

    resolver = getResolver(mockEvent.address);

    assert.assertNotNull(resolver);
  });
  test("handlePubkeyChanged", () => {
    const mockEvent = newMockEvent();
    const resolverEvent = new PubkeyChanged(
      mockEvent.address,
      mockEvent.logIndex,
      mockEvent.transactionLogIndex,
      mockEvent.logType,
      mockEvent.block,
      mockEvent.transaction,
      mockEvent.parameters,
      mockEvent.receipt
    );
    resolverEvent.parameters = new Array();

    let nodeParam = new ethereum.EventParam(
      "node",
      ethereum.Value.fromFixedBytes(namehash)
    );
    let xParam = new ethereum.EventParam(
      "x",
      ethereum.Value.fromBytes(Bytes.fromI32(1))
    );
    let yParam = new ethereum.EventParam(
      "y",
      ethereum.Value.fromBytes(Bytes.fromI32(1))
    );
    resolverEvent.parameters.push(nodeParam);
    resolverEvent.parameters.push(xParam);
    resolverEvent.parameters.push(yParam);

    let resolver = getResolver(mockEvent.address);

    assert.assertNull(resolver);

    handlePubkeyChanged(resolverEvent);

    resolver = getResolver(mockEvent.address);

    assert.assertNotNull(resolver);
  });
  test("handleTextChanged", () => {
    const mockEvent = newMockEvent();
    const resolverEvent = new TextChanged(
      mockEvent.address,
      mockEvent.logIndex,
      mockEvent.transactionLogIndex,
      mockEvent.logType,
      mockEvent.block,
      mockEvent.transaction,
      mockEvent.parameters,
      mockEvent.receipt
    );
    resolverEvent.parameters = new Array();

    let nodeParam = new ethereum.EventParam(
      "node",
      ethereum.Value.fromFixedBytes(namehash)
    );
    let indexedKeyParam = new ethereum.EventParam(
      "indexedKey",
      ethereum.Value.fromString("foo")
    );
    let keyParam = new ethereum.EventParam(
      "key",
      ethereum.Value.fromString("foo")
    );
    resolverEvent.parameters.push(nodeParam);
    resolverEvent.parameters.push(indexedKeyParam);
    resolverEvent.parameters.push(keyParam);

    let resolver = getResolver(mockEvent.address);

    assert.assertNull(resolver);

    handleTextChanged(resolverEvent);

    resolver = getResolver(mockEvent.address);

    assert.assertNotNull(resolver);
  });
  test("handleTextChangedWithValue", () => {
    const mockEvent = newMockEvent();
    const resolverEvent = new TextChangedWithValue(
      mockEvent.address,
      mockEvent.logIndex,
      mockEvent.transactionLogIndex,
      mockEvent.logType,
      mockEvent.block,
      mockEvent.transaction,
      mockEvent.parameters,
      mockEvent.receipt
    );
    resolverEvent.parameters = new Array();

    let nodeParam = new ethereum.EventParam(
      "node",
      ethereum.Value.fromFixedBytes(namehash)
    );
    let indexedKeyParam = new ethereum.EventParam(
      "indexedKey",
      ethereum.Value.fromString("foo")
    );
    let keyParam = new ethereum.EventParam(
      "key",
      ethereum.Value.fromString("foo")
    );
    let valueParam = new ethereum.EventParam(
      "value",
      ethereum.Value.fromString("bar")
    );
    resolverEvent.parameters.push(nodeParam);
    resolverEvent.parameters.push(indexedKeyParam);
    resolverEvent.parameters.push(keyParam);
    resolverEvent.parameters.push(valueParam);

    let resolver = getResolver(mockEvent.address);

    assert.assertNull(resolver);

    handleTextChangedWithValue(resolverEvent);

    resolver = getResolver(mockEvent.address);

    assert.assertNotNull(resolver);
  });
  test("handleContentHashChanged", () => {
    const mockEvent = newMockEvent();
    const resolverEvent = new ContenthashChanged(
      mockEvent.address,
      mockEvent.logIndex,
      mockEvent.transactionLogIndex,
      mockEvent.logType,
      mockEvent.block,
      mockEvent.transaction,
      mockEvent.parameters,
      mockEvent.receipt
    );
    resolverEvent.parameters = new Array();

    let nodeParam = new ethereum.EventParam(
      "node",
      ethereum.Value.fromFixedBytes(namehash)
    );
    let hashParam = new ethereum.EventParam(
      "hash",
      ethereum.Value.fromBytes(Bytes.fromI32(0))
    );
    resolverEvent.parameters.push(nodeParam);
    resolverEvent.parameters.push(hashParam);

    let resolver = getResolver(mockEvent.address);

    assert.assertNull(resolver);

    handleContentHashChanged(resolverEvent);

    resolver = getResolver(mockEvent.address);

    assert.assertNotNull(resolver);
  });
  test("handleInterfaceChanged", () => {
    const mockEvent = newMockEvent();
    const resolverEvent = new InterfaceChanged(
      mockEvent.address,
      mockEvent.logIndex,
      mockEvent.transactionLogIndex,
      mockEvent.logType,
      mockEvent.block,
      mockEvent.transaction,
      mockEvent.parameters,
      mockEvent.receipt
    );
    resolverEvent.parameters = new Array();

    let nodeParam = new ethereum.EventParam(
      "node",
      ethereum.Value.fromFixedBytes(namehash)
    );
    let interfaceIdParam = new ethereum.EventParam(
      "interfaceID",
      ethereum.Value.fromFixedBytes(Bytes.fromI32(0))
    );
    let implementerParam = new ethereum.EventParam("implementer", addressValue);
    resolverEvent.parameters.push(nodeParam);
    resolverEvent.parameters.push(interfaceIdParam);
    resolverEvent.parameters.push(implementerParam);

    let resolver = getResolver(mockEvent.address);

    assert.assertNull(resolver);

    handleInterfaceChanged(resolverEvent);

    resolver = getResolver(mockEvent.address);

    assert.assertNotNull(resolver);
  });
  test("handleAuthorisationChanged", () => {
    const mockEvent = newMockEvent();
    const resolverEvent = new AuthorisationChanged(
      mockEvent.address,
      mockEvent.logIndex,
      mockEvent.transactionLogIndex,
      mockEvent.logType,
      mockEvent.block,
      mockEvent.transaction,
      mockEvent.parameters,
      mockEvent.receipt
    );
    resolverEvent.parameters = new Array();

    let nodeParam = new ethereum.EventParam(
      "node",
      ethereum.Value.fromFixedBytes(namehash)
    );
    let ownerParam = new ethereum.EventParam("owner", addressValue);
    let targetParam = new ethereum.EventParam(
      "target",
      ethereum.Value.fromAddress(
        Address.fromString("0x8e8db5ccef88cca9d624701db544989c996e3216")
      )
    );
    let isAuthorised = new ethereum.EventParam(
      "isAuthorised",
      ethereum.Value.fromBoolean(true)
    );
    resolverEvent.parameters.push(nodeParam);
    resolverEvent.parameters.push(ownerParam);
    resolverEvent.parameters.push(targetParam);
    resolverEvent.parameters.push(isAuthorised);

    let resolver = getResolver(mockEvent.address);

    assert.assertNull(resolver);

    handleAuthorisationChanged(resolverEvent);

    resolver = getResolver(mockEvent.address);

    assert.assertNotNull(resolver);
  });
  test("handleVersionChanged", () => {
    const mockEvent = newMockEvent();
    const resolverEvent = new VersionChanged(
      mockEvent.address,
      mockEvent.logIndex,
      mockEvent.transactionLogIndex,
      mockEvent.logType,
      mockEvent.block,
      mockEvent.transaction,
      mockEvent.parameters,
      mockEvent.receipt
    );
    resolverEvent.parameters = new Array();

    let nodeParam = new ethereum.EventParam(
      "node",
      ethereum.Value.fromFixedBytes(namehash)
    );
    let newVersionParam = new ethereum.EventParam(
      "newVersion",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(1))
    );

    resolverEvent.parameters.push(nodeParam);
    resolverEvent.parameters.push(newVersionParam);

    let resolver = getResolver(mockEvent.address);

    assert.assertNull(resolver);

    handleVersionChanged(resolverEvent);

    resolver = getResolver(mockEvent.address);

    assert.assertNotNull(resolver);
  });
});
