# Generate

> NOTE: paperclip only supports swagger_2, so we need to convert from v3 to v2

```bash
npm install -g api-spec-converter

api-spec-converter --from=openapi_3 --to=swagger_2 --syntax=yaml --order=alpha https://raw.githubusercontent.com/blockscout/blockscout-api-v2-swagger/main/swagger.yaml | sed "s|790000000000000000000|1|g"  > swagger_v2.yaml

cargo install paperclip --features cli

paperclip --api v2 -o ./src swagger_v2.yaml
```
