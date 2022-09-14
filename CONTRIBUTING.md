# Contributing

[![Project](https://img.shields.io/badge/project-holochain-blue.svg?style=flat-square)](http://holochain.org/)
[![Discuss](https://img.shields.io/badge/chat-forum%2eholochain%2eorg-blue.svg?style=flat-square)](https://forum.holochain.org)

As an Open Source project, Holochain welcomes contributions of all sorts. Bug reports (and fixes), code and documention contributions, tests, feedback, and more are welcome. This document describes how to most effectively make each type of contribution.

## Social

We are committed to foster a vibrant thriving community, including growing a culture that breaks cycles of marginalization and dominance behavior. In support of this, some open source communities adopt [Codes of Conduct](http://contributor-covenant.org/version/1/3/0/). We are still working on our social protocols, and empower each team to describe its own *Protocols for Inclusion*. Until our teams have published their guidelines, please use the link above as a general guideline.

## Coordination

* For support be sure to explore our [Developer Resources and Documentation](https://developer.holochain.org)
* Chat with us on our [DEV.HC channel on Discord](https://discord.gg/MwPvM4Vffg)
* Ask and answer questions on our [online forums](https://forum.holochain.org)

Please make use of these resources to support your contributions, or simply to contribute your voice.

## Git Hygiene
This section describes our practices and guidelines for using git and making changes to the repo.

* We use Github's pull requests as our code review tool
* We encourage any dev to comment on pull requests and we think of the pull request not as a "please approve my code" but as a space for co-developing, i.e. asynchronous "pair-coding" of a sort.
* We develop features on separate branches
* We use merge (not rebase) so that commits related to a ticket can be retroactively explored.
* In most repos development happens on a `develop` branch which gets merged to `main` when there's a release.

### Pull requests

We warmly welcome pull requests for bug fixes, bug reproductions, documentation improvements, and any other "obviously good" enhancements to the codebase. If you are unsure if an enhancement is "obviously good", please coordinate with us first through a GitHub issue, or through our forums or Discord channel. We reserve the right to close any PR which doesn't fit our overall development trajectory, but we will gladly review any PRs and work with authors who have taken the time to identify a real problem or need and take steps to address it.

To open a PR, fork our Github repo, create a branch whose name describes your fix, and base your pull request on our `develop` branch.

If you add or change functionality, be sure to add both unit tests and integration tests to show that it works. Pull requests without tests will most likely not be accepted!


## Bug reports

The simplest way to report a bug is via [Github Issues](https://github.com/holochain/holochain/issues/new/choose) by selecting the Bug Report issue type. Please fill out all relevant areas of the bug report, including steps to reproduce. If your report is not specific enough, we will have a hard time addressing it without further followup.

## Bug fixes and minimal reproductions

We gladly welcome pull requests that help us identify and fix bugs!

The end goal of addressing any bug is to have a test written in our codebase to reproduce the bug, and of course to implement the fix for the bug. A PR with at least a minimal reproduction demonstrating the bug is extremely helpful, even if the fix has not been discovered.

To write a minimal reproduction of a problem discovered "in the wild", we recommend you to write a sample zome, DNA, or hApp which demonstrates the problem, and open a PR with your failing test. We have a library called [`sweettest`](https://docs.rs/holochain/latest/holochain/sweettest/index.html) which is well-suited to the task of testing the behavior of Holochain applications. 

When writing your reproduction PR, you can recreate the problematic part of your app in one of two ways: "inline zomes", or "test wasms".

### How to create "inline zomes"

The quickest, most preferable way to reproduce a problem is through "inline zomes". Inline zomes are written in terms of a collection of functions, like normal Wasm zomes, but they don't get compiled to wasm, and instead are run inline by Rust directly. This lightweight approach to writing zomes is well suited for quick test cases, or for cases that require a multitude of zomes in order to reproduce a problem.

To create a test based on inline zomes, see existing tests using `InlineZomeSet` or `SweetInlineZomes` for guidance. Just put your test in a place that feels appropriate.

### How to create a "test wasm"

Holochain has many "test wasms", which are sample zomes written to demonstrate specific functionality. These

It may be necessary to write a test wasm if the problem you've encountered has to do with the actual machinery of executing Wasm code. It may also be a good option if you discovered a problem while writing a zome yourself, in which case you can simply copy and paste the offending code into a new test wasm.

To create a test wasm:

1. Create a new crate in `crates/test_utils/wasm/wasm_workspace`
  - See the other test wasms for guidance on proper setup. In particular:
  - The `integrity.lib` will become your integrity zome, and `lib.rs` will become your coordinator zome. Your coordinator zome will be named after the crate name, and the integrity zome will be named with a `_integrity` suffix added.
2. Add your test wasm name to the `TestWasm` enum in [crates/test_utils/wasm/src/lib.rs](https://github.com/holochain/holochain/blob/1b663ec03c86462646cc7693391702a1de02b3a6/crates/test_utils/wasm/src/lib.rs#L21).
  - Make sure its PascalCase name matches the snake_case name of the crate.
  - You will have to make two other changes in the same file to specify the mapping to snake case, and the location of the built wasm, which will be straightforward to do by observing how other test wasms have done it.
3. Add your test wasm's crate name to the `[workspace]` section of `crates/test_utils/wasm/wasm_workspace/Cargo.toml`.
4. Build your test wasm (and all others) with `cargo build --features 'build_wasms' --manifest-path=crates/holochain/Cargo.toml`.

To write a test using your test wasm, you can use `sweettest::SweetDnaFile::unique_from_test_wasms(vec![TestWasm::YourWasm])` to set up your DNA. See existing tests which use this function to inspiration. [Here is a simple example](https://github.com/holochain/holochain/blob/1a9d85d79a900ad153843a851797f8d46d6ec0e1/crates/holochain/tests/agent_scaling/mod.rs#L101-L134) to follow for guidance.

## Compiler warnings

Compilation warnings are NOT OK in shared/production level code.

Warnings have a nasty habit of piling up over time. This makes your code increasingly unpleasant for other people to work with.

CI MUST fail or pass, there is no use in the ever noisier "maybe" status.

If you are facing a warning locally you can try:

0. Fixing it
1. Using `#[allow(***)]` inline to surgically override a once-off issue
2. Proposing a global `allow` for a specific rule
  - this is an extreme action to take
  - this should only be considered if it can be shown that:
    - the issue is common (e.g. dozens of `#allow[***]`)
    - disabling it won't cause issues/mess to pile up elsewhere
    - the wider Rust community won't find our codebase harder to work with

If you don't know the best approach, please ask for help!

It is NOT OK to disable `deny` for warnings globally at the CI or makefile/nix level.

You can allow warnings locally during development by setting the `RUSTFLAGS` environment variable.

#### Code style
We use rust-fmt to enforce code style so that we don't spend time arguing about this.

Run the formatter with:

``` shell
nix-shell --run hc-rust-fmt
```

or, if you have a version of `cargo` locally installed which matches the version used in the nix-shell:

```shell
cargo fmt
```

## Continuous Integration changes

Please also be aware that extending/changing the CI configuration can be very time consuming. Seemingly minor changes can have large downstream impact.

Some notable things to watch out for:

- Adding changes that cause the CI cache to be dropped on every run
- Changing the compiler/lint rules that are shared by many people
- Changing versions of crates/libs that also impact downstream crates/repos
- Changing the version of Rust used
- Adding/removing tools or external libs

The change may not be immediately apparent to you. The change may break the development environment on a different operating system, e.g. Windows.

At the same time, we do not want to catastrophise and stifle innovation or legitimate upgrades.

If you have a proposal to improve our CI config, that's great! Please open a dedicated branch for the change in isolation so we can discuss the proposal together. And then broadcast the proposal in chat to maximise visibility and the opportunity for everyone to respond.

It is NOT OK to change the behaviour of tests/CI in otherwise unrelated PRs. SOMETIMES it MAY be OK to change CI in a related PR, e.g. adding a new lib that your code requires. DO expect that a change like this will probably attract additional scrutiny during the PR review process, which is unfortunate but important.

Use your best judgement and respect that other people, across all timezones, rely on this repository remaining a productive working environment 24/7/365.

## License
Holochain is licensed under the Cryptographic Autonomy License [![License: CAL v1](https://img.shields.io/badge/License-CAL%201.0-blue.svg)](https://github.com/holochain/cryptographic-autonomy-license) which is the first [Open Source Initiative approved](https://opensource.org/licenses/CAL-1.0) license designed for distributed software. As such it is designed to protect the rights of end-users of applications built on Holochain to own their own data and cryptographic keys. See [this article](https://medium.com/holochain/understanding-the-cryptographic-autonomy-license-172ac920966d) for more detail about licensing requirements of P2P software.

Other components, applications, and libraries we build are typically shared under the [Apache License v2](http://www.apache.org/licenses/LICENSE-2.0) as a simple, lighweight, and flexible way to share code.

Copyright (C) 2017 - 2022, Holochain Foundation
