# Holochain ORM

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

**Note:** These require a database connection at compile time and a `DATABASE_URL` environment variable.

```rust
// Requires compile-time database connection
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

**Cons:**
- Requires database connection during compilation
- More complex build setup
- Not suitable for Holochain's use case (can't assume dev database available)

## Recommendation for Holochain

Use **`query_as` with `FromRow` derive** (#1) as the default pattern:

- Provides good ergonomics without compile-time database requirements
- Type-safe with explicit struct definitions
- Works well with Holochain's build process
- Easy to test and maintain

Use **manual `Row` access** (#2) for:
- Dynamic queries where column set isn't known
- Simple utility queries that don't warrant a struct
- Performance-critical code where you need fine control

Avoid **compile-time macros** (#3) because:
- Holochain builds in various environments without access to a development database
- The embedded migrations approach already provides schema documentation
- Runtime query validation is sufficient for our needs

## Example Usage

```rust
use holochain_orm::{setup_holochain_orm, HolochainOrmConfig};

// Set up database with encryption
let key = DbKey::generate(passphrase).await?;
let config = HolochainOrmConfig::new()
    .with_key(key)
    .with_sync_level(DbSyncLevel::Normal);

let db = setup_holochain_orm(path, db_id, config).await?;

// Migrations run automatically
// Now use the connection pool for queries
```

See `tests/integration.rs` for comprehensive examples.
