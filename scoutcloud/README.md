Scoutcloud Service
===

Scoutcloud provides API to deploy and manage blockscout instances. 
It tracks amount of time each instance is running and charges user for it.

## Dev

+ Install [just](https://github.com/casey/just) cli. Just is like make but better.
+ Execute `just` to see available dev commands

```bash
just
```
+ Start dev postgres service by just typing

```bash
just start-postgres
```

+ Now you ready to start API server! Just run it:
```bash
just run
```

## Troubleshooting

1. Invalid tonic version

```
`Router` and `Router` have similar names, but are actually distinct types
```

To fix this error you need to change tonic version of `tonic` in `blockscout-service-launcer` to `0.8`

For now you can only change in `Cargo.lock`
