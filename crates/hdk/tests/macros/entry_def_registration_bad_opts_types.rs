use hdk::prelude::holochain_deterministic_integrity::prelude::*;

#[derive(EntryDefRegistration)]
enum Nesting {
    #[entry_def(name = 55)]
    A(A),
    B(B)
}

enum A {
    A,
    B,
}

enum B {
    A,
    B,
}

fn main() {}
