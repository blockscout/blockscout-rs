import { Address, Bytes, ethereum } from "@graphprotocol/graph-ts";
import { newMockEvent } from "matchstick-as/assembly/index";
import { handleNewOwner } from "../src/ensRegistry";
import { NewOwner } from "../src/types/ENSRegistry/EnsRegistry";

export const DEFAULT_OWNER = "0x89205A3A3b2A69De6Dbf7f01ED13B2108B2c43e7";

export const createNewOwnerEvent = (
  node: string,
  label: string,
  owner: string
): NewOwner => {
  let mockEvent = newMockEvent();
  let newNewOwnerEvent = new NewOwner(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );

  newNewOwnerEvent.parameters = new Array();
  let nodeParam = new ethereum.EventParam(
    "node",
    ethereum.Value.fromBytes(Bytes.fromHexString(node))
  );
  let labelParam = new ethereum.EventParam(
    "label",
    ethereum.Value.fromBytes(Bytes.fromHexString(label))
  );
  let ownerParam = new ethereum.EventParam(
    "owner",
    ethereum.Value.fromAddress(Address.fromString(owner))
  );
  newNewOwnerEvent.parameters.push(nodeParam);
  newNewOwnerEvent.parameters.push(labelParam);
  newNewOwnerEvent.parameters.push(ownerParam);
  return newNewOwnerEvent;
};

export const setEthOwner = (): void => {
  const ethLabelhash =
    "0x4f5b812789fc606be1b3b16908db13fc7a9adf7ca72641f84d75b47069d3d7f0";
  const emptyNode =
    "0x0000000000000000000000000000000000000000000000000000000000000000";
  const newNewOwnerEvent = createNewOwnerEvent(
    emptyNode,
    ethLabelhash,
    DEFAULT_OWNER
  );
  handleNewOwner(newNewOwnerEvent);
};
