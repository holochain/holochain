## Guide setting up a Holochain developer environment on Windows

The recommended way to develop hApps on Holochain if you're a Windows user is to use WSL2. If you need to build on Windows
then this guide will give you the steps we know are needed. However, we don't have reproducible automated builds to ensure
that these steps stay up-to-date so please be prepared to encounter problems. You are invited to raise an 
[issue](https://github.com/holochain/holochain/issues/new?assignees=&labels=&projects=&template=bug_report.md&title=%5BBUG%5D) 
if you do!

#### 1. Install the Rust toolchain

Get `rustup` from [here](https://www.rust-lang.org/tools/install). Rather than installing a specific Rust version  after
installing `rustup`, we recommend using a toolchain file. You can find an [example file](https://github.com/holochain/holochain/blob/develop/rust-toolchain.toml)
in the Holochain repository. Place a copy of this file in your project root and Cargo will read it, then install the
correct Rust version and tools for you.

#### 2. Set up build dependencies

Building some of Holochain's dependencies from source on Windows requires Perl, which is used for configuration scripts.

A good option for Perl on Windows is [Strawberry Perl](https://strawberryperl.com/). Any Perl distribution will do though,
if you would prefer something else or already have Perl.

Holochain also depends on SQLite and OpenSSL, but these are supposed to be built for you by default. This means you should 
not need to provide either of these when building Holochain. If you get errors about them, it's likely an issue with the 
build configuration of Holochain, so please [let us know](https://github.com/holochain/holochain/issues/new?assignees=&labels=&projects=&template=bug_report.md&title=%5BBUG%5D).

#### 3. Install Holochain 

Install Holochain and the other binaries you'll need to work with it:

```shell
cargo install --version 0.5.2 holochain
cargo install --version 0.5.2 holochain_cli
cargo install --version 0.6.1 lair_keystore
cargo install --version 0.500.0 holochain_scaffolding_cli
cargo install --version 0.500.0 holochain_cli_launch
```

To find the latest versions of these tools, you can check crates.io. For example, [lair_keystore](https://crates.io/crates/lair_keystore)

Check that your Cargo installed binaries are in your path and that you have the right versions by doing

```shell
holochain --version
hc --version
lair-keystore --version
hc scaffold --version
hc launch --version
```

#### 4. Get started!

You should now be able to develop with Holochain using the tools installed in the previous section. You will need to ignore
the Nix commands in guides and use the CLI tools directly. Otherwise you shouldn't need to do anything special.

Get started [here](https://developer.holochain.org/get-building/).

#### 5. Find other projects using Windows

Most projects are using MacOS and Linux development environments, including WSL2. There are projects being built on Windows 
and you may want to learn from their experiences and documentation:

- https://github.com/NextGenSoftwareUK/OASIS-Holochain-hApp
- https://github.com/holochain-open-dev/wiki/wiki/Installing-Holochain--&-Building-hApps-Natively-On-Windows

There is also the c# client HoloNET which you may want to check out:

https://github.com/holochain-open-dev/holochain-client-csharp
