// THIS IS AN AUTOGENERATED FILE. DO NOT EDIT THIS FILE DIRECTLY.

import {
  ethereum,
  JSONValue,
  TypedMap,
  Entity,
  Bytes,
  Address,
  BigInt,
} from "@graphprotocol/graph-ts";

export class Approval extends ethereum.Event {
  get params(): Approval__Params {
    return new Approval__Params(this);
  }
}

export class Approval__Params {
  _event: Approval;

  constructor(event: Approval) {
    this._event = event;
  }

  get owner(): Address {
    return this._event.parameters[0].value.toAddress();
  }

  get approved(): Address {
    return this._event.parameters[1].value.toAddress();
  }

  get tokenId(): BigInt {
    return this._event.parameters[2].value.toBigInt();
  }
}

export class ApprovalForAll extends ethereum.Event {
  get params(): ApprovalForAll__Params {
    return new ApprovalForAll__Params(this);
  }
}

export class ApprovalForAll__Params {
  _event: ApprovalForAll;

  constructor(event: ApprovalForAll) {
    this._event = event;
  }

  get owner(): Address {
    return this._event.parameters[0].value.toAddress();
  }

  get operator(): Address {
    return this._event.parameters[1].value.toAddress();
  }

  get approved(): boolean {
    return this._event.parameters[2].value.toBoolean();
  }
}

export class ControllerAdded extends ethereum.Event {
  get params(): ControllerAdded__Params {
    return new ControllerAdded__Params(this);
  }
}

export class ControllerAdded__Params {
  _event: ControllerAdded;

  constructor(event: ControllerAdded) {
    this._event = event;
  }

  get controller(): Address {
    return this._event.parameters[0].value.toAddress();
  }
}

export class ControllerRemoved extends ethereum.Event {
  get params(): ControllerRemoved__Params {
    return new ControllerRemoved__Params(this);
  }
}

export class ControllerRemoved__Params {
  _event: ControllerRemoved;

  constructor(event: ControllerRemoved) {
    this._event = event;
  }

  get controller(): Address {
    return this._event.parameters[0].value.toAddress();
  }
}

export class NameMigrated extends ethereum.Event {
  get params(): NameMigrated__Params {
    return new NameMigrated__Params(this);
  }
}

export class NameMigrated__Params {
  _event: NameMigrated;

  constructor(event: NameMigrated) {
    this._event = event;
  }

  get id(): BigInt {
    return this._event.parameters[0].value.toBigInt();
  }

  get owner(): Address {
    return this._event.parameters[1].value.toAddress();
  }

  get expires(): BigInt {
    return this._event.parameters[2].value.toBigInt();
  }
}

export class NameRegistered extends ethereum.Event {
  get params(): NameRegistered__Params {
    return new NameRegistered__Params(this);
  }
}

export class NameRegistered__Params {
  _event: NameRegistered;

  constructor(event: NameRegistered) {
    this._event = event;
  }

  get id(): BigInt {
    return this._event.parameters[0].value.toBigInt();
  }

  get owner(): Address {
    return this._event.parameters[1].value.toAddress();
  }

  get expires(): BigInt {
    return this._event.parameters[2].value.toBigInt();
  }
}

export class NameRenewed extends ethereum.Event {
  get params(): NameRenewed__Params {
    return new NameRenewed__Params(this);
  }
}

export class NameRenewed__Params {
  _event: NameRenewed;

  constructor(event: NameRenewed) {
    this._event = event;
  }

  get id(): BigInt {
    return this._event.parameters[0].value.toBigInt();
  }

  get expires(): BigInt {
    return this._event.parameters[1].value.toBigInt();
  }
}

export class Transfer extends ethereum.Event {
  get params(): Transfer__Params {
    return new Transfer__Params(this);
  }
}

export class Transfer__Params {
  _event: Transfer;

  constructor(event: Transfer) {
    this._event = event;
  }

  get from(): Address {
    return this._event.parameters[0].value.toAddress();
  }

  get to(): Address {
    return this._event.parameters[1].value.toAddress();
  }

  get tokenId(): BigInt {
    return this._event.parameters[2].value.toBigInt();
  }
}

export class Base__royaltyInfoResult {
  value0: Address;
  value1: BigInt;

  constructor(value0: Address, value1: BigInt) {
    this.value0 = value0;
    this.value1 = value1;
  }

  toMap(): TypedMap<string, ethereum.Value> {
    let map = new TypedMap<string, ethereum.Value>();
    map.set("value0", ethereum.Value.fromAddress(this.value0));
    map.set("value1", ethereum.Value.fromUnsignedBigInt(this.value1));
    return map;
  }

  getValue0(): Address {
    return this.value0;
  }

  getValue1(): BigInt {
    return this.value1;
  }
}

export class Base extends ethereum.SmartContract {
  static bind(address: Address): Base {
    return new Base("Base", address);
  }

  GRACE_PERIOD(): BigInt {
    let result = super.call("GRACE_PERIOD", "GRACE_PERIOD():(uint256)", []);

    return result[0].toBigInt();
  }

  try_GRACE_PERIOD(): ethereum.CallResult<BigInt> {
    let result = super.tryCall("GRACE_PERIOD", "GRACE_PERIOD():(uint256)", []);
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toBigInt());
  }

  available(id: BigInt): boolean {
    let result = super.call("available", "available(uint256):(bool)", [
      ethereum.Value.fromUnsignedBigInt(id),
    ]);

    return result[0].toBoolean();
  }

  try_available(id: BigInt): ethereum.CallResult<boolean> {
    let result = super.tryCall("available", "available(uint256):(bool)", [
      ethereum.Value.fromUnsignedBigInt(id),
    ]);
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toBoolean());
  }

  balanceOf(owner: Address): BigInt {
    let result = super.call("balanceOf", "balanceOf(address):(uint256)", [
      ethereum.Value.fromAddress(owner),
    ]);

    return result[0].toBigInt();
  }

  try_balanceOf(owner: Address): ethereum.CallResult<BigInt> {
    let result = super.tryCall("balanceOf", "balanceOf(address):(uint256)", [
      ethereum.Value.fromAddress(owner),
    ]);
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toBigInt());
  }

  baseNode(): Bytes {
    let result = super.call("baseNode", "baseNode():(bytes32)", []);

    return result[0].toBytes();
  }

  try_baseNode(): ethereum.CallResult<Bytes> {
    let result = super.tryCall("baseNode", "baseNode():(bytes32)", []);
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toBytes());
  }

  baseUri(): string {
    let result = super.call("baseUri", "baseUri():(string)", []);

    return result[0].toString();
  }

  try_baseUri(): ethereum.CallResult<string> {
    let result = super.tryCall("baseUri", "baseUri():(string)", []);
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toString());
  }

  getApproved(tokenId: BigInt): Address {
    let result = super.call("getApproved", "getApproved(uint256):(address)", [
      ethereum.Value.fromUnsignedBigInt(tokenId),
    ]);

    return result[0].toAddress();
  }

  try_getApproved(tokenId: BigInt): ethereum.CallResult<Address> {
    let result = super.tryCall(
      "getApproved",
      "getApproved(uint256):(address)",
      [ethereum.Value.fromUnsignedBigInt(tokenId)],
    );
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toAddress());
  }

  identifier(): BigInt {
    let result = super.call("identifier", "identifier():(uint256)", []);

    return result[0].toBigInt();
  }

  try_identifier(): ethereum.CallResult<BigInt> {
    let result = super.tryCall("identifier", "identifier():(uint256)", []);
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toBigInt());
  }

  isApprovedForAll(owner: Address, operator: Address): boolean {
    let result = super.call(
      "isApprovedForAll",
      "isApprovedForAll(address,address):(bool)",
      [ethereum.Value.fromAddress(owner), ethereum.Value.fromAddress(operator)],
    );

    return result[0].toBoolean();
  }

  try_isApprovedForAll(
    owner: Address,
    operator: Address,
  ): ethereum.CallResult<boolean> {
    let result = super.tryCall(
      "isApprovedForAll",
      "isApprovedForAll(address,address):(bool)",
      [ethereum.Value.fromAddress(owner), ethereum.Value.fromAddress(operator)],
    );
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toBoolean());
  }

  name(): string {
    let result = super.call("name", "name():(string)", []);

    return result[0].toString();
  }

  try_name(): ethereum.CallResult<string> {
    let result = super.tryCall("name", "name():(string)", []);
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toString());
  }

  nameExpires(id: BigInt): BigInt {
    let result = super.call("nameExpires", "nameExpires(uint256):(uint256)", [
      ethereum.Value.fromUnsignedBigInt(id),
    ]);

    return result[0].toBigInt();
  }

  try_nameExpires(id: BigInt): ethereum.CallResult<BigInt> {
    let result = super.tryCall(
      "nameExpires",
      "nameExpires(uint256):(uint256)",
      [ethereum.Value.fromUnsignedBigInt(id)],
    );
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toBigInt());
  }

  ownerOf(tokenId: BigInt): Address {
    let result = super.call("ownerOf", "ownerOf(uint256):(address)", [
      ethereum.Value.fromUnsignedBigInt(tokenId),
    ]);

    return result[0].toAddress();
  }

  try_ownerOf(tokenId: BigInt): ethereum.CallResult<Address> {
    let result = super.tryCall("ownerOf", "ownerOf(uint256):(address)", [
      ethereum.Value.fromUnsignedBigInt(tokenId),
    ]);
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toAddress());
  }

  register(id: BigInt, owner: Address, duration: BigInt): BigInt {
    let result = super.call(
      "register",
      "register(uint256,address,uint256):(uint256)",
      [
        ethereum.Value.fromUnsignedBigInt(id),
        ethereum.Value.fromAddress(owner),
        ethereum.Value.fromUnsignedBigInt(duration),
      ],
    );

    return result[0].toBigInt();
  }

  try_register(
    id: BigInt,
    owner: Address,
    duration: BigInt,
  ): ethereum.CallResult<BigInt> {
    let result = super.tryCall(
      "register",
      "register(uint256,address,uint256):(uint256)",
      [
        ethereum.Value.fromUnsignedBigInt(id),
        ethereum.Value.fromAddress(owner),
        ethereum.Value.fromUnsignedBigInt(duration),
      ],
    );
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toBigInt());
  }

  registerOnly(id: BigInt, owner: Address, duration: BigInt): BigInt {
    let result = super.call(
      "registerOnly",
      "registerOnly(uint256,address,uint256):(uint256)",
      [
        ethereum.Value.fromUnsignedBigInt(id),
        ethereum.Value.fromAddress(owner),
        ethereum.Value.fromUnsignedBigInt(duration),
      ],
    );

    return result[0].toBigInt();
  }

  try_registerOnly(
    id: BigInt,
    owner: Address,
    duration: BigInt,
  ): ethereum.CallResult<BigInt> {
    let result = super.tryCall(
      "registerOnly",
      "registerOnly(uint256,address,uint256):(uint256)",
      [
        ethereum.Value.fromUnsignedBigInt(id),
        ethereum.Value.fromAddress(owner),
        ethereum.Value.fromUnsignedBigInt(duration),
      ],
    );
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toBigInt());
  }

  renew(id: BigInt, duration: BigInt): BigInt {
    let result = super.call("renew", "renew(uint256,uint256):(uint256)", [
      ethereum.Value.fromUnsignedBigInt(id),
      ethereum.Value.fromUnsignedBigInt(duration),
    ]);

    return result[0].toBigInt();
  }

  try_renew(id: BigInt, duration: BigInt): ethereum.CallResult<BigInt> {
    let result = super.tryCall("renew", "renew(uint256,uint256):(uint256)", [
      ethereum.Value.fromUnsignedBigInt(id),
      ethereum.Value.fromUnsignedBigInt(duration),
    ]);
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toBigInt());
  }

  royaltyInfo(tokenId: BigInt, salePrice: BigInt): Base__royaltyInfoResult {
    let result = super.call(
      "royaltyInfo",
      "royaltyInfo(uint256,uint256):(address,uint256)",
      [
        ethereum.Value.fromUnsignedBigInt(tokenId),
        ethereum.Value.fromUnsignedBigInt(salePrice),
      ],
    );

    return new Base__royaltyInfoResult(
      result[0].toAddress(),
      result[1].toBigInt(),
    );
  }

  try_royaltyInfo(
    tokenId: BigInt,
    salePrice: BigInt,
  ): ethereum.CallResult<Base__royaltyInfoResult> {
    let result = super.tryCall(
      "royaltyInfo",
      "royaltyInfo(uint256,uint256):(address,uint256)",
      [
        ethereum.Value.fromUnsignedBigInt(tokenId),
        ethereum.Value.fromUnsignedBigInt(salePrice),
      ],
    );
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(
      new Base__royaltyInfoResult(value[0].toAddress(), value[1].toBigInt()),
    );
  }

  sann(): Address {
    let result = super.call("sann", "sann():(address)", []);

    return result[0].toAddress();
  }

  try_sann(): ethereum.CallResult<Address> {
    let result = super.tryCall("sann", "sann():(address)", []);
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toAddress());
  }

  sidRegistry(): Address {
    let result = super.call("sidRegistry", "sidRegistry():(address)", []);

    return result[0].toAddress();
  }

  try_sidRegistry(): ethereum.CallResult<Address> {
    let result = super.tryCall("sidRegistry", "sidRegistry():(address)", []);
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toAddress());
  }

  supplyAmount(): BigInt {
    let result = super.call("supplyAmount", "supplyAmount():(uint256)", []);

    return result[0].toBigInt();
  }

  try_supplyAmount(): ethereum.CallResult<BigInt> {
    let result = super.tryCall("supplyAmount", "supplyAmount():(uint256)", []);
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toBigInt());
  }

  supportsInterface(interfaceID: Bytes): boolean {
    let result = super.call(
      "supportsInterface",
      "supportsInterface(bytes4):(bool)",
      [ethereum.Value.fromFixedBytes(interfaceID)],
    );

    return result[0].toBoolean();
  }

  try_supportsInterface(interfaceID: Bytes): ethereum.CallResult<boolean> {
    let result = super.tryCall(
      "supportsInterface",
      "supportsInterface(bytes4):(bool)",
      [ethereum.Value.fromFixedBytes(interfaceID)],
    );
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toBoolean());
  }

  symbol(): string {
    let result = super.call("symbol", "symbol():(string)", []);

    return result[0].toString();
  }

  try_symbol(): ethereum.CallResult<string> {
    let result = super.tryCall("symbol", "symbol():(string)", []);
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toString());
  }

  tld(): string {
    let result = super.call("tld", "tld():(string)", []);

    return result[0].toString();
  }

  try_tld(): ethereum.CallResult<string> {
    let result = super.tryCall("tld", "tld():(string)", []);
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toString());
  }

  tokenURI(tokenId: BigInt): string {
    let result = super.call("tokenURI", "tokenURI(uint256):(string)", [
      ethereum.Value.fromUnsignedBigInt(tokenId),
    ]);

    return result[0].toString();
  }

  try_tokenURI(tokenId: BigInt): ethereum.CallResult<string> {
    let result = super.tryCall("tokenURI", "tokenURI(uint256):(string)", [
      ethereum.Value.fromUnsignedBigInt(tokenId),
    ]);
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toString());
  }

  totalSupply(): BigInt {
    let result = super.call("totalSupply", "totalSupply():(uint256)", []);

    return result[0].toBigInt();
  }

  try_totalSupply(): ethereum.CallResult<BigInt> {
    let result = super.tryCall("totalSupply", "totalSupply():(uint256)", []);
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toBigInt());
  }
}

export class ConstructorCall extends ethereum.Call {
  get inputs(): ConstructorCall__Inputs {
    return new ConstructorCall__Inputs(this);
  }

  get outputs(): ConstructorCall__Outputs {
    return new ConstructorCall__Outputs(this);
  }
}

export class ConstructorCall__Inputs {
  _call: ConstructorCall;

  constructor(call: ConstructorCall) {
    this._call = call;
  }

  get _sann(): Address {
    return this._call.inputValues[0].value.toAddress();
  }

  get _sidRegistry(): Address {
    return this._call.inputValues[1].value.toAddress();
  }

  get _identifier(): BigInt {
    return this._call.inputValues[2].value.toBigInt();
  }

  get _tld(): string {
    return this._call.inputValues[3].value.toString();
  }

  get _baseUri(): string {
    return this._call.inputValues[4].value.toString();
  }
}

export class ConstructorCall__Outputs {
  _call: ConstructorCall;

  constructor(call: ConstructorCall) {
    this._call = call;
  }
}

export class ApproveCall extends ethereum.Call {
  get inputs(): ApproveCall__Inputs {
    return new ApproveCall__Inputs(this);
  }

  get outputs(): ApproveCall__Outputs {
    return new ApproveCall__Outputs(this);
  }
}

export class ApproveCall__Inputs {
  _call: ApproveCall;

  constructor(call: ApproveCall) {
    this._call = call;
  }

  get to(): Address {
    return this._call.inputValues[0].value.toAddress();
  }

  get tokenId(): BigInt {
    return this._call.inputValues[1].value.toBigInt();
  }
}

export class ApproveCall__Outputs {
  _call: ApproveCall;

  constructor(call: ApproveCall) {
    this._call = call;
  }
}

export class DeleteDefaultRoyaltyCall extends ethereum.Call {
  get inputs(): DeleteDefaultRoyaltyCall__Inputs {
    return new DeleteDefaultRoyaltyCall__Inputs(this);
  }

  get outputs(): DeleteDefaultRoyaltyCall__Outputs {
    return new DeleteDefaultRoyaltyCall__Outputs(this);
  }
}

export class DeleteDefaultRoyaltyCall__Inputs {
  _call: DeleteDefaultRoyaltyCall;

  constructor(call: DeleteDefaultRoyaltyCall) {
    this._call = call;
  }
}

export class DeleteDefaultRoyaltyCall__Outputs {
  _call: DeleteDefaultRoyaltyCall;

  constructor(call: DeleteDefaultRoyaltyCall) {
    this._call = call;
  }
}

export class ReclaimCall extends ethereum.Call {
  get inputs(): ReclaimCall__Inputs {
    return new ReclaimCall__Inputs(this);
  }

  get outputs(): ReclaimCall__Outputs {
    return new ReclaimCall__Outputs(this);
  }
}

export class ReclaimCall__Inputs {
  _call: ReclaimCall;

  constructor(call: ReclaimCall) {
    this._call = call;
  }

  get id(): BigInt {
    return this._call.inputValues[0].value.toBigInt();
  }

  get owner(): Address {
    return this._call.inputValues[1].value.toAddress();
  }
}

export class ReclaimCall__Outputs {
  _call: ReclaimCall;

  constructor(call: ReclaimCall) {
    this._call = call;
  }
}

export class RegisterCall extends ethereum.Call {
  get inputs(): RegisterCall__Inputs {
    return new RegisterCall__Inputs(this);
  }

  get outputs(): RegisterCall__Outputs {
    return new RegisterCall__Outputs(this);
  }
}

export class RegisterCall__Inputs {
  _call: RegisterCall;

  constructor(call: RegisterCall) {
    this._call = call;
  }

  get id(): BigInt {
    return this._call.inputValues[0].value.toBigInt();
  }

  get owner(): Address {
    return this._call.inputValues[1].value.toAddress();
  }

  get duration(): BigInt {
    return this._call.inputValues[2].value.toBigInt();
  }
}

export class RegisterCall__Outputs {
  _call: RegisterCall;

  constructor(call: RegisterCall) {
    this._call = call;
  }

  get value0(): BigInt {
    return this._call.outputValues[0].value.toBigInt();
  }
}

export class RegisterOnlyCall extends ethereum.Call {
  get inputs(): RegisterOnlyCall__Inputs {
    return new RegisterOnlyCall__Inputs(this);
  }

  get outputs(): RegisterOnlyCall__Outputs {
    return new RegisterOnlyCall__Outputs(this);
  }
}

export class RegisterOnlyCall__Inputs {
  _call: RegisterOnlyCall;

  constructor(call: RegisterOnlyCall) {
    this._call = call;
  }

  get id(): BigInt {
    return this._call.inputValues[0].value.toBigInt();
  }

  get owner(): Address {
    return this._call.inputValues[1].value.toAddress();
  }

  get duration(): BigInt {
    return this._call.inputValues[2].value.toBigInt();
  }
}

export class RegisterOnlyCall__Outputs {
  _call: RegisterOnlyCall;

  constructor(call: RegisterOnlyCall) {
    this._call = call;
  }

  get value0(): BigInt {
    return this._call.outputValues[0].value.toBigInt();
  }
}

export class RenewCall extends ethereum.Call {
  get inputs(): RenewCall__Inputs {
    return new RenewCall__Inputs(this);
  }

  get outputs(): RenewCall__Outputs {
    return new RenewCall__Outputs(this);
  }
}

export class RenewCall__Inputs {
  _call: RenewCall;

  constructor(call: RenewCall) {
    this._call = call;
  }

  get id(): BigInt {
    return this._call.inputValues[0].value.toBigInt();
  }

  get duration(): BigInt {
    return this._call.inputValues[1].value.toBigInt();
  }
}

export class RenewCall__Outputs {
  _call: RenewCall;

  constructor(call: RenewCall) {
    this._call = call;
  }

  get value0(): BigInt {
    return this._call.outputValues[0].value.toBigInt();
  }
}

export class ResetTokenRoyaltyCall extends ethereum.Call {
  get inputs(): ResetTokenRoyaltyCall__Inputs {
    return new ResetTokenRoyaltyCall__Inputs(this);
  }

  get outputs(): ResetTokenRoyaltyCall__Outputs {
    return new ResetTokenRoyaltyCall__Outputs(this);
  }
}

export class ResetTokenRoyaltyCall__Inputs {
  _call: ResetTokenRoyaltyCall;

  constructor(call: ResetTokenRoyaltyCall) {
    this._call = call;
  }

  get _tokenId(): BigInt {
    return this._call.inputValues[0].value.toBigInt();
  }
}

export class ResetTokenRoyaltyCall__Outputs {
  _call: ResetTokenRoyaltyCall;

  constructor(call: ResetTokenRoyaltyCall) {
    this._call = call;
  }
}

export class SafeTransferFromCall extends ethereum.Call {
  get inputs(): SafeTransferFromCall__Inputs {
    return new SafeTransferFromCall__Inputs(this);
  }

  get outputs(): SafeTransferFromCall__Outputs {
    return new SafeTransferFromCall__Outputs(this);
  }
}

export class SafeTransferFromCall__Inputs {
  _call: SafeTransferFromCall;

  constructor(call: SafeTransferFromCall) {
    this._call = call;
  }

  get from(): Address {
    return this._call.inputValues[0].value.toAddress();
  }

  get to(): Address {
    return this._call.inputValues[1].value.toAddress();
  }

  get tokenId(): BigInt {
    return this._call.inputValues[2].value.toBigInt();
  }
}

export class SafeTransferFromCall__Outputs {
  _call: SafeTransferFromCall;

  constructor(call: SafeTransferFromCall) {
    this._call = call;
  }
}

export class SafeTransferFrom1Call extends ethereum.Call {
  get inputs(): SafeTransferFrom1Call__Inputs {
    return new SafeTransferFrom1Call__Inputs(this);
  }

  get outputs(): SafeTransferFrom1Call__Outputs {
    return new SafeTransferFrom1Call__Outputs(this);
  }
}

export class SafeTransferFrom1Call__Inputs {
  _call: SafeTransferFrom1Call;

  constructor(call: SafeTransferFrom1Call) {
    this._call = call;
  }

  get from(): Address {
    return this._call.inputValues[0].value.toAddress();
  }

  get to(): Address {
    return this._call.inputValues[1].value.toAddress();
  }

  get tokenId(): BigInt {
    return this._call.inputValues[2].value.toBigInt();
  }

  get _data(): Bytes {
    return this._call.inputValues[3].value.toBytes();
  }
}

export class SafeTransferFrom1Call__Outputs {
  _call: SafeTransferFrom1Call;

  constructor(call: SafeTransferFrom1Call) {
    this._call = call;
  }
}

export class SetApprovalForAllCall extends ethereum.Call {
  get inputs(): SetApprovalForAllCall__Inputs {
    return new SetApprovalForAllCall__Inputs(this);
  }

  get outputs(): SetApprovalForAllCall__Outputs {
    return new SetApprovalForAllCall__Outputs(this);
  }
}

export class SetApprovalForAllCall__Inputs {
  _call: SetApprovalForAllCall;

  constructor(call: SetApprovalForAllCall) {
    this._call = call;
  }

  get operator(): Address {
    return this._call.inputValues[0].value.toAddress();
  }

  get approved(): boolean {
    return this._call.inputValues[1].value.toBoolean();
  }
}

export class SetApprovalForAllCall__Outputs {
  _call: SetApprovalForAllCall;

  constructor(call: SetApprovalForAllCall) {
    this._call = call;
  }
}

export class SetDefaultRoyaltyCall extends ethereum.Call {
  get inputs(): SetDefaultRoyaltyCall__Inputs {
    return new SetDefaultRoyaltyCall__Inputs(this);
  }

  get outputs(): SetDefaultRoyaltyCall__Outputs {
    return new SetDefaultRoyaltyCall__Outputs(this);
  }
}

export class SetDefaultRoyaltyCall__Inputs {
  _call: SetDefaultRoyaltyCall;

  constructor(call: SetDefaultRoyaltyCall) {
    this._call = call;
  }

  get _receiver(): Address {
    return this._call.inputValues[0].value.toAddress();
  }

  get _feeNumerator(): BigInt {
    return this._call.inputValues[1].value.toBigInt();
  }
}

export class SetDefaultRoyaltyCall__Outputs {
  _call: SetDefaultRoyaltyCall;

  constructor(call: SetDefaultRoyaltyCall) {
    this._call = call;
  }
}

export class SetResolverCall extends ethereum.Call {
  get inputs(): SetResolverCall__Inputs {
    return new SetResolverCall__Inputs(this);
  }

  get outputs(): SetResolverCall__Outputs {
    return new SetResolverCall__Outputs(this);
  }
}

export class SetResolverCall__Inputs {
  _call: SetResolverCall;

  constructor(call: SetResolverCall) {
    this._call = call;
  }

  get resolver(): Address {
    return this._call.inputValues[0].value.toAddress();
  }
}

export class SetResolverCall__Outputs {
  _call: SetResolverCall;

  constructor(call: SetResolverCall) {
    this._call = call;
  }
}

export class SetTokenRoyaltyCall extends ethereum.Call {
  get inputs(): SetTokenRoyaltyCall__Inputs {
    return new SetTokenRoyaltyCall__Inputs(this);
  }

  get outputs(): SetTokenRoyaltyCall__Outputs {
    return new SetTokenRoyaltyCall__Outputs(this);
  }
}

export class SetTokenRoyaltyCall__Inputs {
  _call: SetTokenRoyaltyCall;

  constructor(call: SetTokenRoyaltyCall) {
    this._call = call;
  }

  get _tokenId(): BigInt {
    return this._call.inputValues[0].value.toBigInt();
  }

  get _receiver(): Address {
    return this._call.inputValues[1].value.toAddress();
  }

  get _feeNumerator(): BigInt {
    return this._call.inputValues[2].value.toBigInt();
  }
}

export class SetTokenRoyaltyCall__Outputs {
  _call: SetTokenRoyaltyCall;

  constructor(call: SetTokenRoyaltyCall) {
    this._call = call;
  }
}

export class SetURICall extends ethereum.Call {
  get inputs(): SetURICall__Inputs {
    return new SetURICall__Inputs(this);
  }

  get outputs(): SetURICall__Outputs {
    return new SetURICall__Outputs(this);
  }
}

export class SetURICall__Inputs {
  _call: SetURICall;

  constructor(call: SetURICall) {
    this._call = call;
  }

  get newURI(): string {
    return this._call.inputValues[0].value.toString();
  }
}

export class SetURICall__Outputs {
  _call: SetURICall;

  constructor(call: SetURICall) {
    this._call = call;
  }
}

export class TransferFromCall extends ethereum.Call {
  get inputs(): TransferFromCall__Inputs {
    return new TransferFromCall__Inputs(this);
  }

  get outputs(): TransferFromCall__Outputs {
    return new TransferFromCall__Outputs(this);
  }
}

export class TransferFromCall__Inputs {
  _call: TransferFromCall;

  constructor(call: TransferFromCall) {
    this._call = call;
  }

  get from(): Address {
    return this._call.inputValues[0].value.toAddress();
  }

  get to(): Address {
    return this._call.inputValues[1].value.toAddress();
  }

  get tokenId(): BigInt {
    return this._call.inputValues[2].value.toBigInt();
  }
}

export class TransferFromCall__Outputs {
  _call: TransferFromCall;

  constructor(call: TransferFromCall) {
    this._call = call;
  }
}
