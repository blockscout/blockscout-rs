# ArcNS Subgraph

Blockscout/BENS-compatible subgraph for [ArcNS](https://docs.arcns.xyz) — the name service for Arc Testnet (Chain ID: 5042002).

Indexes `.arc` and `.circle` domain names using the ENS-like schema required by the BENS microservice.

## Contracts indexed (Arc Testnet, start block 38856377)

| Contract | Address |
|---|---|
| ArcController | `0xe0A67F2E74Bcb740F0446fF2aCF32081DB877D46` |
| CircleController | `0x4CB0650847459d9BbDd5823cc6D320C900D883dA` |
| ArcBaseRegistrar | `0xD600B8D80e921ec48845fC1769c292601e5e90C4` |
| CircleBaseRegistrar | `0xE1fdE46df4bAC6F433C52a337F4818822735Bf8a` |
| ArcNSRegistry | `0xc20B3F8C7A7B4FcbFfe35c6C63331a1D9D12fD1A` |
| ArcNSResolver | `0x4c3a2D4245346732CE498937fEAD6343e77Eb097` |
| ArcNSReverseRegistrar | `0x352a1917Dd82158eC9bc71A0AC84F1b95Af26304` |

## Build

```bash
npm install
npm run codegen
npm run build
```

## Deploy

```bash
npm run deploy-local
```
