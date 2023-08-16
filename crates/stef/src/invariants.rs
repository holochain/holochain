//! Helpers to fuzz test your [`State`] to check that the invariants are upheld.
//! This works best if you have a single `State` which unifies all of your other
//! States, so that the interactions between different state machines can be tested.

pub trait StateInvariants {
    fn fuzz_test_invariants() {
        todo!()
    }
}
