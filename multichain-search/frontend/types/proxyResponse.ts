export type NetworkGroup = 'mainnets' | 'testnets' | 'other';

export interface ProxyResponse {
  [instance_name: string]: InstanceResponse
}

export interface Instance {
id: string,
title: string,
url: string
}

export interface InstanceResponse {
    instance: Instance,
    content: string,
    status: number
    uri: string,
    elapsed_secs: number
}


export interface InstanceSearchResponse {
    items: InstanceSearchResponseItem[],
}

export type InstanceSearchResponseItem = Token | Contract | Address | Block | Transaction


export interface Token {
    type: "token",
    name: string,
    symbol: string,
    address: string,
    token_url: string,
    address_url: string,
}

export interface Contract {
    type: "contract",
    name: string,
    address: string,
    url: string,
}

export interface Address {
    type: "address",
    name: string,
    address: string,
    url: string,
}

export interface Block {
    type: "block",
    block_number: string,
    block_hash: string,
    url: string,
}

export interface Transaction {
    type: "transaction",
    tx_hash: string,
    url: string,
}