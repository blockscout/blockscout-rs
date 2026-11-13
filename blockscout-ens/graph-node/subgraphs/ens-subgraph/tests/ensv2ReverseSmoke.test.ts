// Minimal smoke-test layer (mock event -> call handler -> basic assertion
// only) for every handler wired from src/ensv2Reverse.ts. Deliberately
// independent of and redundant with the richer behavioral tests in
// ensv2Reverse.test.ts -- a baseline safety net, not a replacement.
import { Address, ethereum } from "@graphprotocol/graph-ts";
import { assert, newMockEvent, test } from "matchstick-as/assembly/index";
import { handleNameForAddrChanged } from "../src/ensv2Reverse";
import { NameForAddrChanged } from "../src/types/DefaultReverseRegistrar/DefaultReverseRegistrar";

const REGISTRAR_ADDRESS = "0x4F382928805ba0e23B30cFB75fC9E848e82DFD47";
const ADDR = "0xf1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1";

test("smoke: handleNameForAddrChanged creates an ENSv2PrimaryName row", () => {
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
    new ethereum.EventParam("addr", ethereum.Value.fromAddress(Address.fromString(ADDR)))
  );
  event.parameters.push(
    new ethereum.EventParam("name", ethereum.Value.fromString("smoketest.eth"))
  );

  handleNameForAddrChanged(event);

  assert.fieldEquals(
    "ENSv2PrimaryName",
    Address.fromString(ADDR).toHexString(),
    "name",
    "smoketest.eth"
  );
});
