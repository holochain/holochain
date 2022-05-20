use hdk::prelude::holochain_deterministic_integrity::prelude::*;

#[hdk_to_local_types(nested)]
enum Nesting {
    A(Nested1),
    #[allow(dead_code)]
    B {
        nested: Nested2,
    },
    C,
}
#[hdk_to_local_types]
enum Nested1 {
    A,
    B,
}

enum Nested2 {
    X,
    Y,
    Z,
}

fn main() {}
