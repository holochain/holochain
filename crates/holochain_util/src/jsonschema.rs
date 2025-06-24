//! Custom schema representations for serializing third-party types in JSON Schema format.

use schemars::{generate::SchemaGenerator, Schema};

/// Custom schemars representation for `Url2`
pub fn url2_schema(_: &mut SchemaGenerator) -> Schema {
    schemars::json_schema!({
        "type": "string",
        "format": "uri",
    })
}
