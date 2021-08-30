//! Test-only conductor config

/// Test-only conductor config
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct TestConfig {
    // Empty for now. I plumbed this through to enable what I thought was a
// necessary test feature, but then found it wasn't necessary. I kept the
// plumbing in to make it easier to add something in the future -MD
}
