//! Facts for Records

use crate::prelude::*;
use contrafact::*;
use holo_hash::*;

type Pair = (Action, RecordEntry);

/// Fact: Given a pair of an action and optional Entry:
/// - If the action references an Entry,
///     - the Entry will exist and be of the appropriate hash,
///     - and the entry types will match
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

pub fn action_and_entry_match<'a>(must_be_public: bool) -> impl Fact<'a, Pair> {
    facts![
        brute(
            "Action type matches Entry existence, and is public if exists",
            move |(action, entry): &Pair| {
                let data = action.entry_data();
                match (data, entry) {
                    (Some((_entry_hash, entry_type)), RecordEntry::Present(_)) => {
                        // Ensure that entries are public
                        !must_be_public || entry_type.visibility().is_public()
                    }
                    (None, RecordEntry::Present(_)) => false,
                    (None, _) => true,
                    _ => false,
                }
            }
        ),
        lambda_unit(
            "If there is entry data, the action must point to it",
            |g, pair: Pair| {
                if let Some(entry) = pair.1.as_option() {
                    // NOTE: this could be a `lens` if the previous check were short-circuiting,
                    // but it is possible that this check will run even if the previous check fails,
                    // so use a prism instead.
                    prism(
                        "action's entry hash",
                        |(action, _): &mut Pair| action.entry_data_mut().map(|(hash, _)| hash),
                        eq(EntryHash::with_data_sync(entry)),
                    )
                    .mutate(g, pair)
                } else {
                    Ok(pair)
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
            let ns = noise(Some(seed), 100_000);
            let mut gg = unstructured(&ns).into();
            let g = &mut gg;

            let e = brute("Is App entry", |e| matches!(e, Entry::App(_))).build(g);
            let a0 = action_facts::is_not_entry_action().build(g);
            let mut a1 = action_facts::is_new_entry_action().build(g);
            *a1.entry_data_mut().unwrap().0 = EntryHash::with_data_sync(&e);
            let a1 = Action::from(a1);

            let pair1: Pair = (a0.clone(), RecordEntry::NA);
            let pair2: Pair = (a0.clone(), RecordEntry::Present(e.clone()));
            let pair3: Pair = (a1.clone(), RecordEntry::NA);
            let pair4: Pair = (a1.clone(), RecordEntry::Present(e.clone()));

            // dbg!(&a0, &a1, &e);

            let fact = action_and_entry_match(false);

            fact.clone().check(&pair1).unwrap();
            assert!(fact.clone().check(&pair2).is_err());
            assert!(fact.clone().check(&pair3).is_err());
            fact.clone().check(&pair4).unwrap();
        }
    }
}
