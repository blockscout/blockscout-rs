import { Address, BigInt, Bytes, ethereum } from "@graphprotocol/graph-ts";
import {
  assert,
  beforeAll,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import { handleNewOwner } from "../src/ensRegistry";
import {
  handleNameRegistered,
  handleNameRegisteredByController,
} from "../src/ethRegistrar";
import { NameRegistered } from "../src/types/BaseRegistrar/BaseRegistrar";
import { NewOwner } from "../src/types/ENSRegistry/EnsRegistry";
import { NameRegistered as NameRegisteredByController } from "../src/types/EthRegistrarController/EthRegistrarController";
import { Registration } from "../src/types/schema";

const ETH_NAMEHASH =
  "0x93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae";

const DEFAULT_OWNER = "0x89205A3A3b2A69De6Dbf7f01ED13B2108B2c43e7";

const createNameRegisteredByControllerEvent = (
  name: string,
  label: string,
  owner: string,
  expires: string
): NameRegisteredByController => {
  let mockEvent = newMockEvent();
  let nameRegisteredByControllerEvent = new NameRegisteredByController(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );

  nameRegisteredByControllerEvent.parameters = new Array();
  let nameParam = new ethereum.EventParam(
    "name",
    ethereum.Value.fromString(name)
  );
  let labelParam = new ethereum.EventParam(
    "label",
    ethereum.Value.fromBytes(Bytes.fromHexString(label))
  );
  let ownerParam = new ethereum.EventParam(
    "owner",
    ethereum.Value.fromAddress(Address.fromString(owner))
  );
  let baseCostParam = new ethereum.EventParam(
    "baseCost",
    ethereum.Value.fromSignedBigInt(BigInt.fromI32(0))
  );
  let premiumParam = new ethereum.EventParam(
    "premium",
    ethereum.Value.fromSignedBigInt(BigInt.fromI32(0))
  );
  let expiresParam = new ethereum.EventParam(
    "expires",
    ethereum.Value.fromSignedBigInt(BigInt.fromString(expires))
  );
  nameRegisteredByControllerEvent.parameters.push(nameParam);
  nameRegisteredByControllerEvent.parameters.push(labelParam);
  nameRegisteredByControllerEvent.parameters.push(ownerParam);
  nameRegisteredByControllerEvent.parameters.push(baseCostParam);
  nameRegisteredByControllerEvent.parameters.push(premiumParam);
  nameRegisteredByControllerEvent.parameters.push(expiresParam);

  return nameRegisteredByControllerEvent;
};

const createNewOwnerEvent = (
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

const createNameRegisteredEvent = (
  id: string,
  owner: string,
  expires: string
): NameRegistered => {
  let mockEvent = newMockEvent();
  let newNameRegisteredEvent = new NameRegistered(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  newNameRegisteredEvent.parameters = new Array();
  let idParam = new ethereum.EventParam(
    "id",
    ethereum.Value.fromSignedBigInt(BigInt.fromString(id))
  );
  let ownerParam = new ethereum.EventParam(
    "owner",
    ethereum.Value.fromAddress(Address.fromString(owner))
  );
  let expiresParam = new ethereum.EventParam(
    "expires",
    ethereum.Value.fromSignedBigInt(BigInt.fromString(expires))
  );
  newNameRegisteredEvent.parameters.push(idParam);
  newNameRegisteredEvent.parameters.push(ownerParam);
  newNameRegisteredEvent.parameters.push(expiresParam);
  return newNameRegisteredEvent;
};

beforeAll(() => {
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
});

const checkNullLabelName = (
  labelhash: string,
  labelhashAsInt: string,
  label: string
): void => {
  const newNewOwnerEvent = createNewOwnerEvent(
    ETH_NAMEHASH,
    labelhash,
    DEFAULT_OWNER
  );
  handleNewOwner(newNewOwnerEvent);

  let newRegistrationEvent = createNameRegisteredEvent(
    labelhashAsInt,
    DEFAULT_OWNER,
    "1610000000"
  );
  handleNameRegistered(newRegistrationEvent);

  let fetchedRegistration = Registration.load(labelhash)!;

  // set labelName to null because handleNameRegistered sets it to a mocked value of "default"
  // which comes from ens.nameByHash()
  fetchedRegistration.labelName = null;
  fetchedRegistration.save();

  const nameRegisteredByControllerEvent = createNameRegisteredByControllerEvent(
    label,
    labelhash,
    DEFAULT_OWNER,
    "1610000000"
  );
  handleNameRegisteredByController(nameRegisteredByControllerEvent);

  fetchedRegistration = Registration.load(labelhash)!;

  assert.assertNull(fetchedRegistration.labelName);
};

test("does not assign label name to null byte label", () => {
  const labelhash =
    "0x465b93df44674596a1f5cd92ec83053bb8a78f6083e1752b3162c739bba1f9ed";
  const labelhashAsInt =
    "31823703059708284547668674100687316300171847632515296374731848165239501748717";
  const label = "default\0";

  checkNullLabelName(labelhash, labelhashAsInt, label);
});

test("does not assign label name to label with '.' separator", () => {
  const labelhash =
    "0xf8a2e15376341ae37c90b754e5ef3f1e43d1d136a5c7ba6b34c50b466848dfbc";
  const labelhashAsInt =
    "112461370816196049012812662280597321405198137204162513382374556989424524648380";
  const label = "test.123";

  checkNullLabelName(labelhash, labelhashAsInt, label);
});
