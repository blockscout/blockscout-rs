# .hood Name Service Subgraph

ENS-compatible `.hood` names on **Robinhood Chain** (chain id `4663`).

Indexes Registry / BaseRegistrar / RegistrarController / PublicResolver so
Blockscout BENS can resolve names like `monkey.hood`.

## Mainnet

| Source | Address | startBlock |
|---|---|---|
| Registry | `0xEA48F389c296A6f823a488210194c50af41517d8` | 15140962 |
| Registrar (HOODNAME) | `0x4942CE5912706A05034F7018dFBF19953B7dFC80` | 15140992 |
| Controller | `0xebEb27e29cE202365a57A1aDeeb25B2CB5e77923` | 15141206 |
| Resolver | `0xeDBC408F887aE72c692232FE2437510f27288046` | 15141293 |

Base node = `namehash("hood")` =
`0x17f79377132793bf63f8c99a522a617a401dc4826aa34aa9cc11e97310c22e5d`

## Note vs HoodID (#1702)

This subgraph uses **stock ENS event signatures** (same as Basenames / Mode),
not custom `Bens*` events. Live Goldsky deployment already indexes `monkey.hood`.
