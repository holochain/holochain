# holochain_state

## Persisted State building blocks

This crate provides a few types for working with LMDB databases. The types build upon those found in [holochain_lmdb::buffer].

- [ElementBuf]: the union of two CasBuffers, one for Entries, one for Headers
- [ChainSequenceBuf]: database representing the chain sequence DB, which provides a special method for accessing the chain head
- [SourceChainBuf]: the union of a [ElementBuf] and a [ChainSequenceBuf], which fully represents a source chain
- [MetadataBuf]: (*unimplemented*) Uses a KvvBuffer to represent EAV-like relationships between CAS entries
- [Cascade]: (*unimplemented*) Unifies two [ElementBuf] and two [MetadataBuf] references (one of each is a cache) in order to perform the complex metadata-aware queries for getting entries and links, including CRUD resolution

The follow diagram shows the composition hierarchy.
The arrows mean "contains at least one of".

```none
              Cascade         SourceChain
                 |                 |
                 |                 V
                 |           SourceChainBuf
                 |                 |
                 |                 |
           +----------+      +-----+------+
           |          |      |            |
           |          V      V            |
           V         ElementBuf          V
      MetadataBuf         |        ChainSequenceBuf
           |              V               |
           |           CasBuf             |
           |              |               |
           V              V               V
        KvvBuf          KvBuf          IntKvBuf

source: https://textik.com/#d7907793784e17e9
```
