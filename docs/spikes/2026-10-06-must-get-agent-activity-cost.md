# `must_get_agent_activity` cost

Spike for [holochain#6007](https://github.com/holochain/holochain/issues/6007),
measured 2026-10-06 on `develop` in the release profile (opt-level 2, thin
LTO, on-disk SQLCipher store). All timings are median milliseconds per call.

## Findings

1. **`take(n)` reads the whole chain.** The `Take` path sets no lower sequence
   bound, so `take(2)` on a 2,000-action chain costs the same as walking to
   genesis.
2. **Every call scans the whole store.** `ChainOp` has no index on
   `action_hash` and `Action` has none on `author`, so the activity query, the
   chain-top lookup, the authority record lookup and the integration-time
   lookups by action hash each scan every `ChainOp` row. A one-row
   `until_hash` read grows with the number of ops held by any agent: about
   0.7 µs per op while the store is small and cached, 5 to 13 µs per op on
   disk at 20k to 120k ops. The conductor sets no SQLite `cache_size`, so
   deployed stores hit the slow end early. Op integration slows down the same
   way: 543 ops/s at 20k ops, 36 ops/s at 120k.
3. **The chain itself is cheap.** Reading and decoding an action costs about
   8 µs, so a 2,000-action to-genesis walk is 17 to 28 ms.

Per call on a peer's store, 2,000-action chain, 6,000 ops held:

| Configuration | `take(2)` | `until_hash`, 1 row | to genesis |
|---|---:|---:|---:|
| `develop` | 15 | 4.1 | 20 |
| fix 1: bounded take | 7 | 11 | 23 |
| fix 2: indexes | 19 | 0.3 | 22 |
| both | 0.3 | 0.6 | 17 |
| both, 1,000,000 ops held | 0.4 | 0.3 | 127 |

## Fixes

- **Fix 1.** When the filter is `Take(n)` and no `until_hash` resolved, bound
  the scan to `[chain_top_seq - (n - 1), chain_top_seq]`. Every valid chain
  lowers the sequence by one per step, so the answer does not change.
- **Fix 2.** Add indexes `Action(author, seq)` and
  `ChainOp(action_hash, op_type)`, and push `a.seq >= ?` into the query only
  when a lower bound exists. The `(? IS NULL OR a.seq >= ?)` form cannot be
  used as an index range.

## Effect on an app that walks chains in validation

Unyt validates every app entry on two ops, and for each one walks the
author's chain to genesis, reads one action by `until_hash`, and fetches
every app-entry record on the chain. Projected store work per validated entry
on a full-arc node, for 200 full-arc and 1,000 zero-arc agents:

| Chain length | Ops held | `develop`, cached to cold | Both fixes |
|---:|---:|---:|---:|
| 500 | 2.1M | 6 s to 84 s | 0.1 s |
| 2,000 | 8.4M | 24 s to 5.6 min | 0.4 s |
| 10,000 | 42M | 2 min to 28 min | 1.8 s |

Every full-arc node validates every entry, so the second-to-last column is
also the interval between entries above which validation queues never drain.
After the fixes, what remains is the app's own to-genesis walk and record
replay; a checkpoint entry in the DNA would remove that term too.

## Tests

Both are `#[ignore]`d measurements and print their tables.

```sh
# Three conductors, walks timed inside validation and on a peer's store as
# the chain grows. MGAA_CHAIN_LENGTH, MGAA_STEP and MGAA_FILLER tune it.
cargo test --release -p holochain --no-default-features \
  --features slow_tests,build_wasms,encryption,wasmer-sys-cranelift \
  --test integration must_get_agent_activity_chain_growth -- --ignored --nocapture

# Store only, on disk and encrypted, filled to the op counts in MGAA_STORE_OPS.
cargo test --release -p holochain_state --features test_utils,encryption \
  must_get_agent_activity_store_scan_cost -- --ignored --nocapture
```
