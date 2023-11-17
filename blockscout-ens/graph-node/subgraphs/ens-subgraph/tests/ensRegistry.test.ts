import { Address, Bytes, ethereum } from "@graphprotocol/graph-ts";
import {
  assert,
  beforeAll,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import { handleNewOwner, handleNewResolver } from "../src/ensRegistry";
import { NewOwner, NewResolver } from "../src/types/ENSRegistry/EnsRegistry";
import { Domain } from "../src/types/schema";

const ETH_NAMEHASH =
  "0x93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae";

const DEFAULT_OWNER = "0x89205A3A3b2A69De6Dbf7f01ED13B2108B2c43e7";

const DEFAULT_RESOLVER = "0x4976fb03C32e5B8cfe2b6cCB31c09Ba78EBaBa41";

const EMPTY_ADDRESS = "0x0000000000000000000000000000000000000000";

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

const createNewResolverEvent = (
  node: string,
  resolver: string
): NewResolver => {
  let mockEvent = newMockEvent();
  let newResolverEvent = new NewResolver(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );

  newResolverEvent.parameters = new Array();
  let nodeParam = new ethereum.EventParam(
    "node",
    ethereum.Value.fromFixedBytes(Bytes.fromHexString(node))
  );
  let resolverParam = new ethereum.EventParam(
    "resolver",
    ethereum.Value.fromAddress(Address.fromString(resolver))
  );
  newResolverEvent.parameters.push(nodeParam);
  newResolverEvent.parameters.push(resolverParam);

  return newResolverEvent;
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

test("sets 0x0 resolver to null", () => {
  // something.eth
  const labelhash =
    "0x68371d7e884c168ae2022c82bd837d51837718a7f7dfb7aa3f753074a35e1d87";
  const namehash =
    "0x7857c9824139b8a8c3cb04712b41558b4878c55fa9c1e5390e910ee3220c3cce";
  const newNewOwnerEvent = createNewOwnerEvent(
    ETH_NAMEHASH,
    labelhash,
    DEFAULT_OWNER
  );
  handleNewOwner(newNewOwnerEvent);

  const newNewResolverEvent = createNewResolverEvent(
    namehash,
    DEFAULT_RESOLVER
  );
  handleNewResolver(newNewResolverEvent);

  let fetchedDomain = Domain.load(namehash)!;

  assert.assertNotNull(fetchedDomain.resolver);

  const emptyResolverEvent = createNewResolverEvent(namehash, EMPTY_ADDRESS);
  handleNewResolver(emptyResolverEvent);

  fetchedDomain = Domain.load(namehash)!;

  assert.assertNull(fetchedDomain.resolver);
});
