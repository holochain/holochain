use hdk_derive::*;

#[derive(UnitEnum)]
enum Nesting {
    A(A),
    B(A),
}

struct A;

fn main() {}
