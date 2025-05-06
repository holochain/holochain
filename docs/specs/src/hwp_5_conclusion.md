# Conclusion

We have described an approach to distributed systems design that achieves increasing capacities of social coordination and coherence without requiring the bottlenecks of global consensus, which delivers on the promise of massively scalable and secure distributed applications fit for heterogeneous contexts. This approach has been fully demonstrated.

In lieu of a reference specification for the wire protocols and application programming interfaces (APIs) that make up a functioning Holochain implementation, along with their data types, we instead refer readers to Holochain's "living specification"; that is, its code base, and more specifically the documentation generated from this codebase. This will always be the most faithful reference for the state of Holochain at any given time.

* [`holochain_conductor_api` on docs.rs](https://docs.rs/holochain_conductor_api), documentation of the APIs by which clients interact with a running Holochain conductor.
* [`hdk` on docs.rs](https://docs.rs/hdk), documentation of the software development kit (SDK) for use by Rust-based guest applications comprising integrity and coordinator zomes. By exposing access to Holochain's WASM Host API via ergonomic Rust functions, this SDK also indirectly documents said Host API.
* [Holochain codebase on GitHub](https://github.com/holochain/holochain), an implementation of a Holochain runtime, as well as the aforementioned HDK and various test specifications that demonstrate proper and improper usage of the conductor's APIs.
* [Kitsune2 codebase on GitHub](https://github.com/holochain/kitsune2), an implementation of Holochain's wire protocol.
* [Lair codebase on GitHub](https://github.com/holochain/lair), an implementation of Holochain's key store.