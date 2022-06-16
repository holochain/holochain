//! Facts for Elements

use crate::prelude::*;
use contrafact::*;
use holo_hash::*;

type Pair = (Action, Option<Entry>);

/// Fact: Given a pair of a action and optional Entry:
/// - If the action references an Entry, the Entry will exist and be of the appropriate hash
/// - If the action does not references an Entry, the entry will be None
//
// TODO: this Fact is useless until we can write "traversals" in addition to lenses and prisms,
// because we cannot in general use a lens to extract a `&mut (Action, Option<Entry>)`
// from any type, and instead need to operate on a `(&mut Action, &mut Option<Entry>)`.
// (A Traversal is like a lens that can focus on more than one thing at once.)
// Alternatively, this might be an argument for making contrafact work with immutable values
// instead of mutable references.
//
// At least we can use this as a reference to write the same logic for DhtOp and Element,
// which require the same sort of checks.

pub fn action_and_entry_match() -> Facts<'static, Pair> {
    facts![
        brute(
            "Action type matches Entry existence",
            |(action, entry): &Pair| {
                let has_action = action.entry_data().is_some();
                let has_entry = entry.is_some();
                has_action == has_entry
            }
        ),
        mapped(
            "If there is entry data, the action must point to it",
            |pair: &Pair| {
                if let Some(entry) = &pair.1 {
                    // NOTE: this could be a `lens` if the previous check were short-circuiting,
                    // but it is possible that this check will run even if the previous check fails,
                    // so use a prism instead.
                    facts![prism(
                        "action's entry hash",
                        |pair: &mut Pair| pair.0.entry_data_mut().map(|(hash, _)| hash),
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
    use crate::action::facts as action_facts;
    use arbitrary::{Arbitrary, Unstructured};

    #[test]
    fn test_action_and_entry_match() {
        let mut uu = Unstructured::new(&crate::NOISE);
        let u = &mut uu;

        let e = Entry::arbitrary(u).unwrap();
        let hn = not_(action_facts::is_new_entry_action()).build(u);
        let mut he = action_facts::is_new_entry_action().build(u);
        *he.entry_data_mut().unwrap().0 = EntryHash::with_data_sync(&e);
        let he = Action::from(he);

        let pair1: Pair = (hn.clone(), None);
        let pair2: Pair = (hn.clone(), Some(e.clone()));
        let pair3: Pair = (he.clone(), None);
        let pair4: Pair = (he.clone(), Some(e.clone()));

        let fact = action_and_entry_match();

        fact.check(&pair1).unwrap();
        assert!(fact.check(&pair2).is_err());
        assert!(fact.check(&pair3).is_err());
        fact.check(&pair4).unwrap();
    }
}
