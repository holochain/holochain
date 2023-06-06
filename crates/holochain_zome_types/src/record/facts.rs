//! Facts for Records

use crate::prelude::*;
use contrafact::*;
use holo_hash::*;

type Pair = (Action, Option<Entry>);

/// Fact: Given a pair of an action and optional Entry:
/// - If the action references an Entry, the Entry will exist and be of the appropriate hash, and the entry types will match
/// - If the action does not reference an Entry, the entry will be None
//
// TODO: this Fact is useless until we can write "traversals" in addition to lenses and prisms,
// because we cannot in general use a lens to extract a `&mut (Action, Option<Entry>)`
// from any type, and instead need to operate on a `(&mut Action, &mut Option<Entry>)`.
// (A Traversal is like a lens that can focus on more than one thing at once.)
// Alternatively, this might be an argument for making contrafact work with immutable values
// instead of mutable references.
//
// At least we can use this as a reference to write the same logic for DhtOp and Record,
// which require the same sort of checks.

pub fn action_and_entry_match() -> Facts<Pair> {
    facts![
        brute(
            "Action type matches Entry existence",
            |(action, entry): &Pair| {
                match (action.entry_data(), entry) {
                    (Some((_, et)), Some(entry)) => entry_type_matches(et, entry),
                    (None, None) => true,
                    _ => false,
                }
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

    proptest::proptest! {
        #[test]
        fn test_action_and_entry_match(seed: u64) {
            let ns = noise(seed);
            let mut uu = unstructured(&ns);
            let u = &mut uu;

            let e = brute("Is App entry", |e| matches!(e, Entry::App(_))).build(u);
            let a0 = not_(action_facts::is_new_entry_action()).build(u);
            let mut a1 = action_facts::is_new_entry_action().build(u);
            *a1.entry_data_mut().unwrap().0 = EntryHash::with_data_sync(&e);
            let a1 = Action::from(a1);

            let pair1: Pair = (a0.clone(), None);
            let pair2: Pair = (a0.clone(), Some(e.clone()));
            let pair3: Pair = (a1.clone(), None);
            let pair4: Pair = (a1.clone(), Some(e.clone()));

            dbg!(&a0, &a1, &e);

            let fact = action_and_entry_match();

            fact.check(&pair1).unwrap();
            assert!(fact.check(&pair2).is_err());
            assert!(fact.check(&pair3).is_err());
            fact.check(&pair4).unwrap();
        }
    }
}
