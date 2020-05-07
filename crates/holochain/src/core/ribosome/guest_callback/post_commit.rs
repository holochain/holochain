use holo_hash::HeaderHash;

pub struct PostCommitInvocation<'a> {
    zome_name: &'a str,
    header: &'a HeaderHash,
}

pub struct PostCommitResult;
