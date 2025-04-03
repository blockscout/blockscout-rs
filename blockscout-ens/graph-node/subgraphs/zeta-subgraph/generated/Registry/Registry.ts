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

export class NewOwner extends ethereum.Event {
  get params(): NewOwner__Params {
    return new NewOwner__Params(this);
  }
}

export class NewOwner__Params {
  _event: NewOwner;

  constructor(event: NewOwner) {
    this._event = event;
  }

  get node(): Bytes {
    return this._event.parameters[0].value.toBytes();
  }

  get label(): Bytes {
    return this._event.parameters[1].value.toBytes();
  }

  get owner(): Address {
    return this._event.parameters[2].value.toAddress();
  }
}

export class NewResolver extends ethereum.Event {
  get params(): NewResolver__Params {
    return new NewResolver__Params(this);
  }
}

export class NewResolver__Params {
  _event: NewResolver;

  constructor(event: NewResolver) {
    this._event = event;
  }

  get node(): Bytes {
    return this._event.parameters[0].value.toBytes();
  }

  get resolver(): Address {
    return this._event.parameters[1].value.toAddress();
  }
}

export class NewTTL extends ethereum.Event {
  get params(): NewTTL__Params {
    return new NewTTL__Params(this);
  }
}

export class NewTTL__Params {
  _event: NewTTL;

  constructor(event: NewTTL) {
    this._event = event;
  }

  get node(): Bytes {
    return this._event.parameters[0].value.toBytes();
  }

  get ttl(): BigInt {
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

  get node(): Bytes {
    return this._event.parameters[0].value.toBytes();
  }

  get owner(): Address {
    return this._event.parameters[1].value.toAddress();
  }
}

export class Registry extends ethereum.SmartContract {
  static bind(address: Address): Registry {
    return new Registry("Registry", address);
  }

  isApprovedForAll(_owner: Address, _operator: Address): boolean {
    let result = super.call(
      "isApprovedForAll",
      "isApprovedForAll(address,address):(bool)",
      [
        ethereum.Value.fromAddress(_owner),
        ethereum.Value.fromAddress(_operator),
      ],
    );

    return result[0].toBoolean();
  }

  try_isApprovedForAll(
    _owner: Address,
    _operator: Address,
  ): ethereum.CallResult<boolean> {
    let result = super.tryCall(
      "isApprovedForAll",
      "isApprovedForAll(address,address):(bool)",
      [
        ethereum.Value.fromAddress(_owner),
        ethereum.Value.fromAddress(_operator),
      ],
    );
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toBoolean());
  }

  owner(_node: Bytes): Address {
    let result = super.call("owner", "owner(bytes32):(address)", [
      ethereum.Value.fromFixedBytes(_node),
    ]);

    return result[0].toAddress();
  }

  try_owner(_node: Bytes): ethereum.CallResult<Address> {
    let result = super.tryCall("owner", "owner(bytes32):(address)", [
      ethereum.Value.fromFixedBytes(_node),
    ]);
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toAddress());
  }

  recordExists(_node: Bytes): boolean {
    let result = super.call("recordExists", "recordExists(bytes32):(bool)", [
      ethereum.Value.fromFixedBytes(_node),
    ]);

    return result[0].toBoolean();
  }

  try_recordExists(_node: Bytes): ethereum.CallResult<boolean> {
    let result = super.tryCall("recordExists", "recordExists(bytes32):(bool)", [
      ethereum.Value.fromFixedBytes(_node),
    ]);
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toBoolean());
  }

  resolver(_node: Bytes): Address {
    let result = super.call("resolver", "resolver(bytes32):(address)", [
      ethereum.Value.fromFixedBytes(_node),
    ]);

    return result[0].toAddress();
  }

  try_resolver(_node: Bytes): ethereum.CallResult<Address> {
    let result = super.tryCall("resolver", "resolver(bytes32):(address)", [
      ethereum.Value.fromFixedBytes(_node),
    ]);
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toAddress());
  }

  setSubnodeOwner(_node: Bytes, _label: Bytes, _owner: Address): Bytes {
    let result = super.call(
      "setSubnodeOwner",
      "setSubnodeOwner(bytes32,bytes32,address):(bytes32)",
      [
        ethereum.Value.fromFixedBytes(_node),
        ethereum.Value.fromFixedBytes(_label),
        ethereum.Value.fromAddress(_owner),
      ],
    );

    return result[0].toBytes();
  }

  try_setSubnodeOwner(
    _node: Bytes,
    _label: Bytes,
    _owner: Address,
  ): ethereum.CallResult<Bytes> {
    let result = super.tryCall(
      "setSubnodeOwner",
      "setSubnodeOwner(bytes32,bytes32,address):(bytes32)",
      [
        ethereum.Value.fromFixedBytes(_node),
        ethereum.Value.fromFixedBytes(_label),
        ethereum.Value.fromAddress(_owner),
      ],
    );
    if (result.reverted) {
      return new ethereum.CallResult();
    }
    let value = result.value;
    return ethereum.CallResult.fromValue(value[0].toBytes());
  }

  ttl(_node: Bytes): BigInt {
    let result = super.call("ttl", "ttl(bytes32):(uint64)", [
      ethereum.Value.fromFixedBytes(_node),
    ]);

    return result[0].toBigInt();
  }

  try_ttl(_node: Bytes): ethereum.CallResult<BigInt> {
    let result = super.tryCall("ttl", "ttl(bytes32):(uint64)", [
      ethereum.Value.fromFixedBytes(_node),
    ]);
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

  get owner(): Address {
    return this._call.inputValues[0].value.toAddress();
  }
}

export class ConstructorCall__Outputs {
  _call: ConstructorCall;

  constructor(call: ConstructorCall) {
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

  get _operator(): Address {
    return this._call.inputValues[0].value.toAddress();
  }

  get _approved(): boolean {
    return this._call.inputValues[1].value.toBoolean();
  }
}

export class SetApprovalForAllCall__Outputs {
  _call: SetApprovalForAllCall;

  constructor(call: SetApprovalForAllCall) {
    this._call = call;
  }
}

export class SetOwnerCall extends ethereum.Call {
  get inputs(): SetOwnerCall__Inputs {
    return new SetOwnerCall__Inputs(this);
  }

  get outputs(): SetOwnerCall__Outputs {
    return new SetOwnerCall__Outputs(this);
  }
}

export class SetOwnerCall__Inputs {
  _call: SetOwnerCall;

  constructor(call: SetOwnerCall) {
    this._call = call;
  }

  get _node(): Bytes {
    return this._call.inputValues[0].value.toBytes();
  }

  get _owner(): Address {
    return this._call.inputValues[1].value.toAddress();
  }
}

export class SetOwnerCall__Outputs {
  _call: SetOwnerCall;

  constructor(call: SetOwnerCall) {
    this._call = call;
  }
}

export class SetRecordCall extends ethereum.Call {
  get inputs(): SetRecordCall__Inputs {
    return new SetRecordCall__Inputs(this);
  }

  get outputs(): SetRecordCall__Outputs {
    return new SetRecordCall__Outputs(this);
  }
}

export class SetRecordCall__Inputs {
  _call: SetRecordCall;

  constructor(call: SetRecordCall) {
    this._call = call;
  }

  get _node(): Bytes {
    return this._call.inputValues[0].value.toBytes();
  }

  get _owner(): Address {
    return this._call.inputValues[1].value.toAddress();
  }

  get _resolver(): Address {
    return this._call.inputValues[2].value.toAddress();
  }

  get _ttl(): BigInt {
    return this._call.inputValues[3].value.toBigInt();
  }
}

export class SetRecordCall__Outputs {
  _call: SetRecordCall;

  constructor(call: SetRecordCall) {
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

  get _node(): Bytes {
    return this._call.inputValues[0].value.toBytes();
  }

  get _resolver(): Address {
    return this._call.inputValues[1].value.toAddress();
  }
}

export class SetResolverCall__Outputs {
  _call: SetResolverCall;

  constructor(call: SetResolverCall) {
    this._call = call;
  }
}

export class SetSubnodeOwnerCall extends ethereum.Call {
  get inputs(): SetSubnodeOwnerCall__Inputs {
    return new SetSubnodeOwnerCall__Inputs(this);
  }

  get outputs(): SetSubnodeOwnerCall__Outputs {
    return new SetSubnodeOwnerCall__Outputs(this);
  }
}

export class SetSubnodeOwnerCall__Inputs {
  _call: SetSubnodeOwnerCall;

  constructor(call: SetSubnodeOwnerCall) {
    this._call = call;
  }

  get _node(): Bytes {
    return this._call.inputValues[0].value.toBytes();
  }

  get _label(): Bytes {
    return this._call.inputValues[1].value.toBytes();
  }

  get _owner(): Address {
    return this._call.inputValues[2].value.toAddress();
  }
}

export class SetSubnodeOwnerCall__Outputs {
  _call: SetSubnodeOwnerCall;

  constructor(call: SetSubnodeOwnerCall) {
    this._call = call;
  }

  get value0(): Bytes {
    return this._call.outputValues[0].value.toBytes();
  }
}

export class SetSubnodeRecordCall extends ethereum.Call {
  get inputs(): SetSubnodeRecordCall__Inputs {
    return new SetSubnodeRecordCall__Inputs(this);
  }

  get outputs(): SetSubnodeRecordCall__Outputs {
    return new SetSubnodeRecordCall__Outputs(this);
  }
}

export class SetSubnodeRecordCall__Inputs {
  _call: SetSubnodeRecordCall;

  constructor(call: SetSubnodeRecordCall) {
    this._call = call;
  }

  get _node(): Bytes {
    return this._call.inputValues[0].value.toBytes();
  }

  get _label(): Bytes {
    return this._call.inputValues[1].value.toBytes();
  }

  get _owner(): Address {
    return this._call.inputValues[2].value.toAddress();
  }

  get _resolver(): Address {
    return this._call.inputValues[3].value.toAddress();
  }

  get _ttl(): BigInt {
    return this._call.inputValues[4].value.toBigInt();
  }
}

export class SetSubnodeRecordCall__Outputs {
  _call: SetSubnodeRecordCall;

  constructor(call: SetSubnodeRecordCall) {
    this._call = call;
  }
}

export class SetTTLCall extends ethereum.Call {
  get inputs(): SetTTLCall__Inputs {
    return new SetTTLCall__Inputs(this);
  }

  get outputs(): SetTTLCall__Outputs {
    return new SetTTLCall__Outputs(this);
  }
}

export class SetTTLCall__Inputs {
  _call: SetTTLCall;

  constructor(call: SetTTLCall) {
    this._call = call;
  }

  get _node(): Bytes {
    return this._call.inputValues[0].value.toBytes();
  }

  get _ttl(): BigInt {
    return this._call.inputValues[1].value.toBigInt();
  }
}

export class SetTTLCall__Outputs {
  _call: SetTTLCall;

  constructor(call: SetTTLCall) {
    this._call = call;
  }
}
