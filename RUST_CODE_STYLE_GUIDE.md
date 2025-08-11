Rust Code Style Guide
===

## Table of Contents
1. [General Principles](#general-principles)
1. [Code Formatting](#code-formatting)
1. [Naming Conventions](#naming-conventions)
1. [Project Structure](#project-structure)
1. [Error Handling](#error-handling)
1. [Logging and Tracing](#logging-and-tracing)
1. [Database Patterns](#database-patterns)
1. [Testing](#testing)
1. [Dependencies and Cargo](#dependencies-and-cargo)
1. [Documentation](#documentation)

## General Principles

### Code Quality Standards
- **Functional Approach**: Prefer functional patterns like map/filter over imperative loops and mutable state
- **Immutability**: Use immutable variables by default, only make mutable when necessary
- **Zero Warnings**: All code MUST compile without warnings
- **Clippy Compliance**: Use `cargo clippy --all --all-targets --all-features -- -D warnings`
- **Documentation**: All public APIs SHOULD be documented with examples showing functional usage
- **Error Handling**: Use Result/Option types and combinators like map/and_then/map_err for error handling
- **Performance**: Write efficient code but prioritize readability and functional purity
- **Consistent Style**: Code SHOULD follow functional patterns consistently throughout codebase
- **Safety**: Avoid unsafe code. If unsafe is needed, document safety invariants and use functional wrappers


### Rust Edition
- Use **Rust 2024 edition** for all new projects
- Target **stable Rust** toolchain

## Code Formatting

### Rustfmt Configuration
Use the following rustfmt configuration:
```bash
cargo fmt --all -- --config imports_granularity=Crate
```

and clippy:
```bash
cargo clippy --all --all-targets --all-features -- -D warnings
```

### Import Organization

Put all imports without new lines so that cargo fmt can format them properly:

```rust
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
    str::Lines,
    sync::Arc,
};
use anyhow::{anyhow, Context, Result};
use sea_orm::{DatabaseConnection, DbErr};
use thiserror::Error;
use tokio::time::sleep;
use crate::{
    database::TacDatabase,
    types::{ChainId, api_keys::ApiKeyError},
};
```

### Code Style Rules
- **Line length**: 100 characters maximum
- **Indentation**: 4 spaces
- **Braces**: Opening brace on the same line, closing brace on its own line
- **Spacing**: One space around operators, after commas, before opening braces

## Naming Conventions

### Variables and Functions
- **snake_case** for variables, functions, and modules
- **SCREAMING_SNAKE_CASE** for constants
- **PascalCase** for types, traits, and enums
- Short or single letter variables are allowed only in one-line closures like:

```rust
// Good
const MAX_RETRY_ATTEMPTS: usize = 3;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

struct UserOperation {
    sender: Address,
    nonce: U256,
}

async fn process_user_operation(op: &UserOperation) -> Result<(), Error> {
    // Implementation
}

// Bad
const maxRetryAttempts: usize = 3;
const default_timeout: Duration = Duration::from_secs(30);

struct userOperation {
    Sender: Address,
    Nonce: U256,
}

async fn ProcessUserOperation(op: &userOperation) -> Result<(), Error> {
    // Implementation
}

// Good
let ids = objects
    .into_iter()
    .map(|x| x.id);

// Also fine
let ids = objects
    .into_iter()
    .map(|object| object.id);

// Bad
let o = get_objects();
let mut i = o.into_iter().filter().map();
let l = i.pop();
```

### Database and API Naming
- **snake_case** for database columns and API endpoints
- **PascalCase** for database tables and API models
- Use descriptive names that clearly indicate purpose

```rust
// Database entities
#[derive(FromQueryResult)]
pub struct AccountDB {
    pub address: Vec<u8>,
    pub factory: Option<Vec<u8>>,
    pub creation_transaction_hash: Option<Vec<u8>>,
    pub total_ops: i64,
}

// API endpoints
// GET /api/v1/accounts/{address}
// POST /api/v1/userOps
```

### Basic CRUD function names

- `get_` functions take an identifier and return the object; they must return an error if the object does not exist
  - `get_book(id: String) -> Result<Book, Error>`
- `find_` function is `get_` function but can return None in case object doesn't exist
  - `find_book(id: String) -> Result<Option<Book>, Error>`
- `list_` or `search_` functions can take some filter params and return list of objects. It can also return pagination information
  - `search_books(filter: BookFilter) -> Result<Vec<Book>, Error>`  
  - `list_books(filter: BookFilter) -> Result<(Vec<Book>, PaginationResult), Error>`
- `delete_` function should take identifier and return Result<(), Error>
  - `delete_book(id: String) -> Result<(), Error>`
- `update_` function should take identifier and update params and return updated object
  - `update_book(id: String, params: UpdateBookParams) -> Result<Book, Error>`
- `create_` function should take creation params and return created object
  - `create_book(params: CreateBookParams) -> Result<Book, Error>`

## Project Structure

### Service Layout
Follow the established pattern for service organization:

```txt
{service-name}/
├── {service-name}-proto/          # gRPC protocol definitions
│   ├── proto/                     # .proto files
│   ├── src/                       # Generated Rust code
|   ├── build.rs   # description of Rust code generation
│   └── Cargo.toml
├── {service-name}-logic/          # Business logic implementation
│   ├── src/
│   │   ├── lib.rs
│   │   ├── error.rs
│   │   ├── types.rs
│   │   └── ...
│   └── Cargo.toml
├── {service-name}-server/         # Server implementation
│   ├── src/
│   │   ├── main.rs
│   │   ├── server.rs
│   │   └── ...
│   └── Cargo.toml
├── {service-name}-entity/         # Database entities (if needed)
├── {service-name}-migration/      # Database migrations (if needed)
├── types/                         # TypeScript types for frontend team (if needed)
├── docker-compose.yml
├── Dockerfile
└── Cargo.toml                     # Workspace root
```

### Module Organization
```rust
// lib.rs - Main library entry point
pub mod error;
pub mod types;
pub mod database;
pub mod indexer;
pub mod repository;

pub use error::*;
pub use types::*;

// error.rs - Error definitions
#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("api key error: {0}")]
    ApiKey(#[from] ApiKeyError),
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
    // ...
}

// types.rs - Type definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserOperation {
    pub hash: TxHash,
    pub sender: Address,
    pub nonce: U256,
    // ...
}
```

## Error Handling

### Error Types
Use `thiserror` for custom error types:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("api key error: {0}")]
    ApiKey(#[from] ApiKeyError),
    #[error("convert error: {0}")]
    Convert(#[from] ParseError),
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
    #[error("external api error: {0}")]
    ExternalApi(#[from] api_client_framework::Error),
    #[error("db error: {0}")]
    Db(#[from] DbErr),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("cache error: {0}")]
    Cache(#[from] CacheRequestError<RedisStoreError>),
}

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("parse error: invalid integer")]
    ParseInt(#[from] ParseIntError),
    #[error("parse error: invalid hex")]
    ParseHex(#[from] FromHexError),
    #[error("parse error: {0}")]
    Custom(String),
}
```

### Error Conversion
Implement `From` traits for error conversion:

> NOTE: probably in production run we need to hide error message from user, log it and show to user some basic info

```rust
impl From<ServiceError> for tonic::Status {
    fn from(error: ServiceError) -> Self {
        let code = match &error {
            ServiceError::NotFound(_) => Code::NotFound,
            ServiceError::ApiKey(_) => Code::Unauthenticated,
            ServiceError::Convert(_) => Code::InvalidArgument,
            _ => Code::Internal,
        };
        tonic::Status::new(code, error.to_string())
    }
}
```

### Result Handling
Use `anyhow::Result` for internal functions and custom error types for public APIs:

```rust
// Internal function
async fn process_data(data: &[u8]) -> Result<Vec<u8>, anyhow::Error> {
    // Implementation
}

// Public API
async fn get_user_operation(hash: TxHash) -> Result<UserOperation, ServiceError> {
    let data = process_data(&hash.as_slice())
        .await
        .map_err(ServiceError::Internal)?;
    
    UserOperation::try_from(data)
        .map_err(ServiceError::Convert)
}
```

## Logging and Tracing

### Tracing Setup
Use the `blockscout-service-launcher` for consistent tracing setup:

```rust
use blockscout_service_launcher::tracing::{init_logs, JaegerSettings, TracingSettings};

pub async fn run(settings: Settings) -> Result<(), anyhow::Error> {
    let tracing_settings = TracingSettings {
        enabled: true,
        format: TracingFormat::Json,
    };
    
    let jaeger_settings = JaegerSettings {
        enabled: settings.jaeger_enabled,
        agent_endpoint: settings.jaeger_endpoint,
    };
    
    init_logs("service-name", &tracing_settings, &jaeger_settings)?;
    
    // Service implementation
    Ok(())
}
```

### Logging Levels
- **ERROR**: System errors, failures that need immediate attention
- **WARN**: Unexpected but recoverable situations
- **INFO**: Important business events, service lifecycle
- **DEBUG**: Detailed debugging information
- **TRACE**: Very detailed debugging information

```rust

#[instrument(skip(self))]
async fn process_job(&self, job: &Job) -> Result<(), Error> {
    tracing::debug!(job_type = ?job.job_type(), "Processing job");
    
    match self.process(job).await {
        Ok(result) => {
            tracing::info!(job_id = ?job.id(), "Job completed successfully");
            Ok(result)
        }
        Err(err) => {
            tracing::error!(job_id = ?job.id(), error = ?err, "Job processing failed");
            Err(err)
        }
    }
}
```

### Static Logging Messages
Use static logging messages and move dynamic information to fields:

```rust
// Good
tracing::info!(
    current_realtime_timestamp,
    concurrency = ?self.settings.concurrency,
    "Starting indexing stream"
);

// Bad
tracing::error!(
    "Starting indexing stream with concurrency = {concurrency:?} and current_realtime_timestamp = {current_realtime_timestamp:?}"
);
```

## Database Patterns

### SeaORM Usage
Use SeaORM for database operations with proper error handling:

```rust
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection,
    EntityTrait, QueryFilter, QuerySelect, Set,
};

pub async fn find_account_by_address(
    db: &DatabaseConnection,
    addr: Address,
) -> Result<Option<Account>, anyhow::Error> {
    let account = account::Entity::find()
        .filter(account::Column::Address.eq(addr.as_slice()))
        .one(db)
        .await?;
    
    Ok(account.map(Account::from))
}

pub async fn create_account(
    db: &DatabaseConnection,
    account_data: AccountData,
) -> Result<Account, anyhow::Error> {
    let account_model = account::ActiveModel {
        address: Set(account_data.address.as_slice().to_vec()),
        factory: Set(account_data.factory.map(|f| f.as_slice().to_vec())),
        ..Default::default()
    };
    
    let result = account_model.insert(db).await?;
    Ok(Account::from(result))
}
```

### Database Transactions
Use transactions for operations that require atomicity:

```rust
use sea_orm::{DatabaseTransaction, TransactionTrait};

pub async fn process_batch(
    db: &DatabaseConnection,
    operations: Vec<Operation>,
) -> Result<(), anyhow::Error> {
    let tx = db.begin().await?;
    
    for operation in operations {
        process_operation(&tx, operation).await?;
    }
    
    tx.commit().await?;
    Ok(())
}
```

### Migration Management
Use SeaORM migrations for database schema changes, but prefer raw SQL

```rust
// In migration files
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(manager, include_str!("migrations_up/m20220101_000001_initial_up.sql")).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(manager, include_str!("migrations_down/m20220101_000001_initial_down.sql")).await?;
        Ok(())
    }
}
```

## Testing

* **Unit tests** -- Place unit tests in the same file as the code being tested:
    ```rust

    pub fn parse_address(...) {
        ...
    }

    pub async async_function() {
        ...
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        
        #[test]
        fn test_parse_address() {
            let input = "0x1234567890123456789012345678901234567890";
            let result = parse_address(input);
            assert!(result.is_ok());
        }
        
        #[tokio::test]
        async fn test_async_function() {
            let result = async_function().await;
            assert!(result.is_ok());
        }
    }
    ```
* **Integration tests** -- Create integration tests in the `tests/` directory


## Dependencies and Cargo

### Cargo.toml Structure
Define all your dependencies in the Cargo workspace and use `{ workspace = true }` in crates:

```toml
# Root Cargo.toml
[workspace]
members = [
    "service-name-proto",
    "service-name-logic", 
    "service-name-server",
    "service-name-entity",
    "service-name-migration",
]

[workspace.dependencies]
anyhow = "1.0"
thiserror = "2.0"
tokio = { version = "1.45", features = ["rt", "macros"] }
blockscout-service-launcher = { version = "0.20.0" }
```

```toml
# Cargo.toml of a crate
[package]
name = "service-name-logic"
version = "0.1.0"
edition = "2021"

[dependencies]
blockscout-service-launcher = { workspace = true, features = ["database-1"] }
url = { workspace = true }
reqwest = { workspace = true }
```


## Documentation

### Code Documentation

One SHOULD document all public APIs with doc comments:

```rust
/// Represents a user operation in the ERC-4337 standard.
/// 
/// User operations are the primary way users interact with account abstraction
/// contracts. They contain the necessary data for executing transactions
/// on behalf of the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserOperation {
    /// The unique hash of this user operation
    pub hash: TxHash,
    /// The address of the sender account
    pub sender: Address,
    /// The nonce for this operation
    pub nonce: U256,
    /// The call data to be executed
    pub call_data: Bytes,
}

impl UserOperation {
    /// Creates a new user operation with the given parameters.
    /// 
    /// # Arguments
    /// 
    /// * `sender` - The address of the sender account
    /// * `nonce` - The nonce for this operation
    /// * `call_data` - The call data to be executed
    /// 
    /// # Returns
    /// 
    /// A new `UserOperation` instance
    pub fn new(sender: Address, nonce: U256, call_data: Bytes) -> Self {
        // Implementation
    }
    
    /// Validates the user operation.
    /// 
    /// # Returns
    /// 
    /// `Ok(())` if the operation is valid, `Err(ValidationError)` otherwise
    pub fn validate(&self) -> Result<(), ValidationError> {
        // Implementation
    }
}
```
