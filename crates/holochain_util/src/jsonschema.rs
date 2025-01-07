use schemars::{
    gen::SchemaGenerator,
    schema::{InstanceType, Schema, SchemaObject},
};

/// Custom schemars representation for `Url2`
pub fn url2_schema(_: &mut SchemaGenerator) -> Schema {
    Schema::Object(SchemaObject {
        instance_type: Some(InstanceType::String.into()),
        format: Some("uri".to_string()),
        ..Default::default()
    })
}
