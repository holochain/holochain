pub enum AgentMigrateDnaDirection {
    Open,
    Close,
}

#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum AgentMigrateCallbackResult {
    Pass,
    Fail(String),
}
