use crate::prelude::*;
use arbitrary::{Arbitrary, Unstructured};
use contrafact::*;
use holo_hash::*;

/// A rough check that a sequence of Headers constitutes a valid source chain
/// - First header must be Dna
/// - Each subsequent header's prev_header hash must match the previous header
/// - The seq number must be increasing by 1, from 0
///
/// Notably, this does NOT check the following:
/// xxx Genesis headers in the proper place
/// xxx Genesis headers in the *wrong* place
///
/// TODO: It would be more readable/composable to break this into several parts:
/// - constrain header types based on position
/// - constrain seq num
/// - constrain prev_hashes
/// ...but, this does it all in one Fact
#[derive(Default)]
struct ValidChainFact {
    hash: Option<HeaderHash>,
    seq: u32,
}

impl Fact<Header> for ValidChainFact {
    fn check(&self, header: &Header) -> Check {
        let header_hash = HeaderHash::with_data_sync(header);
        let result = match (header.prev_header(), self.hash.as_ref()) {
            (Some(prev), Some(stored)) => {
                if prev == stored {
                    Check::pass()
                } else {
                    vec![format!("Hashes don't match: {} != {}", prev, stored)].into()
                }
            }
            (None, None) => Check::pass(),
            (None, Some(_)) => vec![format!(
                "Found Dna in position other than beginning of the chain. Hash: {}",
                header_hash
            )]
            .into(),
            (Some(_), None) => vec![format!(
                "First header must be of type Dna, but instead got type {:?}",
                header.header_type()
            )]
            .into(),
        };

        result
    }

    fn mutate(&self, header: &mut Header, u: &mut Unstructured<'static>) {
        if let Some(stored_hash) = self.hash.as_ref() {
            // This is not the first header we've seen
            while header.prev_header().is_none() {
                // Generate arbitrary headers until we get one with a prev header
                *header = Header::arbitrary(u).unwrap();
            }
            // Set the header's prev hash to the one we stored from our previous
            // visit
            *header.prev_header_mut().unwrap() = stored_hash.clone();
            // Also set the seq to the next value (this should only be None
            // iff prev_header is None)
            *header.header_seq_mut().unwrap() = self.seq;
        } else {
            // This is the first header we've seen, so it must be a Dna
            *header = Header::Dna(Dna::arbitrary(u).unwrap());
        }

        println!(
            "{}  =>  {:?}\n",
            self.hash.as_ref().unwrap(),
            header.prev_header()
        );
    }

    fn advance(&mut self, header: &Header) {
        self.hash = Some(HeaderHash::with_data_sync(header));
        self.seq += 1;
    }
}

pub fn is_of_type(header_type: HeaderType) -> Facts<'static, Header> {
    facts![brute("header is of type", move |h: &Header| h
        .header_type()
        == header_type)]
}

pub fn is_new_entry_header() -> Facts<'static, Header> {
    facts![or(
        "is NewEntryHeader",
        is_of_type(HeaderType::Create),
        is_of_type(HeaderType::Update)
    )]
}

/// WIP: Fact: The headers form a valid SourceChain
pub fn valid_chain() -> Facts<'static, Header> {
    facts![ValidChainFact::default(),]
}

/// Fact: The header must be a NewEntryHeader
pub fn new_entry_header() -> Facts<'static, Header> {
    facts![brute("Is a NewEntryHeader", |h: &Header| {
        matches!(h.header_type(), HeaderType::Create | HeaderType::Update)
    }),]
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_valid_chain_fact() {
        let mut u = Unstructured::new(&NOISE);

        let chain = build_seq(&mut u, 5, valid_chain());
        check_seq(chain.as_slice(), valid_chain()).unwrap();

        let hashes: Vec<_> = chain
            .iter()
            .map(|h| HeaderHash::with_data_sync(h))
            .collect();
        let backlinks: Vec<_> = chain
            .iter()
            .filter_map(|h| h.prev_header())
            .cloned()
            .collect();
        let header_seqs: Vec<_> = chain.iter().map(|h| h.header_seq()).collect();

        // Ensure that the backlinks line up with the actual hashes
        assert_eq!(hashes[0..chain.len() - 1], backlinks[..]);
        // Ensure that the header seqs form a sequence
        assert_eq!(header_seqs, vec![0, 1, 2, 3, 4]);
    }
}

/// Some necessary extra mutators for lenses/prisms over Headers
impl Header {
    /// returns a mutable reference to the author
    pub fn author_mut(&mut self) -> &mut AgentPubKey {
        match *self {
            Self::Dna(Dna { ref mut author, .. })
            | Self::AgentValidationPkg(AgentValidationPkg { ref mut author, .. })
            | Self::InitZomesComplete(InitZomesComplete { ref mut author, .. })
            | Self::CreateLink(CreateLink { ref mut author, .. })
            | Self::DeleteLink(DeleteLink { ref mut author, .. })
            | Self::Delete(Delete { ref mut author, .. })
            | Self::CloseChain(CloseChain { ref mut author, .. })
            | Self::OpenChain(OpenChain { ref mut author, .. })
            | Self::Create(Create { ref mut author, .. })
            | Self::Update(Update { ref mut author, .. }) => author,
        }
    }
    /// returns a mutable reference to the sequence ordinal of this header
    pub fn header_seq_mut(&mut self) -> Option<&mut u32> {
        match *self {
            // Dna is always 0
            Self::Dna(Dna { .. }) => None,
            Self::AgentValidationPkg(AgentValidationPkg {
                ref mut header_seq, ..
            })
            | Self::InitZomesComplete(InitZomesComplete {
                ref mut header_seq, ..
            })
            | Self::CreateLink(CreateLink {
                ref mut header_seq, ..
            })
            | Self::DeleteLink(DeleteLink {
                ref mut header_seq, ..
            })
            | Self::Delete(Delete {
                ref mut header_seq, ..
            })
            | Self::CloseChain(CloseChain {
                ref mut header_seq, ..
            })
            | Self::OpenChain(OpenChain {
                ref mut header_seq, ..
            })
            | Self::Create(Create {
                ref mut header_seq, ..
            })
            | Self::Update(Update {
                ref mut header_seq, ..
            }) => Some(header_seq),
        }
    }

    /// returns the previous header except for the DNA header which doesn't have a previous
    pub fn prev_header_mut(&mut self) -> Option<&mut HeaderHash> {
        match self {
            Self::Dna(Dna { .. }) => None,
            Self::AgentValidationPkg(AgentValidationPkg {
                ref mut prev_header,
                ..
            }) => Some(prev_header),
            Self::InitZomesComplete(InitZomesComplete {
                ref mut prev_header,
                ..
            }) => Some(prev_header),
            Self::CreateLink(CreateLink {
                ref mut prev_header,
                ..
            }) => Some(prev_header),
            Self::DeleteLink(DeleteLink {
                ref mut prev_header,
                ..
            }) => Some(prev_header),
            Self::Delete(Delete {
                ref mut prev_header,
                ..
            }) => Some(prev_header),
            Self::CloseChain(CloseChain {
                ref mut prev_header,
                ..
            }) => Some(prev_header),
            Self::OpenChain(OpenChain {
                ref mut prev_header,
                ..
            }) => Some(prev_header),
            Self::Create(Create {
                ref mut prev_header,
                ..
            }) => Some(prev_header),
            Self::Update(Update {
                ref mut prev_header,
                ..
            }) => Some(prev_header),
        }
    }

    pub fn entry_data_mut(&mut self) -> Option<(&mut EntryHash, &mut EntryType)> {
        match self {
            Self::Create(Create {
                ref mut entry_hash,
                ref mut entry_type,
                ..
            }) => Some((entry_hash, entry_type)),
            Self::Update(Update {
                ref mut entry_hash,
                ref mut entry_type,
                ..
            }) => Some((entry_hash, entry_type)),
            _ => None,
        }
    }
}
