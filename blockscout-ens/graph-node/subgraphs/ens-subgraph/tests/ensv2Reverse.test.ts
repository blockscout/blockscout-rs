// ENSIP-19 "default" reverse resolution (DefaultReverseRegistrar). See
// schema.graphql's comment on ENSv2PrimaryName for scope.
import { Address, Bytes, ethereum } from "@graphprotocol/graph-ts";
import {
  afterEach,
  assert,
  clearStore,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import { handleNameForAddrChanged } from "../src/ensv2Reverse";
import { NameForAddrChanged } from "../src/types/DefaultReverseRegistrar/DefaultReverseRegistrar";
import { ENSv2PrimaryName } from "../src/types/schema";

const REGISTRAR_ADDRESS = "0x4F382928805ba0e23B30cFB75fC9E848e82DFD47";
const ADDR_A = "0x1111111111111111111111111111111111111aa1";
const ADDR_B = "0x2222222222222222222222222222222222222bb2";

const createNameForAddrChangedEvent = (
  addr: string,
  name: string
): NameForAddrChanged => {
  let mockEvent = newMockEvent();
  let event = new NameForAddrChanged(
    Address.fromString(REGISTRAR_ADDRESS),
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
    new ethereum.EventParam("addr", ethereum.Value.fromAddress(Address.fromString(addr)))
  );
  event.parameters.push(
    new ethereum.EventParam("name", ethereum.Value.fromString(name))
  );
  return event;
};

afterEach(() => {
  clearStore();
});

test("handleNameForAddrChanged creates ENSv2PrimaryName with name and account set", () => {
  let event = createNameForAddrChangedEvent(ADDR_A, "alice.eth");
  handleNameForAddrChanged(event);

  let id = Address.fromString(ADDR_A).toHexString();
  assert.fieldEquals("ENSv2PrimaryName", id, "name", "alice.eth");
  assert.fieldEquals("ENSv2PrimaryName", id, "account", id);
});

test("a second NameForAddrChanged for the same address overwrites the existing row rather than duplicating", () => {
  let first = createNameForAddrChangedEvent(ADDR_A, "alice.eth");
  handleNameForAddrChanged(first);

  let second = createNameForAddrChangedEvent(ADDR_A, "alice-new.eth");
  handleNameForAddrChanged(second);

  assert.entityCount("ENSv2PrimaryName", 1);
  let id = Address.fromString(ADDR_A).toHexString();
  assert.fieldEquals("ENSv2PrimaryName", id, "name", "alice-new.eth");
});

test("an empty-string name clears the primary name to null instead of deleting the row", () => {
  let setEvent = createNameForAddrChangedEvent(ADDR_A, "alice.eth");
  handleNameForAddrChanged(setEvent);

  let clearEvent = createNameForAddrChangedEvent(ADDR_A, "");
  handleNameForAddrChanged(clearEvent);

  let id = Address.fromString(ADDR_A).toHexString();
  assert.entityCount("ENSv2PrimaryName", 1);
  let primaryName = ENSv2PrimaryName.load(Bytes.fromHexString(id));
  assert.assertTrue(primaryName != null);
  assert.assertNull(primaryName!.name);
});

test("two different addresses each get their own independent row", () => {
  handleNameForAddrChanged(createNameForAddrChangedEvent(ADDR_A, "alice.eth"));
  handleNameForAddrChanged(createNameForAddrChangedEvent(ADDR_B, "bob.eth"));

  assert.entityCount("ENSv2PrimaryName", 2);
  assert.fieldEquals(
    "ENSv2PrimaryName",
    Address.fromString(ADDR_A).toHexString(),
    "name",
    "alice.eth"
  );
  assert.fieldEquals(
    "ENSv2PrimaryName",
    Address.fromString(ADDR_B).toHexString(),
    "name",
    "bob.eth"
  );
});

// Real repro from the external bug report (jmulq/ens-subgraph "ensv2-sepolia-final"):
// Sepolia tx 0x6b9eb47824bc3cb21170cd8f0f5407b5b0bea8c36ea43986c6227885231a8473
// (block 11,912,466) fired this exact NameForAddrChanged -- addr
// 0xC2D72Cc58d7574C238ea258209076a7d9C719c2b set its ENSIP-19 default
// primary name to "testintegration.eth" via the v2 flow. A later tx in the
// same report (0x851b761e..., block 11,912,467) clears the *legacy*
// addr.reverse resolver on the v1 ENSRegistry for the same address (already
// indexed generically by the existing v1 NewResolver handler) -- before this
// fix, that left the subgraph with no record of the primary name at all,
// even though NameForAddrChanged (this test) is the event that actually
// carries it.
test("real Sepolia repro: testintegration.eth primary name for 0xC2D72Cc58d7574C238ea258209076a7d9C719c2b is now indexed", () => {
  let reportedAddress = "0xC2D72Cc58d7574C238ea258209076a7d9C719c2b";
  let event = createNameForAddrChangedEvent(reportedAddress, "testintegration.eth");
  handleNameForAddrChanged(event);

  let id = Address.fromString(reportedAddress).toHexString();
  assert.fieldEquals("ENSv2PrimaryName", id, "name", "testintegration.eth");
});
