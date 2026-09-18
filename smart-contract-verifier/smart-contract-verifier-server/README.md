# <h1 align="center"> Smart-contract Verifier (Server) </h1>

Smart-contract verification service. Runs HTTP and (or) GRPC server and allows
making verification requests through corresponding APIs. The server itself is stateless
and answers requests based on provided information only.

## Configuration

Service supports configuration via configuration file and environment variables.
The latter overwrites the former in case if both are provided. For all missing fields
default values are used (if possible).

### Configuration file

Service uses a configuration file the path to which is specified via `SMART_CONTRACT_VERIFIER__CONFIG=[path]` environment variable.
The base configuration file with all available options could be found at [config/base.toml](./config/base.toml).

Below is an example of a simple configuration file which is filled with default values.

```toml
[server.http]
# When disabled, HTTP server is not running
enabled = true
# IP address and port number the HTTP server should listen to
addr = "0.0.0.0:8050"
# The maximum JSON payload is able to be processed
max_body_size = 2097152

[server.grpc]
# (Disabled by default) When disabled, GRPC server is not running
enabled = false
# IP address and port number the GRPC server should listen to
addr = "0.0.0.0:8051"

[solidity]
# When disabled, solidity related handlers are not available
enabled = true
# A directory where compilers would be downloaded to
compilers_dir = "/tmp/solidity-compilers"
# List of available solidity versions updates cron formatted schedule 
refresh_versions_schedule = "0 0 * * * * *"

[solidity.fetcher.list]
# List of all available solidity compilers and information about them.
list_url = "https://binaries.soliditylang.org/linux-amd64/list.json"

[vyper]
# When disabled, vyper related handlers are not available
enabled = true
# A directory where vyper compilers would be downloaded to
compilers_dir = "/tmp/vyper-compilers"
# List of available versions updates cron formatted schedule
refresh_versions_schedule = "0 0 * * * * *"

[vyper.fetcher.list]
# List of all available vyper compilers and information about them
list_url = "https://raw.githubusercontent.com/blockscout/solc-bin/main/vyper.list.json"

[sourcify]
# When disabled, sourcify related handlers are not available
enabled = true
# Sourcify API endpoint
api_url = "https://sourcify.dev/server/"
# Number of failing attempts the server makes to Sourcify API
verification_attempts = 3
# The maximum period (in seconds) the service is waiting for the Sourcify response
request_timeout = 10

[compilers]
# Maximum concurrent compiler invocations, including version probes. If omitted, the number of
# CPU cores is used. In Docker mode this is the remote-job limit.
max_threads = 8

# Compiler-backed endpoints fail to start until this is changed explicitly.
[compilers.execution]
type = "disabled"

[metrics]
# When disabled, metrics are not available
enabled = false
# IP address and port number metrics related endpoint should listen to
addr = "0.0.0.0:6060"
# A route at which metrics related endpoint is available
route = "/metrics"

[jaeger]
# When disabled, jaeger tracing is not available
enabled = false
# An endpoint where jaeger collects all traces
agent_endpoint = "localhost:6831"
```

For local development, native execution accepts the same invocation timeout and output-size keys:

```toml
[compilers.execution]
type = "native"
execution_timeout_seconds = 600
max_output_bytes = 268435456
```

Both fields retain these defaults when omitted and reject zero. Set them from Kubernetes-style
environment variables as `SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__EXECUTION_TIMEOUT_SECONDS`
and `SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__MAX_OUTPUT_BYTES`. Benchmark representative
`viaIR` and optimizer workloads before choosing a deployment-specific timeout.

### Isolated compiler execution

Production deployments should run compiler jobs on a dedicated Docker host reached over SSH. The
service uploads request-specific source inputs through the Docker API, starts one fresh container
for one invocation, and removes the container and its anonymous job volume after completion. Compiler
binaries are SHA-256 content-addressed and kept in named volumes on the Docker host, so a compiler is
transferred only on the first cache miss and then mounted read-only into later jobs. The Kubernetes
pod does not need a Docker socket or a shared filesystem with the Docker host.

```toml
[compilers]
max_threads = 8

[compilers.execution]
type = "docker"
addr = "ssh://compiler-runner@compiler-vm.example.org"
key_path = "/home/app/.ssh/id_ed25519"
# Tags are rejected. Preload this exact digest on the remote host before starting the service.
runner_image = "ghcr.io/blockscout/smart-contract-verifier-compiler-runner@sha256:<64-hex-digest>"
platform = "linux/amd64"
connect_timeout_seconds = 30
api_timeout_seconds = 30
execution_timeout_seconds = 600
memory_limit_bytes = 1073741824
nano_cpus = 2000000000
# Threads count toward this limit; zksolc starts a thread per host CPU plus a process per contract.
pids_limit = 1024
max_upload_bytes = 268435456
max_output_bytes = 268435456
# Optional: a hardened runtime installed on the external host, such as gVisor.
# runtime = "runsc"
```

`connect_timeout_seconds` independently bounds the lazy SSH host-key preflight. Docker API
requests use the greater of `api_timeout_seconds` and `execution_timeout_seconds`, so cold-cache
uploads retain the compiler execution budget. Attach and wait streams remain bounded by
`execution_timeout_seconds` after their response headers arrive.

Upload tarballs are assembled in bounded anonymous temporary files and streamed to the Docker
host. During a cold cache fill, a job spool and one compiler-seed spool can coexist; size pod
ephemeral storage for up to roughly `2 * max_threads * max_upload_bytes` of concurrent spooling.
The verifier does not retain complete tarballs in memory.

Build the minimal runner image with `compiler-runner.Dockerfile`, push it, and preload the selected
digest on the dedicated VM. Mount the SSH private key and a pinned `known_hosts` file into the
service pod. The remote SSH account must be dedicated to this service. The configured compiler list
must match `platform` (the production defaults download Linux amd64 compiler binaries).

Each job keeps `/tmp` as a size-bounded `noexec` home directory. `TMPDIR` points to a separate,
private, size-bounded executable tmpfs at `/compiler-tmp`; this is required by the PyInstaller
one-file Vyper release binaries and is removed with the container.

Compiler volumes are named `scv-compiler-v1-sha256-<digest>` and must use the local volume driver
with these ownership labels: `org.blockscout.smart-contract-verifier.compiler-cache=true`,
`org.blockscout.smart-contract-verifier.compiler-cache-schema=1`, and
`org.blockscout.smart-contract-verifier.compiler-digest=sha256:<digest>`, plus an
`org.blockscout.smart-contract-verifier.compiler-cache-generation=<uuid>` label that binds the
in-process warm-cache state to one volume incarnation. No other labels or driver options are
accepted; any other volume with that name, including one without a generation label, is rejected
until it is removed. The volume root must contain the corresponding binary as `/compiler`.

The service first creates a stopped job container, which pins its named cache volumes against
concurrent deletion. It then validates and, when necessary, seeds each volume before starting that
job. This cache phase, including any wait for another fill of the same compiler in the same process,
has a single deadline sized per distinct compiler, and the job container's expiry label covers that
deadline plus one run. Missing volumes are atomically recreated with the ownership and generation
labels from the mount configuration. The service hashes the remote compiler before trusting a new or changed volume.
A missing compiler is staged through a digest-specific initializer container, checked again on the
Docker VM, and atomically published before the job starts. Initializer names also serialize cache
fills across multiple verifier pods. Job containers mount these volumes read-only; normal job cleanup
retains them. To avoid cold-start transfers on a WAN link, warm the required compiler versions with
the SSH round-trip tests or representative verification requests before directing production traffic
to a new Docker host.

ZKsync standard JSON verification rejects a non-empty `settings.LLVMOptions` (an empty list is
accepted and ignored). Arbitrary LLVM options include
process-execution hooks and are not a safe public compiler interface. Contracts whose bytecode
depends on custom LLVM options are therefore not supported by this verifier.

Cached compiler volumes are retained indefinitely. Prefer pruning labeled
`scv-compiler-v1-sha256-*` volumes during drained maintenance, after the corresponding compiler
versions have been retired. If an unused volume is deleted during traffic, the next stopped job
recreates, validates, and seeds that cache generation before its compiler starts.

Remote initialization starts with a strict SSH host-key preflight: the target must already exist in
`known_hosts` and its key must match. Keep that file present and immutable for the pod lifetime.
Bollard 0.21 uses `accept-new` for the separate SSH sessions carrying Docker requests and exposes no
strict-host-key override; enforcing the same strict policy on every transport session requires an
upstream or connector change. Jobs carry an `org.blockscout.smart-contract-verifier.expires-at`
label; the service reaps expired jobs before the executor first becomes ready and every 30 seconds
afterward. Configure an independent timer on the dedicated Docker VM to force-remove expired labeled
containers as well, so jobs are reclaimed even while every verifier pod is down.

Compiler-backed endpoints fail to start when execution remains `disabled`. In Docker mode, static
configuration errors still fail startup, but the service does not contact the compiler VM until the
first compiler-readiness probe or compiler request. Transient SSH, Docker daemon, or runner-image
failures are request/readiness failures and are retried; they do not prevent Sourcify and the
server from starting. Orphan cleanup is best effort: a container that cannot be removed is logged
and counted, and retried by the periodic sweep. There is no Docker-to-native fallback. `native` remains available for
tests and local development only.

Use `/health` as the process-liveness probe. Use `/health?service=compiler` as the compiler dependency
readiness probe; it returns HTTP 503 without exposing connection details until strict SSH validation,
the Docker daemon, and the pinned runner image are available. The equivalent gRPC health request
uses `service = "compiler"`. Any other service name reports process liveness. Readiness is pod-wide in Kubernetes: a mixed pod
cannot be removed from compiler traffic while remaining routable for Sourcify. If Sourcify must stay
available during a compiler-VM outage, keep pod readiness on `/health`, monitor the compiler probe
separately, or deploy Sourcify and compiler-backed endpoints as separate workloads.

When the metrics endpoint is enabled, the compiler runner exports:

- `smart_contract_verifier_compiler_runner_jobs_total{executor,outcome}` for success, compiler
  failure, OOM, validation/limit/timeout/infrastructure errors, and canceled requests;
- `smart_contract_verifier_compiler_runner_operation_duration_seconds{executor,operation,family}`
  for total, queue, prepare, create, upload, execute, and cleanup timing;
- `smart_contract_verifier_compiler_runner_transfer_bytes_total{executor,direction,family}` for
  successfully transferred input/output bytes (Docker archive framing is included);
- `smart_contract_verifier_compiler_runner_jobs_current{executor,state}` and
  `smart_contract_verifier_compiler_runner_max_concurrent_jobs{executor}` for runner queue and slot
  utilization;
- `smart_contract_verifier_compiler_runner_active_containers{family}` for container lifecycles
  currently managed by the process; and
- `smart_contract_verifier_compiler_runner_orphan_cleanup_sweeps_total{trigger,outcome}` plus
  `smart_contract_verifier_compiler_runner_orphan_cleanup_containers_total{family,outcome}` for
  lazy initialization and periodic recovery.

All labels use fixed, low-cardinality values. Sum
`smart_contract_verifier_compiler_runner_max_concurrent_jobs{executor="shared"}` across healthy
replica scrape targets to verify configured runner-slot capacity. Its value comes from
`compilers.max_threads`, the single admission limit shared by compilations and compiler version
probes. The active-container gauge is process-local and resets on restart; use the orphan-cleanup
counters to observe recovery of containers left behind by a terminated process.

The repository includes ignored end-to-end transport/cache and real-Vyper tests. Set `SCV_TEST_DOCKER_ADDR`,
`SCV_TEST_DOCKER_RUNNER_IMAGE`, and optionally `SCV_TEST_DOCKER_KEY_PATH` and
`SCV_TEST_DOCKER_PLATFORM`, then run the `ssh_docker_round_trip` and
`ssh_docker_vyper_round_trip` tests with `--ignored` before deploying a new runner image or Docker
Engine version. Each test runs its compiler twice to cover a cold fill followed by a cache hit. The
Vyper test downloads the checksum-verified Linux amd64 release and compiles a standard-JSON contract
inside the isolated container. That real-Vyper test currently requires `linux/amd64`, matching the
architecture of the published Linux Vyper release binary.

### Environment variables

Besides configuration file, one could use environment variables
to configure the service. If case of overlapping, those values
overwrite values from configuration file.
Variables have a hierarchical nature which
corresponds to the hierarchy in configuration file.
Double underscore (`__`) is used as a separator. All variables should use
`SMART_CONTRACT_VERIFIER` as a prefix.

All available options for configuration through environment variables could be found at
[config/base.env](./config/base.env)

# Grpc Api

Grpc description of available methods could be found in [proto](../smart-contract-verifier-proto/proto/v2). 

# Http Api

Swagger description is available in [swagger](../smart-contract-verifier-proto/swagger/v2/smart-contract-verifier.swagger.yaml).  

## Solidity Multi-Part files

### Route
`POST /api/v2/verifier/solidity/sources:verify-multi-part`

### Input

```json5
{
  // Bytecode to compare local compilation result with
  "bytecode": "0x608060...0033000b0c",
  // Either "CREATION_INPUT" or "DEPLOYED_BYTECODE", depending on what should be verified
  "bytecodeType": "CREATION_INPUT",
  // Compiler version used to compile the contract
  "compilerVersion": "v0.8.14+commit.80d49f37",
  // (optional) Version of the EVM to compile for. 
  // If absent results in default EVM version
  "evmVersion":  "default",
  // (optional) If present, optimizations are enabled with specified number of runs,
  // otherwise optimizations are disabled
  "optimizationRuns": 200,
  // Map from a source file name to the actual source code
  "sourceFiles": {
    "A.sol": "pragma solidity ^0.8.14; contract A {}",
    "B.sol": "pragma solidity ^0.8.14; contract B {}"
  },
  // Map from a library name to its address
  "libraries": {
    "MyLib": "0x123123..."
  }
}
```

## Solidity Standard-JSON input

### Route
`POST /api/v2/verifier/solidity/sources:verify-standard-json`

### Input
```json5
{
  // Bytecode to compare local compilation result with
  "bytecode": "0x608060...0033000b0c",
  // Either "CREATION_INPUT" or "DEPLOYED_BYTECODE", depending on what should be verified
  "bytecodeType": "CREATION_INPUT",
  // Compiler version used to compile the contract
  "compilerVersion": "v0.8.14+commit.80d49f37",
  // https://docs.soliditylang.org/en/latest/using-the-compiler.html#input-description
  "input": "{\"language\": \"Solidity\",\"sources\": { ... }, \"settings\": { ... }}"
}
```

## Vyper Multi-Part files

### Route
`POST /api/v2/verifier/vyper/sources:verify-multi-part`

### Input
```json5
{
  // Bytecode to compare local compilation result with
  "bytecode": "0x608060...0033000b0c",
  // Either "CREATION_INPUT" or "DEPLOYED_BYTECODE", depending on what should be verified
  "bytecodeType": "CREATION_INPUT",
  // Compiler version used to compile the contract
  "compilerVersion": "0.3.6+commit.4a2124d0",
  // (optional) Version of the EVM to compile for. 
  // If absent results in default EVM version
  "evmVersion":  "istanbul",
  // (optional) Flag enabling optimizations. If absent, default value is `true`
  "optimizations": "true",
  // Source file name to the actual source code
  "sourceFiles": {
    "A.vy": "# @version ^0.3.6\r\n\r\nuserName: public(String[100])\r\n\r\n@external\r\ndef __init__(name: String[100]):\r\n    self.userName = name\r\n\r\n@view\r\n@external\r\ndef getUserName() -> String[100]:\r\n    return self.userName\r\n"
  }
}
```

## Sourcify
Proxies verification requests to Sourcify service and returns responses (https://docs.sourcify.dev/docs/api/server/v1/verify/).

### Route
`POST /api/v2/verifier/sourcify/sources:verify`

### Input
```json5
{
  // Address of the contract to be verified 
  "address": "0xcafecafecafecafecafecafecafecafecafecafe",
  // The chain (network) the contract was deployed to 
  // (https://docs.sourcify.dev/docs/api/chains/)
  "chain": "100",
  // Files required for verification (see Sourcify Api)
  "files": {
    "A.sol": "pragma solidity ^0.8.14; contract A {}",
    "B.sol": "pragma solidity ^0.8.14; contract B {}",
    // https://docs.soliditylang.org/en/v0.8.14/metadata.html
    "metadata.json": "{ ... }"
  },
  // (optional) see Sourcify Api
  "chosenContract": 1
}
```

## Outputs
All verification requests have the same response format.

### Success
If verification succeeds, the service returns 200 with a success status:
```json5
{
  "message": "OK",
  "status": "SUCCESS", 
  "source": {
    // The name of the file verified contract was located at
    "fileName":  "A.sol",
    // The name of the contract which was verified
    "contractName": "A",
    // Compiler version used to compile the contract
    "compilerVersion": "v0.8.14+commit.80d49f37",
    // 'settings' key in Standard Input JSON
    // (https://docs.soliditylang.org/en/latest/using-the-compiler.html#input-description)
    "compilerSettings": "{ ... }",
    // One of "SOLIDITY", "VYPER", or "YUL". 
    // "SOURCE_TYPE_UNSPECIFIED" is also an option, but should be considered invalid by the clients. 
    "sourceType": "SOLIDITY",
    "sourceFiles": {
      "A.sol": "pragma solidity ^0.8.14; contract A {}",
      "B.sol": "pragma solidity ^0.8.14; contract B {}" 
    },
    // (optional) Contract abi (https://docs.soliditylang.org/en/latest/abi-spec.html?highlight=abi#json);
    // (does not exist for Yul contracts)
    "abi":  "[ { ... } ]";
    // (optional) Constructor arguments used for deploying verified contract
    "constructorArguments": "0xcafecafecafe",
    // Either "PARTIAL" or "FULL".
    // Similar to Sourcify (see https://docs.sourcify.dev/docs/full-vs-partial-match/)
    "matchType": "PARTIAL",
  },
  "extraData": {
    // Creation transaction input resultant from local compilation
    // parsed and split on Main and Meta parts. 
    // Is empty for Sourcify verification.
    "localCreationInputParts": [
      { "type": "main", "data": "0x1234.." },
      { "type": "meta", "data": "0xcafe.." }
    ],
    // Deployed bytecode resultant from local compilation
    // parsed and split on Main and Meta parts. 
    // Is empty for Sourcify verification.
    "localDeployedBytecodeParts": [
      { "type": "main", "data": "0x1234.." },
      { "type": "meta", "data": "0xcafe.." }
    ]
  }
}
```

### Verification Failure
If verification fails because of invalid verification data provided to it from outside,
the service returns 200 with the failure status:
```json5
{
  // Message indicating the reason for failure
  "message": "Compilation error: contracts/3_Ballot.sol:4:1: ParserError: Expected pragma, import directive or contract/interface/library/struct/enum/constant/function definition.\n12312313vddfvfdvfd\n^------^",
  // Non "SUCCESS" statuses indicate errors (currently only "FAILURE" is possible)
  "status": "FAILURE"
}
```

### Bad Request
There are data whose validity the requester is responsible to ensure.
That includes the bytecode to be a valid not-empty hex, the bytecode type to be
either "CREATION_INPUT" or "DEPLOYED_BYTECODE", and the compiler version to be valid.

In case any of that arguments are invalid, the service return 400 BadRequest error,
indicating that something is wrong with the caller.

## Version List

### Route
`GET /api/v2/verifier/solidity/versions`

### Input
No input required

### Output

```json5
{
  // List of all available versions in descending order
  "compilerVersions": ["0.8.15-nightly.2022.5.27+commit.095cc647","0.8.15-nightly.2022.5.25+commit.fdc3c8ee",..]
}
```

### Route
`GET /api/v2/verifier/vyper/versions`

### Input
No input required

### Output

```json5
{
  // List of all available versions in descending order
  "compilerVersions": ["v0.3.6+commit.4a2124d0","v0.3.4+commit.f31f0ec4",..]
}
```

# Compiler Settings (transition)
In the previous version the verifier partially parsed compiler settings and explicitly returned some of its values.
That included `evm_version`, `optimization`, `optimization_runs`, and `contract_libraries`. 
In the new version those keys have been removed, as all of them could be obtained by the client from `compiler_settings`. 
In this section we will describe how the client can parse that value.

Below is the description of settings from [Solidity docs](https://docs.soliditylang.org/en/v0.8.17/using-the-compiler.html#input-description) with the most related parts:
```json5
{
  // Optional: Optimizer settings
  "optimizer": {
    // Disabled by default.
    // NOTE: enabled=false still leaves some optimizations on. See comments below.
    // WARNING: Before version 0.8.6 omitting the 'enabled' key was not equivalent to setting
    // it to false and would actually disable all the optimizations.
    "enabled": true,
    // Optimize for how many times you intend to run the code.
    // Lower values will optimize more for initial deployment cost, higher
    // values will optimize more for high-frequency usage.
    "runs": 200,
  },
  // Version of the EVM to compile for.
  // Affects type checking and code generation. Can be homestead,
  // tangerineWhistle, spuriousDragon, byzantium, constantinople, petersburg, istanbul or berlin
  "evmVersion": "byzantium",
  // Addresses of the libraries. If not all libraries are given here,
  // it can result in unlinked objects whose output data is different.
  "libraries": {
    // The top level key is the name of the source file where the library is used.
    // If remappings are used, this source file should match the global path
    // after remappings were applied.
    // If this key is an empty string, that refers to a global level.
    "myFile.sol": {
      "MyLib": "0x123123..."
    }
  },
}
```

The process of parsing consists of trying to obtain required keys, and if they are missing using the default values instead:
1. `optimization` - either `compilerSettings[optimizer][enabled]`; or `null`, if any key is missed.
2. `optimization_runs` - if `optimization=true`, then either `compilerSettings[optimizer][runs]` (if exists) or `200`. Otherwise, `null`. 
3. `evm_version` - either `compilerSettings[evmVersion]` (if exists) or `default`.
4. `contract_libraries` - `compilerSettings[libraries]` (if exists) or `{}`.
