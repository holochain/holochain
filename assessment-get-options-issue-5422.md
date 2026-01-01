# Assessment: GetOptions Support for All HDK Data Fetching Functions (Issue #5422)

## Executive Summary

Currently, HDK data fetching functions have limited control over network request behavior. The `GetOptions` struct only contains `strategy: GetStrategy` (Network/Local), but `NetworkRequestOptions` has additional important fields (`remote_agent_count`, `timeout_ms`, `as_race`) that should be configurable from the HDK. This assessment outlines the changes needed to expose these network request options to app developers.

## Current State Analysis

### What GetOptions Currently Contains

```rust
// crates/holochain_zome_types/src/entry.rs
pub struct GetOptions {
    pub strategy: GetStrategy,  // Only Network or Local
}
```

### What NetworkRequestOptions Contains (Not Exposed to HDK)

```rust
// crates/holochain_p2p/src/types/actor.rs
pub struct NetworkRequestOptions {
    /// Make requests to this number of remote agents in parallel.
    /// Defaults to `3`
    pub remote_agent_count: u8,
    
    /// Timeout within which responses must arrive.
    /// When `None`, conductor settings determine the timeout.
    pub timeout_ms: Option<u64>,
    
    /// Whether to treat the get as a race, returning the first response.
    /// Defaults to `true`
    pub as_race: bool,
}
```

### Current Cascade Implementation

When the Cascade converts `GetOptions` to `CascadeOptions`, it uses hardcoded defaults:

```rust
// crates/holochain_cascade/src/lib.rs
CascadeOptions {
    network_request_options: NetworkRequestOptions::default(),  // Always uses defaults!
    get_options: options,  // Only strategy is preserved
}
```

This means app developers cannot control:
- How many agents to query (always 3)
- Request timeout (always uses conductor defaults)
- Whether to race or aggregate responses (always races)

### Functions That Currently Support GetOptions (But Only Strategy)

1. **`get()` and `get_details()`** (crates/hdk/src/entry.rs)
2. **`get_links()` and `get_links_details()`** (crates/hdk/src/link.rs)

### Functions That DON'T Support GetOptions At All

1. **`get_agent_activity()`** (crates/hdk/src/chain.rs)
   - Takes `ActivityRequest` parameter but no `GetOptions`
   - Cannot control network behavior at all

### Functions That SHOULDN'T Support GetOptions (By Design)

The HDI validation functions (`must_get_*`) need deterministic, system-controlled behavior and shouldn't support GetOptions.

## Breaking Changes Assessment

### Required Breaking Changes

#### 1. Expand GetOptions Struct with Private Fields

```rust
// crates/holochain_zome_types/src/entry.rs (or move to holochain_integrity_types)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetOptions {
    /// Network or Local strategy
    strategy: GetStrategy,
    
    /// Number of remote agents to query in parallel (1-10)
    /// Only used when strategy is Network
    remote_agent_count: Option<u8>,  // None = use default (3)
    
    /// Timeout for network requests in milliseconds
    /// Only used when strategy is Network
    timeout_ms: Option<u64>,  // None = use conductor settings
    
    /// Whether to race (first response) or aggregate responses
    /// Only used when strategy is Network
    as_race: Option<bool>,  // None = use default (true)
}

impl GetOptions {
    /// Get the strategy for this request
    pub fn strategy(&self) -> GetStrategy {
        self.strategy
    }
    
    /// Get the number of remote agents to query
    pub fn remote_agent_count(&self) -> Option<u8> {
        self.remote_agent_count
    }
    
    /// Get the timeout in milliseconds
    pub fn timeout_ms(&self) -> Option<u64> {
        self.timeout_ms
    }
    
    /// Get whether to race or aggregate responses
    pub fn as_race(&self) -> Option<bool> {
        self.as_race
    }
    
    /// Create options for network strategy with defaults
    pub fn network() -> Self {
        Self {
            strategy: GetStrategy::Network,
            remote_agent_count: None,
            timeout_ms: None,
            as_race: None,
        }
    }
    
    /// Create options for local-only strategy
    pub fn local() -> Self {
        Self {
            strategy: GetStrategy::Local,
            remote_agent_count: None,
            timeout_ms: None,
            as_race: None,
        }
    }
    
    /// Set the number of remote agents to query (1-10)
    pub fn with_remote_agent_count(mut self, count: u8) -> Self {
        self.remote_agent_count = Some(count.min(10));
        self
    }
    
    /// Set the timeout in milliseconds
    pub fn with_timeout_ms(mut self, timeout: u64) -> Self {
        self.timeout_ms = Some(timeout);
        self
    }
    
    /// Set to aggregate responses instead of racing
    /// Note: Aggregation mode is not yet implemented
    pub fn with_aggregation(mut self) -> Self {
        self.as_race = Some(false);
        self
    }
}

impl Default for GetOptions {
    fn default() -> Self {
        Self::network()
    }
}
```

**Breaking Change**: Code that directly accesses `options.strategy` will need to use `options.strategy()` instead.

#### 2. Update get_agent_activity Signature

```rust
// HDK function (crates/hdk/src/chain.rs)
pub fn get_agent_activity(
    agent: AgentPubKey,
    query: ChainQueryFilter,
    request: ActivityRequest,
    get_options: GetOptions,  // NEW PARAMETER
) -> ExternResult<AgentActivity>
```

#### 3. Update GetAgentActivityInput

```rust
// crates/holochain_zome_types/src/agent_activity.rs
pub struct GetAgentActivityInput {
    pub agent_pubkey: AgentPubKey,
    pub chain_query_filter: ChainQueryFilter,
    pub activity_request: ActivityRequest,
    pub get_options: GetOptions,  // NEW FIELD
}
```

### Impact on User Code

#### Breaking: Direct Field Access

```rust
// BEFORE (won't compile)
if options.strategy == GetStrategy::Local { ... }

// AFTER (required update)
if options.strategy() == GetStrategy::Local { ... }
```

#### Minimal Impact for Common Usage

```rust
// Existing builder methods continue to work
let record = get(hash, GetOptions::network())?;
let record = get(hash, GetOptions::local())?;

// get_agent_activity needs update
let activity = get_agent_activity(
    agent, 
    query, 
    request, 
    GetOptions::default()  // NEW
)?;
```

#### Advanced Usage (New Capabilities)

```rust
// Query more agents for better coverage
let options = GetOptions::network()
    .with_remote_agent_count(5)  // Query 5 agents instead of 3
    .with_timeout_ms(5000);       // 5 second timeout

let record = get(hash, options)?;

// Fast local-only query
let record = get(hash, GetOptions::local())?;

// Future: Aggregate responses from multiple agents
let options = GetOptions::network()
    .with_remote_agent_count(10)
    .with_aggregation();  // Note: Not yet implemented
```

## Implementation Strategy

### Phase 1: Expand GetOptions Structure

1. Make existing `strategy` field private
2. Add new private fields as `Option<T>` for backward compatibility
3. Add getter methods for all fields
4. Add builder methods for ergonomic API
5. Update all internal Holochain code that accesses `options.strategy` to use `options.strategy()`

### Phase 2: Update Cascade Implementation

1. Modify cascade to convert expanded `GetOptions` to `NetworkRequestOptions`:
   ```rust
   fn to_network_options(get_options: &GetOptions) -> NetworkRequestOptions {
       NetworkRequestOptions {
           remote_agent_count: get_options.remote_agent_count()
               .unwrap_or(3),
           timeout_ms: get_options.timeout_ms(),
           as_race: get_options.as_race()
               .unwrap_or(true),
       }
   }
   ```

2. Update all cascade calls to use the converted options instead of defaults

### Phase 3: Add GetOptions to get_agent_activity

1. Update function signature
2. Update `GetAgentActivityInput` struct
3. Update host function implementation

### Phase 4: Documentation and Migration

1. Document when to use different options
2. Provide migration guide
3. Add examples for common use cases

## Migration Path for Users

### Breaking Changes

1. **Direct field access**: Any code accessing `options.strategy` directly must change to `options.strategy()`
2. **get_agent_activity**: Must add `GetOptions` parameter

### Backward Compatible Changes

- Builder methods (`GetOptions::network()`, `GetOptions::local()`) continue to work
- `GetOptions::default()` continues to work
- New capabilities are opt-in via builder methods

### Recommended Patterns

```rust
// For most queries - use defaults
get(hash, GetOptions::default())?;

// For critical data - query more agents
get(hash, GetOptions::network().with_remote_agent_count(5))?;

// For UI responsiveness - use local only
get(hash, GetOptions::local())?;

// For time-sensitive operations
get(hash, GetOptions::network().with_timeout_ms(2000))?;
```

## Additional Considerations

### 1. Validation Functions Remain Unchanged

The `must_get_*` functions in HDI continue to use system-controlled network behavior for determinism during validation.

### 2. Performance Implications

- `remote_agent_count` directly impacts network load and latency
- Higher counts increase reliability but also increase resource usage
- Apps can now make informed trade-offs

### 3. Future: Aggregation Mode

The `as_race: false` mode (aggregation) is not yet implemented but the API supports it for future use. When implemented, it will allow collecting responses from multiple agents before returning.

### 4. Reasonable Limits

- `remote_agent_count` should be capped (e.g., max 10) to prevent abuse
- Timeouts should have min/max bounds enforced by the conductor

### 5. Encapsulation Benefits

Using private fields with getter methods provides:
- Ability to add validation logic later
- Flexibility to change internal representation
- Cleaner API surface
- Better forward compatibility

## Recommendation

**Proceed with the expanded GetOptions approach using private fields:**

1. **Make fields private** with getter methods for encapsulation
2. **Expand GetOptions** with optional fields for new capabilities
3. **Add builder methods** for ergonomic API
4. **Update cascade** to respect all options
5. **Add GetOptions to get_agent_activity**
6. **Document patterns** for different use cases

## Estimated Effort

- **Core implementation**: 3-4 days
  - Refactor GetOptions to private fields: 0.5 days
  - Update all internal usage: 0.5 days
  - Expand GetOptions struct: 0.5 days
  - Update cascade implementation: 1 day
  - Add to get_agent_activity: 0.5 days
  - Testing: 1 day

- **Documentation & Migration Guide**: 1 day

- **Total**: ~4-5 days of development work

## Conclusion

This change addresses the real issue: app developers need control over network request behavior, not just Network vs Local strategy. By expanding `GetOptions` with private fields and getter methods, we maintain backward compatibility while giving developers fine-grained control over performance trade-offs. The encapsulation through private fields ensures we can evolve the API in the future without breaking changes. The validation functions remain deterministic and system-controlled as required.
