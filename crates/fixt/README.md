# Fixt

## Fixturator

This crate implements the fixturator pattern.

A fixturator is an iterator over a curve in fixture space.

I.e. each time you call `my_fixturator.next().unwrap()` you get some test value.

The fixturator is created with a curve like `Predictable`, `Empty`, or `Unpredictable`.

For example, if we want some test keypairs like 'alice' and 'bob' for our tests we
define a predictable curve that iterates like `[ alice, bob, charlie, whoever... ]`.

This provides a simple standard interface and some convenience macros to avoid scattering `test_agent_a()`, `test_agent_b()`, etc. everywhere.

The iterator interface in Rust can return `None` but our fixturators must always return a value.
This means it is safe and necessary to call `unwrap()` on the fixturator.
The fixturator implementation itself internally ensures that the iterator cycles or whatever is
needed to ensure values can always be produced. It is valid to simply loop back to the start
of the fixturator sequence as `.cycle()` does.

Note that fixturators are only intended for testing.

The curves:

- `Empty`: Examples of an 'empty' value, which is usually something similar to but not the same as `Default`.
           This was introduced because `Default` is not empty and would conflate testing and production needs.
- `Predictable`: Known values that we want to test against for deterministic function tests like 'alice' and 'bob'.
                 This is especially helpful for testing largely opaque cryptographic behaviour like hashes and signatures.
- `Unpredictable`: Returns different values each test run to try and flush out common mistakes, like division by `0` or `NaN` handling.
                   This is NOT fuzz testing, there is no 'shrinking', no effort to be comprehensive, and only one value is tested per run.
                   This exists as a simple 80/20 effort way to efficiently flush out 'just so' implementations that result in common bugs.

Fixturators work from a seeded PRNG so if a test fails you can lookup the seed from the logs and reproduce the behaviour.