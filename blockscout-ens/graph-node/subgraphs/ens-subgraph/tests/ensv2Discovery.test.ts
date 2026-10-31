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
const PROXY_ADDRESS = "0x11111111111111111111111111111111111111aa";
const IMPLEMENTATION = "0x22222222222222222222222222222222222222bb";
// Must match ensv2Constants.ts's sepolia-network return values exactly —
// deliberately hardcoded here (not imported) so a test breaks if that file's
// constants ever drift, rather than tautologically always agreeing with them.
const USER_REGISTRY_IMPL = "0x9BD8a88719068D09ecee662f36C0E3856708366a";
const WRAPPER_REGISTRY_IMPL = "0xBe768b63E5fBBFBB0Ae97E9064E0002dF8001880";
const PERMISSIONED_RESOLVER_IMPL = "0x115eb53F0c60696633855F90b138178Fb40b2b2C";
const STANDALONE_HCA_IMPL = "0xC940e5C5bF263C0e097054AECf73826769A72CEE";

const createProxyDeployedEvent = (
  proxyAddress: string,
  implementation: string
): ProxyDeployed => {
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
    new ethereum.EventParam(
      "sender",
      ethereum.Value.fromAddress(Address.fromString(SENDER))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "proxyAddress",
      ethereum.Value.fromAddress(Address.fromString(proxyAddress))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "salt",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromI32(1))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "implementation",
      ethereum.Value.fromAddress(Address.fromString(implementation))
    )
  );
  return event;
};

// VerifiableFactory.deployProxy() is used for both registry and resolver
// proxies. This asserts the fallback case: an
// implementation address that's neither a known registry impl (see the
// USER/WRAPPER tests below) nor the known resolver impl (see the
// no-row-created test below) still gets templated and creates an
// ENSv2Registry row classified UNKNOWN for any unrecognized implementation.
// What this test
// *can't* assert (no dataSourceCount/dataSourceExists helper exists in the
// installed matchstick-as) is that ENSv2RegistryTemplate.create() actually
// registered a dynamic data source; that requires an integration test against
// graph-node.
test("handleProxyDeployed creates an UNKNOWN ENSv2Registry for an unrecognized implementation", () => {
  dataSourceMock.setNetwork("sepolia");

  let event = createProxyDeployedEvent(PROXY_ADDRESS, IMPLEMENTATION);
  handleProxyDeployed(event);

  let id = Address.fromString(PROXY_ADDRESS).toHexString();
  assert.fieldEquals("ENSv2Registry", id, "kind", "UNKNOWN");
  assert.fieldEquals(
    "ENSv2Registry",
    id,
    "address",
    Address.fromString(PROXY_ADDRESS).toHexString()
  );
  assert.fieldEquals(
    "ENSv2Registry",
    id,
    "implementation",
    Address.fromString(IMPLEMENTATION).toHexString()
  );
});

test("handleProxyDeployed classifies a UserRegistry implementation as USER", () => {
  dataSourceMock.setNetwork("sepolia");

  // This file has no clearStore()/beforeEach between tests, so each test
  // needs its own proxy address — reusing PROXY_ADDRESS would load the
  // already-created row from an earlier test instead of creating a fresh
  // one (getOrCreateRegistry is a no-op once a row exists at that id).
  let userProxyAddress = "0x33333333333333333333333333333333333333cc";
  let event = createProxyDeployedEvent(userProxyAddress, USER_REGISTRY_IMPL);
  handleProxyDeployed(event);

  let id = Address.fromString(userProxyAddress).toHexString();
  assert.fieldEquals("ENSv2Registry", id, "kind", "USER");
});

test("handleProxyDeployed classifies a WrapperRegistry implementation as WRAPPER", () => {
  dataSourceMock.setNetwork("sepolia");

  let wrapperProxyAddress = "0x44444444444444444444444444444444444444dd";
  let event = createProxyDeployedEvent(wrapperProxyAddress, WRAPPER_REGISTRY_IMPL);
  handleProxyDeployed(event);

  let id = Address.fromString(wrapperProxyAddress).toHexString();
  assert.fieldEquals("ENSv2Registry", id, "kind", "WRAPPER");
});

test("handleProxyDeployed creates no registry row for a PermissionedResolver implementation", () => {
  dataSourceMock.setNetwork("sepolia");

  let resolverProxyAddress = "0x55555555555555555555555555555555555555ee";
  let event = createProxyDeployedEvent(resolverProxyAddress, PERMISSIONED_RESOLVER_IMPL);
  handleProxyDeployed(event);

  let id = Address.fromString(resolverProxyAddress).toHexString();
  assert.notInStore("ENSv2Registry", id);
});

// StandaloneHCAFactory shares this VerifiableFactory instance rather than
// deploying its own, so an HCA deployment fires the identical ProxyDeployed
// event a registry deployment does. Store a dedicated ENSv2HCA row rather
// than an ENSv2Registry row.
test("handleProxyDeployed skips templating and creates an ENSv2HCA row, not an ENSv2Registry row, for a StandaloneHCA implementation", () => {
  dataSourceMock.setNetwork("sepolia");

  let hcaProxyAddress = "0x66666666666666666666666666666666666666ff";
  let event = createProxyDeployedEvent(hcaProxyAddress, STANDALONE_HCA_IMPL);
  handleProxyDeployed(event);

  let id = Address.fromString(hcaProxyAddress).toHexString();
  assert.notInStore("ENSv2Registry", id);
  assert.fieldEquals(
    "ENSv2HCA",
    id,
    "implementation",
    Address.fromString(STANDALONE_HCA_IMPL).toHexString()
  );
  assert.fieldEquals(
    "ENSv2HCA",
    id,
    "deployer",
    Address.fromString(SENDER).toHexString()
  );
});

test("handleProxyDeployed is idempotent for a duplicate ProxyDeployed on the same address", () => {
  dataSourceMock.setNetwork("sepolia");

  let duplicateProxyAddress = "0x77777777777777777777777777777777777777aa";
  let event = createProxyDeployedEvent(duplicateProxyAddress, USER_REGISTRY_IMPL);
  handleProxyDeployed(event);
  // Same address, same implementation, processed a second time -- e.g. a
  // reorg replay. getOrCreateRegistry's own load-or-new guard must make
  // this a no-op, not a second row or an error.
  handleProxyDeployed(event);

  let id = Address.fromString(duplicateProxyAddress).toHexString();
  assert.fieldEquals("ENSv2Registry", id, "kind", "USER");
  assert.fieldEquals(
    "ENSv2Registry",
    id,
    "implementation",
    Address.fromString(USER_REGISTRY_IMPL).toHexString()
  );
});

// Mainnet has two live UserRegistry implementations: the one in namechain's
// deployment records and the newer one the ENS app / ENSjs deploy against.
// Both must classify as USER; the pre-launch 0x75ff… implementation stays
// UNKNOWN by design.
const MAINNET_USER_REGISTRY_IMPL_ARTIFACT = "0xfaa23592aca10a6d8875b18ea86954e8eb6d8d72";
const MAINNET_USER_REGISTRY_IMPL_APP = "0xC927bCaA77cd7bE283518760896Be0CF3Cfbe08d";
const MAINNET_USER_REGISTRY_IMPL_OLD = "0x75ff9adbcfce2469b097ac68f6ae650eae1351bb";

test("mainnet: both live UserRegistry implementations classify as USER, the superseded one stays UNKNOWN", () => {
  dataSourceMock.setNetwork("mainnet");

  let a = "0x55555555555555555555555555555555555555aa";
  handleProxyDeployed(createProxyDeployedEvent(a, MAINNET_USER_REGISTRY_IMPL_ARTIFACT));
  assert.fieldEquals("ENSv2Registry", Address.fromString(a).toHexString(), "kind", "USER");

  let b = "0x55555555555555555555555555555555555555bb";
  handleProxyDeployed(createProxyDeployedEvent(b, MAINNET_USER_REGISTRY_IMPL_APP));
  assert.fieldEquals("ENSv2Registry", Address.fromString(b).toHexString(), "kind", "USER");

  let c = "0x55555555555555555555555555555555555555cc";
  handleProxyDeployed(createProxyDeployedEvent(c, MAINNET_USER_REGISTRY_IMPL_OLD));
  assert.fieldEquals("ENSv2Registry", Address.fromString(c).toHexString(), "kind", "UNKNOWN");

  dataSourceMock.setNetwork("sepolia");
});
