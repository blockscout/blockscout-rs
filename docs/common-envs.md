# Common Environment Variables

By convention each env has a `{SERVICE_NAME}` (e.g, `{SMART_CONTRACT_VERIFIER}`) as a prefix,
and uses `__` as a separator between internal identifiers. 

| Variable                                                       | Required | Description                                                                      | Default value                            |
|----------------------------------------------------------------|----------|----------------------------------------------------------------------------------|------------------------------------------|
| `{SERVICE_NAME}__SERVER__HTTP__ENABLED`                        |          | Enable HTTP API server                                                           | `true`                                   |
| `{SERVICE_NAME}__SERVER__HTTP__ADDR`                           |          | HTTP API listening interface                                                     | `0.0.0.0:8050`                           |
| `{SERVICE_NAME}__SERVER__HTTP__MAX_BODY_SIZE`                  |          | Max HTTP body size for incoming API requests                                     | `2097152`                                |
| `{SERVICE_NAME}__SERVER__HTTP__CORS__ENABLED`                  |          | Enable CORS middleware for incoming HTTP requests                                | `false`                                  |
| `{SERVICE_NAME}__SERVER__HTTP__CORS__ALLOWED_ORIGIN`           |          | Origins allowed to make requests                                                 |                                          |
| `{SERVICE_NAME}__SERVER__HTTP__CORS__ALLOWED_METHODS`          |          | A list of methods which allowed origins can perform                              | `PUT, GET, POST, OPTIONS, DELETE, PATCH` |
| `{SERVICE_NAME}__SERVER__HTTP__CORS__ALLOWED_CREDENTIALS`      |          | Allow users to make authenticated requests                                       | `true`                                   |
| `{SERVICE_NAME}__SERVER__HTTP__CORS__MAX_AGE`                  |          | Sets a maximum time (in seconds) for which this CORS request may be cached       | `3600`                                   |
| `{SERVICE_NAME}__SERVER__HTTP__CORS__BLOCK_ON_ORIGIN_MISMATCH` |          | Configures whether requests should be pre-emptively blocked on mismatched origin | `false`                                  |
| `SMART_CONTRACT_VERIFIER__SERVER__GRPC__ENABLED`               |          | Enable GRPC API server                                                           | `false`                                  |
| `SMART_CONTRACT_VERIFIER__SERVER__GRPC__ADDR`                  |          | GRPC API listening interface                                                     | `0.0.0.0:8051`                           |
| `SMART_CONTRACT_VERIFIER__METRICS__ENABLED`                    |          | Enable metrics collection endpoint                                               | `false`                                  |
| `SMART_CONTRACT_VERIFIER__METRICS__ADDR`                       |          | Metrics collection listening interface                                           | `0.0.0.0:6060`                           |
| `SMART_CONTRACT_VERIFIER__METRICS__ROUTE`                      |          | Metrics collection API route                                                     | `/metrics`                               |
| `SMART_CONTRACT_VERIFIER__TRACING__ENABLED`                    |          | Enable tracing log module                                                        | `true`                                   |
| `SMART_CONTRACT_VERIFIER__TRACING__FORMAT`                     |          | Tracing format. `default` / `json`                                               | `default`                                |
| `SMART_CONTRACT_VERIFIER__JAEGER__ENABLED`                     |          | Enable Jaeger tracing                                                            | `false`                                  |
| `SMART_CONTRACT_VERIFIER__JAEGER__AGENT_ENDPOINT`              |          | Jaeger tracing listening interface                                               | `localhost:6831`                         |
