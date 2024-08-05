## Guide setting up a Holochain developer environment on Windows

The recommended way to develop hApps on Holochain if you're a Windows user is to use WSL2. If you need to build on Windows
then this guide will give you the steps we know are needed. However, we don't have reproducible automated builds to ensure
that these steps stay up-to-date so please be prepared to encounter problems. You are invited to raise an 
[issue](https://github.com/holochain/holochain/issues/new?assignees=&labels=&projects=&template=bug_report.md&title=%5BBUG%5D) 
if you do!

#### 1. Install the Rust toolchain

Get `rustup` from [here](https://www.rust-lang.org/tools/install). This will set up the latest version of Rust, but it is 
preferable to use the same version that Holochain is using. You can optionally install a specific version of rust using

```shell
rustup toolchain install 1.75.0
```

Find the installed toolchain with `rustup toolchain list`, then select it using a command like this, with your toolchain 
in place of the example given here.

```shell
rustup default 1.75.0-x86_64-pc-windows-msvc
```

You can find the current version used by Holochain [here](https://github.com/holochain/holochain/blob/develop/nix/modules/holochain.nix#L8).

You'll need to add the wasm32 target to be able to compile hApps, add this using

```
rustup target add wasm32-unknown-unknown
```

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
cargo install --version 0.1.5 holochain
cargo install --version 0.1.5 holochain_cli
cargo install --version 0.2.4 lair_keystore
cargo install --version 0.1.7 holochain_scaffolding_cli
cargo install --version 0.0.12 holochain_cli_launch
```

For the 0.1 series of Holochain releases, you can find the current version [here](https://github.com/holochain/holochain/blob/develop/versions/0_1/flake.nix#L5)
and the matching Lair keystore version [here](https://github.com/holochain/holochain/blob/develop/versions/0_1/flake.nix#L10).

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
