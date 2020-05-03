#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum PostCommitCallbackResult {
    Success(HeaderAddress),
    Fail(HeaderAddress, FailString),
}
