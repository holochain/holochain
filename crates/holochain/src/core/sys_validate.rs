pub use crate::core::state::source_chain::{SourceChainError, SourceChainResult};
pub use holo_hash::*;
pub use holochain_types::{
    element::{Element, ElementExt},
    HeaderHashed, Timestamp,
};

#[cfg(test)]
mod tests;


/// Ensure that a given pre-fetched element is actually valid on this chain.
///
/// Namely:
/// - The header signature is valid.
/// - The header is valid (see validate_header).
/// - The signature was authored by the agent that owns this chain.
/// - @TODO - The entry content hashes properly & matches the hash in the header.
/// - @TODO - The entry content is shaped properly according to the header type.
/// - @TODO - The serialized entry content is < 100MB.
pub async fn sys_validate_element(
    author: &AgentPubKey,
    element: &Element,
    prev_element: Option<&Element>,
) -> SourceChainResult<()> {
    // The header signature is valid.
    element.validate().await?;

    // The header is valid.
    sys_validate_header(
        element.header_hashed(),
        prev_element.map(|e| e.header_hashed()),
    )
    .await?;

    // The header was authored by the agent that owns this chain.
    if element.header().author() != author {
        tracing::error!(
            "Author mismatch! {} != {}, element: {:?}",
            element.header().author(),
            author,
            element
        );
        return Err(SourceChainError::InvalidSignature);
    }

    // - @TODO - The entry content hashes properly & matches the hash in the header.

    // - @TODO - The entry content is shaped properly according to the header type.

    // - @TODO - The serialized entry content is < 100MB.

    Ok(())
}

/// Ensure that a given pre-fetched header is actually valid on this chain.
///
/// Namely:
/// - If the header type contains a previous header reference
///   (true for everything except the Dna header).
///   Then, ensure the previous header timestamp sequence /
///   ordering is correct, and the previous header is strictly the previous
///   header by sequence.
/// - @TODO - The agent was valid in DPKI at time of signing.
pub async fn sys_validate_header(
    header: &HeaderHashed,
    prev_header: Option<&HeaderHashed>,
) -> SourceChainResult<()> {
    // - If the header type contains a previous header reference
    //   (true for everything except the Dna header).
    //   Then, ensure the previous header timestamp sequence /
    //   ordering is correct, and the previous header is strictly the previous
    //   header by sequence.

    // the only way this can be None is for Dna,
    // in the case of Dna, we don't need to check the previous header.
    if let Some(asserted_prev_header) = header.prev_header() {
        // verify we have the correct previous header
        let prev_header = match prev_header {
            None => {
                return Err(SourceChainError::InvalidPreviousHeader(
                    "expected previous header, received None".to_string(),
                ));
            }
            Some(prev_header) => prev_header,
        };

        // ensure the hashes match
        if asserted_prev_header != prev_header.as_hash() {
            return Err(SourceChainError::InvalidPreviousHeader(format!(
                "expected header hash: {}, received: {}",
                asserted_prev_header,
                prev_header.as_hash(),
            )));
        }

        // make sure the timestamps are in order
        if header.timestamp() < prev_header.timestamp() {
            return Err(SourceChainError::InvalidPreviousHeader(format!(
                "expected timestamp < {}, received: {}",
                Timestamp::from(header.timestamp()).to_string(),
                Timestamp::from(prev_header.timestamp()).to_string(),
            )));
        }

        // make sure the header_seq is strictly ordered
        if header.header_seq() - 1 != prev_header.header_seq() {
            return Err(SourceChainError::InvalidPreviousHeader(format!(
                "expected header_seq: {}, received: {}",
                header.header_seq() - 1,
                prev_header.header_seq(),
            )));
        }
    }

    // - @TODO - The agent was valid in DPKI at time of signing.

    Ok(())
}