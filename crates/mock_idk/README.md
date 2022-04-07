
# Holochain Mock IDK

This is a simple utility crate that allows mocking the IDK.

# Examples

```rust
use idk::prelude::*;

// Create the mock.
let mut mock_idk = holochain_mock_idk::MockIdkT::new();

// Create the a return type.
let empty_agent_key = AgentPubKey::from_raw_36(vec![0u8; 36]);

// Setup the expectation.
mock_idk.expect_hash().once().returning({
    let empty_agent_key = empty_agent_key.clone();
    move |_| Ok(HashOutput::Entry(empty_agent_key.clone().into()))
});

// Set the IDK to use the mock.
set_idk(mock_idk);

// Create an input type.
let hash_input = HashInput::Entry(Entry::Agent(empty_agent_key.clone()));

// Call the IDK and the mock will run.
let hash_output = IDK.with(|i| i.borrow().hash(hash_input)).unwrap();

assert!(matches!(
    hash_output,
    HashOutput::Entry(output) if output == EntryHash::from(empty_agent_key)
));
```