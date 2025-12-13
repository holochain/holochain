# Holochain Data

A wrapper around sqlx configured for Holochain's needs, providing SQLite database connections with encryption, migrations, and query patterns.

## Features

- **SQLCipher encryption** - Full database encryption with secure key management
- **WAL mode** - Write-Ahead Logging enabled for better concurrency
- **Embedded migrations** - Migration files compiled into the binary
- **Connection pooling** - Automatic pool sizing based on CPU count
- **Configurable sync levels** - Control SQLite's durability guarantees

## Query Patterns

sqlx provides several approaches for mapping Rust types to database queries. See `src/example.rs` for detailed examples.

### 1. `query_as` with `FromRow` derive (Recommended)

The most ergonomic approach for most use cases:

```rust
use sqlx::FromRow;

#[derive(FromRow)]
struct SampleData {
    id: i64,
    name: String,
    value: Option<String>,
}

let data = sqlx::query_as::<_, SampleData>(
    "SELECT id, name, value FROM sample_data WHERE id = ?"
)
.bind(id)
.fetch_one(&pool)
.await?;
```

**Pros:**
- Automatic mapping from columns to struct fields
- Type-safe with clear struct definitions
- Good balance of ergonomics and flexibility

**Cons:**
- Column names must match struct field names (or use `#[sqlx(rename = "...")]`)
- Runtime column mapping (no compile-time verification)

### 2. Manual `Row` access

Direct access to row data by index:

```rust
let row = sqlx::query("SELECT id, name FROM sample_data WHERE id = ?")
    .bind(id)
    .fetch_one(&pool)
    .await?;

let id: i64 = row.get(0);
let name: String = row.get(1);
```

**Pros:**
- Maximum flexibility
- No struct definitions needed for simple queries
- Can handle dynamic column sets

**Cons:**
- Easy to make mistakes with column indices
- Less type-safe
- More verbose

### 3. Compile-time checked macros (`query!` / `query_as!`)

**Recommended approach** - Provides compile-time SQL verification using offline prepared queries.

```rust
let data = sqlx::query_as!(
    SampleData,
    "SELECT id, name, value FROM sample_data WHERE id = ?",
    id
)
.fetch_one(&pool)
.await?;
```

**Pros:**
- Compile-time verification of queries against actual schema
- Type inference from database
- Catches SQL errors at compile time
- Works offline with prepared query metadata (`.sqlx/` directory)

**Cons:**
- Requires running `cargo sqlx prepare` when schema changes
- Additional `.sqlx/` directory must be committed to version control

## Development Setup

For compile-time query verification to work, you need to maintain prepared query metadata:

```bash
# Initial setup (or after schema changes)
cd crates/holochain_data

# Create/update the database schema
DATABASE_URL=sqlite:$(pwd)/dev.db sqlx database create
DATABASE_URL=sqlite:$(pwd)/dev.db sqlx migrate run

# Generate query metadata for offline compilation
DATABASE_URL=sqlite:$(pwd)/dev.db cargo sqlx prepare -- --lib
```

The `.sqlx/` directory contains query metadata and should be committed to version control.

**Note:** The `DATABASE_URL` environment variable must point to the development database using inline syntax as shown above.

## CI Integration

In CI, queries are verified without needing a database connection:

```bash
# Just check that queries are valid (uses committed .sqlx/ metadata)
cargo check -p holochain_data
```

When schema changes, developers must run `cargo sqlx prepare` locally and commit the updated `.sqlx/` files.

## Recommendation for Holochain

Use **compile-time checked macros** (#3) as the default pattern:

- Catches SQL errors at compile time
- Type inference from actual database schema
- Works offline in CI using prepared query metadata
- No runtime cost for query verification

Use **`query_as` with `FromRow` derive** (#1) for:
- Queries that need to be constructed dynamically
- Situations where compile-time checking isn't practical

Use **manual `Row` access** (#2) only for:
- Dynamic queries where column set isn't known
- Simple utility queries that don't warrant a struct
- Performance-critical code where you need fine control

## Example Usage

```rust
use holochain_data::{setup_holochain_data, HolochainDataConfig};

// Set up database with encryption
let key = DbKey::generate(passphrase).await?;
let config = HolochainDataConfig::new()
    .with_key(key)
    .with_sync_level(DbSyncLevel::Normal);

let db = setup_holochain_data(path, db_id, config).await?;

// Migrations run automatically
// Now use the connection pool for queries
```

See `tests/integration.rs` for comprehensive examples.

## Validating SQL Queries

This crate uses sqlx's compile-time query checking to validate all SQL queries against the schema. The `.sqlx/` directory contains prepared query metadata that allows offline verification.

To regenerate the query metadata after schema or query changes:

```bash
cd crates/holochain_data
DATABASE_URL=sqlite:$(pwd)/dev.db sqlx database create
DATABASE_URL=sqlite:$(pwd)/dev.db sqlx migrate run
DATABASE_URL=sqlite:$(pwd)/dev.db cargo sqlx prepare -- --lib
```

In CI, queries are validated using the committed `.sqlx/` metadata without requiring a live database connection.
