Multichain Aggregator Service
===

This service is responsible for aggregating data from multiple Blockscout instances and providing a unified search API.

## Dev

+ Install [just](https://github.com/casey/just) cli. Just is like make but better.
+ Execute `just` to see avaliable dev commands

```bash
just
```
+ Start dev postgres service by just typing

```bash
just start-postgres
```

+ Now you ready to start API server! Just run it:
```
just run
```

## Troubleshooting

1. Invalid tonic version

```
`Router` and `Router` have similar names, but are actually distinct types
```

To fix this error you need to change tonic version of `tonic` in `blockscout-service-launcer` to `0.8`

For now you can only change in `Cargo.lock`
