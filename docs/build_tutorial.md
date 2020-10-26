# How to build Holochain DNA

*as of 28-10-2020*

## Steps

### 0. Build `holochain` and `dna-util`

You'll need two binaries on your PATH to develop DNAs: the actual Holochain (`holochain`) conductor binary, and the dna-util library which assists with assembling Wasms into a DNA file.

There are two ways you can approach that, via a [nix-shell](https://nixos.org/manual/nix/stable/#ch-installing-binary) which handles the majority for you, or via direct Rust installation to your computer. Instructions for both follow.

#### nix-shell

Install nix, **linux**
```bash
sh <(curl -L https://nixos.org/nix/install) --no-daemon
```
Install nix, **macOS**
```bash
sh <(curl -L https://nixos.org/nix/install) --darwin-use-unencrypted-nix-store-volume
```

Clone the holochain/holochain repo
```bash
git clone git@github.com:holochain/holochain.git
```

Enter the directory
```bash
cd holochain
```

Launch a nix-shell, based on holochain/holochain's nix-shell configuration
```bash
nix-shell
```

Install the `holochain` and `dna-util` binaries using the built-in installer
```bash
hc-install
```

Confirm that they are there by running `holochain -V` and `dna-util -V`, and that you see simple version number outputs.

#### native rust

Install Rust
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Grab and install the binaries for `holochain` and `dna-util` from github, at the right version
```bash
cargo install holochain --git https://github.com/holochain/holochain.git --branch develop
cargo install dna_util --git https://github.com/holochain/holochain.git --branch develop
```

Confirm that they are there by running `holochain -V` and `dna-util -V`, and that you see simple version number outputs.

### 1. Write your Zomes

Each zome is a Rust crate. See [crates/test_utils/wasm/wasm_workspace/whoami](../crates/test_utils/wasm/wasm_workspace/whoami) and [crates/test_utils/wasm/foo](../crates/test_utils/wasm/wasm_workspace/foo) or any other folder in [crates/test_utils/wasm/wasm_workspace](../crates/test_utils/wasm/wasm_workspace) for examples.

### 2. Build your Zomes into Wasm

When you want to (re)build your zomes into Wasm, simply run

```bash
CARGO_TARGET_DIR=target cargo build --release --target wasm32-unknown-unknown
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

If you are using Tryorama to run tests against your DNA, you can jump over to the [tryorama (rsm branch) README](https://github.com/holochain/tryorama/tree/rsm) and follow the instructions there.

If you are running Holochain using your own setup, you'll have to have a deeper understanding of Holochain than is in scope for this tutorial. Roughly speaking, you'll need to:

- make sure `holochain` is running with a configuration that includes an admin interface websocket port
- send a properly encoded [`InstallApp`](https://github.com/holochain/holochain/blob/7db6c1e340dd0e741dcc9ffd51ffc832caa36449/crates/types/src/app.rs#L14-L23) command over the websocket
- be sure to `ActivateApp` and `AttachAppInterface` as well.
