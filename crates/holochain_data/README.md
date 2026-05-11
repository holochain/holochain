# Holochain Data

A wrapper around sqlx configured for Holochain's needs, providing SQLite database connections with encryption, migrations, and query patterns.

## Features

- **SQLCipher encryption** - Full database encryption with secure key management
- **WAL mode** - Write-Ahead Logging enabled for better concurrency
- **Per-database-type migrations** - Separate migration sets for each database kind
- **Connection pooling** - Automatic pool sizing based on CPU count
- **Configurable sync levels** - Control SQLite's durability guarantees

## Database Types

Each database type has its own migration set under `migrations/`:

| Kind | Migration directory | Purpose |
|------|-------------------|---------|
| `Wasm` | `migrations/wasm/` | WASM bytecode, DNA definitions, entry definitions |
| `Conductor` | `migrations/conductor/` | Conductor state, installed apps, interfaces, nonces, blocks |

Each `DatabaseIdentifier` implementation returns a `DbKind` which selects the appropriate migration set at database open time. Additional database kinds (Authored, Dht, PeerMetaStore) will get their own migration directories when their schemas are added.

## Query Patterns

sqlx provides several approaches for mapping Rust types to database queries. See `src/example.rs` for compile-time checked examples.

### 1. `query_as` with `FromRow` derive

The most ergonomic approach for most use cases:

```rust
use sqlx::FromRow;

#[derive(FromRow)]
struct WasmRow {
    hash: Vec<u8>,
    code: Vec<u8>,
}

let data = sqlx::query_as::<_, WasmRow>(
    "SELECT hash, code FROM Wasm WHERE hash = ?"
)
.bind(hash)
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
let row = sqlx::query("SELECT hash, code FROM Wasm WHERE hash = ?")
    .bind(hash)
    .fetch_one(&pool)
    .await?;

let hash: Vec<u8> = row.get(0);
let code: Vec<u8> = row.get(1);
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
    WasmRow,
    r#"SELECT hash as "hash!", code as "code!" FROM Wasm WHERE hash = ?"#,
    hash
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

### Preparing query metadata

Since migrations are split per database type, `sqlx prepare` needs a combined database containing all schemas. Create it by applying each migration set to a temporary database:

```bash
cd crates/holochain_data

# Build a combined database for prepare
rm -f /tmp/holochain_prepare.db
sqlite3 /tmp/holochain_prepare.db < migrations/wasm/*.up.sql
sqlite3 /tmp/holochain_prepare.db < migrations/conductor/*.up.sql

# Generate query metadata for offline compilation
DATABASE_URL=sqlite:///tmp/holochain_prepare.db cargo sqlx prepare
```

The `.sqlx/` directory contains query metadata and should be committed to version control.

### Verifying query metadata is up to date

```bash
DATABASE_URL=sqlite:///tmp/holochain_prepare.db cargo sqlx prepare --check
```

## CI Integration

In CI, queries are verified without needing a database connection:

```bash
# Just check that queries are valid (uses committed .sqlx/ metadata)
cargo check -p holochain_data
```

When schema or queries change, developers must run `cargo sqlx prepare` locally and commit the updated `.sqlx/` files.

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
use holochain_data::{open_db, HolochainDataConfig};

// Set up database with encryption and a pool with 4 readers.
let key = DbKey::generate(passphrase).await?;
let config = HolochainDataConfig::new()
    .with_key(key)
    .with_sync_level(DbSyncLevel::Normal)
    .with_max_readers(4);

let db = open_db(path, db_id, config).await?;

// Migrations for the database kind are applied automatically
// Now use the connection pool for queries
```

See `tests/integration.rs` for comprehensive examples.
