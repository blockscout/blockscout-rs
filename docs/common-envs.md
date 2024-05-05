# Common Environment Variables

| Variable                                                       | Is required | Example value                            | Comment                                                                          |
|----------------------------------------------------------------|-------------|------------------------------------------|----------------------------------------------------------------------------------|
| `{SERVICE_NAME}__SERVER__HTTP__ENABLED`                        |             | `true`                                   | Enable HTTP API server                                                           |
| `{SERVICE_NAME}__SERVER__HTTP__ADDR`                           |             | `0.0.0.0:8050`                           | HTTP API listening interface                                                     |
| `{SERVICE_NAME}__SERVER__HTTP__MAX_BODY_SIZE`                  |             | `2097152`                                | Max HTTP body size for incoming API requests                                     |
| `{SERVICE_NAME}__SERVER__HTTP__CORS__ENABLED`                  |             | `false`                                  | Enable CORS middleware for incoming HTTP requests                                |
| `{SERVICE_NAME}__SERVER__HTTP__CORS__ALLOWED_ORIGIN`           |             |                                          | Origins allowed to make requests                                                 |
| `{SERVICE_NAME}__SERVER__HTTP__CORS__ALLOWED_METHODS`          |             | `PUT, GET, POST, OPTIONS, DELETE, PATCH` | A list of methods which allowed origins can perform                              |
| `{SERVICE_NAME}__SERVER__HTTP__CORS__ALLOWED_CREDENTIALS`      |             | `true`                                   | Allow users to make authenticated requests                                       |
| `{SERVICE_NAME}__SERVER__HTTP__CORS__MAX_AGE`                  |             | `3600`                                   | Sets a maximum time (in seconds) for which this CORS request may be cached       |
| `{SERVICE_NAME}__SERVER__HTTP__CORS__BLOCK_ON_ORIGIN_MISMATCH` |             | `false`                                  | Configures whether requests should be pre-emptively blocked on mismatched origin |
| `SMART_CONTRACT_VERIFIER__SERVER__GRPC__ENABLED`               |             | `false`                                  | Enable GRPC API server                                                           |
| `SMART_CONTRACT_VERIFIER__SERVER__GRPC__ADDR`                  |             | `0.0.0.0:8051`                           | GRPC API listening interface                                                     |
| `SMART_CONTRACT_VERIFIER__METRICS__ENABLED`                    |             | `false`                                  | Enable metrics collection endpoint                                               |
| `SMART_CONTRACT_VERIFIER__METRICS__ADDR`                       |             | `0.0.0.0:6060`                           | Metrics collection listening interface                                           |
| `SMART_CONTRACT_VERIFIER__METRICS__ROUTE`                      |             | `/metrics`                               | Metrics collection API route                                                     |
| `SMART_CONTRACT_VERIFIER__TRACING__ENABLED`                    |             | `true`                                   | Enable tracing log module                                                        |
| `SMART_CONTRACT_VERIFIER__TRACING__FORMAT`                     |             | `default`                                | Tracing format. `default` / `json`                                               |
| `SMART_CONTRACT_VERIFIER__JAEGER__ENABLED`                     |             | `false`                                  | Enable Jaeger tracing                                                            |
| `SMART_CONTRACT_VERIFIER__JAEGER__AGENT_ENDPOINT`              |             | `localhost:6831`                         | Jaeger tracing listening interface                                               |
