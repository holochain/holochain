use hdk::prelude::holochain_deterministic_integrity::prelude::*;

#[hdk_entry_defs_conversions]
enum Nesting {
    A(A, B),
    B,
    C { a: A },
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
