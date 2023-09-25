
# Holochain Mock HDI

This is a simple utility crate that allows mocking the HDI.

# Examples

```rust
use hdi::prelude::*;

// Create the mock.
let mut mock_hdi = holochain_mock_hdi::MockHdiT::new();

// Create the a return type.
let empty_agent_key = AgentPubKey::from_raw_36(vec![0u8; 36]);

// Setup the expectation.
mock_hdi.expect_hash().once().returning({
    let empty_agent_key = empty_agent_key.clone();
    move |_| Ok(HashOutput::Entry(empty_agent_key.clone().into()))
});

// Set the HDI to use the mock.
set_hdi(mock_hdi);

// Create an input type.
let hash_input = HashInput::Entry(Entry::Agent(empty_agent_key.clone()));

// Call the HDI and the mock will run.
let hash_output = HDI.with(|i| i.borrow().hash(hash_input)).unwrap();

assert!(matches!(
    hash_output,
    HashOutput::Entry(output) if output == EntryHash::from(empty_agent_key)
));
```
