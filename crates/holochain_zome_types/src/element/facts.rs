//! Facts for Elements

use crate::prelude::*;
use contrafact::*;
use holo_hash::*;

type Pair = (Header, Option<Entry>);

/// Fact: Given a pair of a header and optional Entry:
/// - If the header references an Entry, the Entry will exist and be of the appropriate hash
/// - If the header does not references an Entry, the entry will be None
//
// TODO: this Fact is useless until we can write "traversals" in addition to lenses and prisms,
// because we cannot in general use a lens to extract a `&mut (Header, Option<Entry>)`
// from any type, and instead need to operate on a `(&mut Header, &mut Option<Entry>)`.
// (A Traversal is like a lens that can focus on more than one thing at once.)
// Alternatively, this might be an argument for making contrafact work with immutable values
// instead of mutable references.
//
// At least we can use this as a reference to write the same logic for DhtOp and Element,
// which require the same sort of checks.

pub fn header_and_entry_match() -> Facts<'static, Pair> {
    facts![
        brute(
            "Header type matches Entry existence",
            |(header, entry): &Pair| {
                let has_header = header.entry_data().is_some();
                let has_entry = entry.is_some();
                has_header == has_entry
            }
        ),
        mapped(
            "If there is entry data, the header must point to it",
            |pair: &Pair| {
                if let Some(entry) = &pair.1 {
                    // NOTE: this could be a `lens` if the previous check were short-circuiting,
                    // but it is possible that this check will run even if the previous check fails,
                    // so use a prism instead.
                    facts![prism(
                        "header's entry hash",
                        |pair: &mut Pair| dbg!(pair).0.entry_data_mut().map(|(hash, _)| hash),
                        eq("hash of matching entry", EntryHash::with_data_sync(entry)),
                    )]
                } else {
                    facts![always()]
                }
            }
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::facts as header_facts;
    use arbitrary::{Arbitrary, Unstructured};

    #[test]
    fn test_header_and_entry_match() {
        let mut uu = Unstructured::new(&NOISE);
        let u = &mut uu;

        let e = Entry::arbitrary(u).unwrap();
        let hn = not_(header_facts::is_new_entry_header()).build(u);
        let mut he = header_facts::is_new_entry_header().build(u);
        *he.entry_data_mut().unwrap().0 = EntryHash::with_data_sync(&e);
        let he = Header::from(he);

        let pair1: Pair = dbg!((hn.clone(), None));
        let pair2: Pair = dbg!((hn.clone(), Some(e.clone())));
        let pair3: Pair = dbg!((he.clone(), None));
        let pair4: Pair = dbg!((he.clone(), Some(e.clone())));

        let fact = header_and_entry_match();

        fact.check(&pair1).unwrap();
        assert!(fact.check(&pair2).is_err());
        assert!(fact.check(&pair3).is_err());
        fact.check(&pair4).unwrap();
    }
}
