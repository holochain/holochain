# holochain_cli

A library and CLI to help create, run and interact with sandboxed Holochain conductor environments,
for testing and development purposes.
**Warning this is still WIP and subject to change**
There's probably a few bugs. If you find one please open an [issue](https://github.com/holochain/holochain/issues)
or make a PR.

### CLI
The `hc` CLI makes it easy to run a dna that you are working on
or someone has sent you.
It has been designed to use sensible defaults but still give you
the configurability when that's required.
Sandboxes are stored in tmp directories by default and the paths are
persisted in a `.hc` file which is created wherever you are using
the CLI.
#### Install
##### Requirements
- [Rust](https://rustup.rs/)
- [Holochain](https://github.com/holochain/holochain) binary on the path
##### Building
From github:
```shell
cargo install holochain_cli --git https://github.com/holochain/holochain
```
From the holochain repo:
```shell
cargo install --path crates/hc
```
#### Common usage
The best place to start is:
```shell
hc -h
```
This will be more up to date than this readme.
##### Run
This command can be used to generate and run conductor sandboxes.
```shell
hc run -h
# or shorter
hc r -h
```
 In a folder with where your `my-dna.dna` is you can generate and run
 a new sandbox with:
```shell
hc r
```
If you have already created a sandbox previously then it will be reused
(usually cleared on reboots).
##### Generate
Generates new conductor sandboxes and installs apps / dnas.
```shell
hc generate
# or shorter
hc g
```
For example this will generate 5 sandboxes with app ids set to `my-app`
using the `elemental-chat.dna` from the current directory with a quic
network configured to use localhost.
_You don't need to specify dnas when they are in the directory._
```shell
 hc gen -a "my-app" -n 5 ./elemental-chat.dna network quic
```
You can also generate and run in the same command:
(Notice the number of conductors and dna path must come before the gen sub-command).
```shell
 hc r -n 5 ./elemental-chat.dna gen -a "my-app" network quic
```
##### Call
Allows calling the [`AdminRequest`] api.
If the conductors are not already running they
will be run to make the call.

```shell
hc call list-cells
```
##### List and Clean
These commands allow you to list the persisted sandboxes
in the current directory (from the`.hc`) file.
You can use the index from:
```shell
hc list
```
Output:
```shell
hc-admin:
Sandboxes contained in `.hc`
0: /tmp/KOXgKVLBVvoxe8iKD4iSS
1: /tmp/m8VHwwt93Uh-nF-vr6nf6
2: /tmp/t6adQomMLI5risj8K2Tsd
```
To then call or run an individual sandbox (or subset):

```shell
hc r -i=0,2
```
You can clean up these sandboxes with:
```shell
hc clean 0 2
# Or clean all
hc clean
```
### Library
This crate can also be used as a library so you can create more
complex sandboxes / admin calls.
See the docs:
```shell
cargo doc --open
```
and the examples.
