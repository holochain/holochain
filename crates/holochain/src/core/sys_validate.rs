pub use crate::core::state::source_chain::{SourceChainError, SourceChainResult};
pub use holo_hash_core::*;
pub use holochain_types::{element::ChainElement, HeaderHashed};

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
    element: &ChainElement,
    prev_element: Option<&ChainElement>,
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
                header.timestamp().to_string(),
                prev_header.timestamp().to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use holo_hash::{HeaderHash, HoloHashExt};
    use holochain_types::{
        element::SignedHeaderHashed,
        header::InitZomesComplete,
        test_utils::{fake_agent_pubkey_1, fake_header_hash},
        Timestamp,
    };
    use std::convert::TryInto;

    async fn test_gen(ts: Timestamp, seq: u32, prev: HeaderAddress) -> ChainElement {
        let keystore = holochain_state::test_utils::test_keystore();

        let header = InitZomesComplete {
            author: fake_agent_pubkey_1(),
            timestamp: ts,
            header_seq: seq,
            prev_header: prev,
        };

        let hashed = HeaderHashed::with_data(header.into()).await.unwrap();
        let signed = SignedHeaderHashed::new(&keystore, hashed).await.unwrap();
        ChainElement::new(signed, None)
    }

    #[tokio::test(threaded_scheduler)]
    async fn valid_headers_validate() {
        let first = test_gen(
            "2020-05-05T19:16:04.266431045Z".try_into().unwrap(),
            12,
            fake_header_hash(1).await,
        )
        .await;
        let second = test_gen(
            "2020-05-05T19:16:04.366431045Z".try_into().unwrap(),
            13,
            first.header_address().clone(),
        )
        .await;

        sys_validate_element(&fake_agent_pubkey_1(), &second, Some(&first))
            .await
            .unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn invalid_hash_headers_dont_validate() {
        let first = test_gen(
            "2020-05-05T19:16:04.266431045Z".try_into().unwrap(),
            12,
            fake_header_hash(1).await,
        )
        .await;
        let second = test_gen(
            "2020-05-05T19:16:04.366431045Z".try_into().unwrap(),
            13,
            fake_header_hash(2).await,
        )
        .await;

        matches::assert_matches!(
            sys_validate_element(&fake_agent_pubkey_1(), &second, Some(&first)).await,
            Err(SourceChainError::InvalidPreviousHeader(_))
        );
    }

    #[tokio::test(threaded_scheduler)]
    async fn invalid_timestamp_headers_dont_validate() {
        let first = test_gen(
            "2020-05-05T19:16:04.266431045Z".try_into().unwrap(),
            12,
            fake_header_hash(1).await,
        )
        .await;
        let second = test_gen(
            "2020-05-05T19:16:04.166431045Z".try_into().unwrap(),
            13,
            first.header_address().clone(),
        )
        .await;

        matches::assert_matches!(
            sys_validate_element(&fake_agent_pubkey_1(), &second, Some(&first)).await,
            Err(SourceChainError::InvalidPreviousHeader(_))
        );
    }

    #[tokio::test(threaded_scheduler)]
    async fn invalid_seq_headers_dont_validate() {
        let first = test_gen(
            "2020-05-05T19:16:04.266431045Z".try_into().unwrap(),
            12,
            fake_header_hash(1).await,
        )
        .await;
        let second = test_gen(
            "2020-05-05T19:16:04.366431045Z".try_into().unwrap(),
            14,
            first.header_address().clone(),
        )
        .await;

        matches::assert_matches!(
            sys_validate_element(&fake_agent_pubkey_1(), &second, Some(&first)).await,
            Err(SourceChainError::InvalidPreviousHeader(_))
        );
    }
}
