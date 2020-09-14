# Feature List

These are the features that are currently implemented or planned and their status.

## HDK features
| Feature                  | Status      | Example     | Comment |
|--------------------------|-------------|-------------|---------|
| `entry_hash!()`          | Done        | [Example](https://github.com/Holo-Host/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/entry_hash/src/lib.rs) |         |
| `get!()`                 | Done        | [Example](https://github.com/Holo-Host/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/anchor/src/lib.rs) |         |
| `get_details!()`         | Done        | [Example](https://github.com/Holo-Host/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/crud/src/countree.rs) |         |
| `get_links!()`           | Done        | [Example](https://github.com/Holo-Host/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/link/src/lib.rs) |         |
| `get_link_details!()`    | Done        |  |         |
| `query!()`               | Done        | [Example](https://github.com/Holo-Host/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/query/src/lib.rs) |         |
| `commit_entry!()`        | Done        | [Example](https://github.com/Holo-Host/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/commit_entry/src/lib.rs) |         |
| `update_entry!()`        | Done        | [Example](https://github.com/Holo-Host/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/crud/src/countree.rs) |         |
| `delete_entry!()`        | Done        | [Example](https://github.com/Holo-Host/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/crud/src/countree.rs) |         |
| `link_entries!()`        | Done        | [Example](https://github.com/Holo-Host/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/link/src/lib.rs) | LinkTag may get splitted in Tag and Type in the future |
| `zome_info!()`           | Done        | [Example](https://github.com/Holo-Host/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/zome_info/src/lib.rs) |         |
| `agent_info!()`          | Done        | [Example](https://github.com/Holo-Host/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/agent_info/src/lib.rs) |         |
| `call_remote!()`         | Done        | [Example](https://github.com/Holo-Host/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/capability/src/lib.rs) |         |
| `sys_time!()`            | Done        | [Example](https://github.com/Holo-Host/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/sys_time/src/lib.rs) |         |
| `Path` & `Anchors`       | Done        | [Paths](https://github.com/Holo-Host/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/hash_path/src/lib.rs) & [Anchors](https://github.com/Holo-Host/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/anchor/src/lib.rs) |         |
| `generate_cap_secret!()` | Done        | [Example](https://github.com/Holo-Host/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/capability/src/lib.rs) |         |
| `commit_cap_grant!()`    | Done        | [Example](https://github.com/Holo-Host/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/capability/src/lib.rs) |         |
| `update_cap_grant!()`    | Done        | [Example](https://github.com/Holo-Host/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/capability/src/lib.rs) |         |
| `delete_cap_grant!()`    | Done        | [Example](https://github.com/Holo-Host/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/capability/src/lib.rs) |         |
| `commit_cap_claim!()`    | Done        | [Example](https://github.com/Holo-Host/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/capability/src/lib.rs) |         |
| `init`                   | In progress | Callback exists, it is not called yet |
| `post_commit`            | In progress | Callback exists, it is not called yet |
| `keystore!() `           | In progress    |         |         |
| `decrypt!() `            | Not done    |         |         |
| `encrypt!() `            | Not done    |         |         |
| `sign!() `               | Not done    |         |         |
| `schedule!() `           | Not done    |         |         |
| `emit_signal!()`         | Not done    |         |         |
| `property!() `           | Not done    |         |         |

## Core features

| Feature                        | Status      | Comment |
|--------------------------------|-------------|---------|
| Capabilities                   | Done        |         |
| Multi-agent interaction        | Done        | Many agents only work inside one conductor |
| Networking                     | In progress |         |
| Keystore (key-management)      | Provisional | A mocked keystore is in place, the real one close to be completed |
| System Validation              | Done |  |
| App Validation                     | In progress | Callback exists, but validation flows are still being implemented, and callback types may change |
| Validation receipts (warrants) | Not done    |         |

## Ecosystem and tools

| Feature                             | Status      | Comment |
|-------------------------------------|-------------|---------|
| JS side client                      | Done        | [@holochain/conductor-api](https://github.com/Holo-Host/holochain-conductor-api) |
| hApp integration testing            | Provisional | [@holochain/tryorama](https://github.com/Holo-Host/tryorama-rsm) has been adapted, will probably change in the future |
| Cross-platform executable binary    | Not done    | The executable would add a system tray icon from which you can interact with holochain (Note: holoscape is not going to be upgraded to RSM and will be discontinued) |