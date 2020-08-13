# How to build Holochain DNA

*as of 12-08-2020*

## Important note about dependencies

Effective zome development requires a few cargo dependencies which are not yet published to crates.io and exist only in the private [holochain-rsm GitHub repo](https://github.com/Holo-Host/holochain). Since the repo is private, we cannot simply point our Cargo.toml file directly to it without some configuration. There are two options:

1. Clone the Holochain repo, and use local paths in your Cargo.toml(s) to refer to your local checkout of Holochain, or
2. If you use SSH keys for GitHub, configure your ssh-agent so that Cargo can be aware of your key, per https://doc.rust-lang.org/cargo/appendix/git-authentication.html. Then you can use remote git deps in your zomes' Cargo.toml(s).

At the time of writing, I could not get the remote option to work, so the example zomes here use local paths (option 1).

## Steps

### 0. Build `holochain` and `dna-util`

You'll need two binaries to develop DNAs: the actual Holochain conductor binary, and the dna-util library which assists with assembling Wasms into a DNA file.

- Clone the repo: `git clone https://github.com/Holo-Host/holochain && cd ./holochain`
- Install conductor binary: `cargo install --path crates/holochain`
- Install dna-util binary: `cargo install --path crates/dna_util`

You should now have `holochain` and `dna-util` on your PATH.

### 1. Write your Zomes

Each zome is a Rust crate. See [crates/test_utils/wasm/whoami](../crates/test_utils/wasm/whoami) and [crates/test_utils/wasm/foo](../crates/test_utils/wasm/foo) for examples.

### 2. Build your Zomes into Wasm

When you want to (re)build your zomes into Wasm, simply run

```bash
cargo build --release --target wasm32-unknown-unknown
```

and they will be available in `target/wasm32-unknown-unknown/release/`

### 3. Assemble your Wasms into a DNA file

*Note: Soon, this process will be easier in that it will not require a `.dna.workdir`*

1. Create a `demo.dna.workdir` directory (replace "demo" with whatever you want)
2. Create a `demo.dna.workdir/dna.json` file which references the `*.wasm` files you built in the previous step. See the [dna.json](dna.json) file in this repo for an example.
  - Note: this is a bit hacky right now. Normally when using a dna.workdir, you would include the Wasms alongside the `dna.json` in the same directory. However, it is easier for the purposes of this tutorial to let the `dna.json` reference Wasms in a different directory. The workdir construct becomes more useful when you need to go back and forth between an already-built DNA and its constituent Wasms.
3. Run the following command to assemble your Wasms into a DNA file per your dna.json:

```bash
dna-util -c demo.dna.workdir
```

This will produce a `demo.dna.gz` file as a sibling of the `demo.dna.workdir` directory.

### 4. Use the Conductor's admin interface to install your DNA

If you are using Tryorama to run tests against your DNA, you can jump over to the [tryorama README](https://github.com/Holo-Host/tryorama-rsm) (also a private repo) and follow the instructions there.

If you are running Holochain using your own setup, you'll have to have a deeper understanding of Holochain than is in scope for this tutorial. Roughly speaking, you'll need to:

- make sure `holochain` is running with a configuration that includes an admin interface websocket port
- send a properly encoded [`InstallApp`](https://github.com/Holo-Host/holochain/blob/66ca899d23842cadebc214d591475987f4af4f43/crates/holochain/src/conductor/api/api_external/admin_interface.rs#L240) command over the websocket
- be sure to `ActivateApp` and `AttachAppInterface` as well.
