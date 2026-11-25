// Minimal smoke-test layer (mock event -> call handler -> basic assertion
// only) for every handler wired from src/ensv2Discovery.ts. Deliberately
// independent of and redundant with the richer behavioral tests in
// ensv2Discovery.test.ts -- a baseline safety net, not a replacement.
import { Address, BigInt, ethereum } from "@graphprotocol/graph-ts";
import {
  assert,
  dataSourceMock,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import { handleProxyDeployed } from "../src/ensv2Discovery";
import { ProxyDeployed } from "../src/types/VerifiableFactory/VerifiableFactory";

const FACTORY_ADDRESS = "0xD2a632D8a8b67c2c4398c255CbD7aF8dd7236198";
const SENDER = "0x89205A3A3b2A69De6Dbf7f01ED13B2108B2c43e7";
const PROXY_ADDRESS = "0xf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0";
const IMPLEMENTATION = "0xe0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0";

test("smoke: handleProxyDeployed runs and creates an ENSv2Registry row", () => {
  dataSourceMock.setNetwork("sepolia");

  let mockEvent = newMockEvent();
  let event = new ProxyDeployed(
    Address.fromString(FACTORY_ADDRESS),
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
    new ethereum.EventParam("sender", ethereum.Value.fromAddress(Address.fromString(SENDER)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "proxyAddress",
      ethereum.Value.fromAddress(Address.fromString(PROXY_ADDRESS))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("salt", ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(1)))
  );
  event.parameters.push(
    new ethereum.EventParam(
      "implementation",
      ethereum.Value.fromAddress(Address.fromString(IMPLEMENTATION))
    )
  );

  handleProxyDeployed(event);

  assert.fieldEquals(
    "ENSv2Registry",
    Address.fromString(PROXY_ADDRESS).toHexString(),
    "kind",
    "UNKNOWN"
  );
});
