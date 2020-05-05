//! # Persisted State building blocks
//!
//! This crate provides a few types for working with LMDB databases. The types build upon those found in [holochain_state::buffer].
//!
//! - [ChainCasBuffer]: the union of two CasBuffers, one for Entries, one for Headers
//! - [ChainSequenceBuffer]: database representing the chain sequence DB, which provides a special method for accessing the chain head
//! - [SourceChainBuffer]: the union of a [ChainCasBuffer] and a [ChainSequenceBuffer], which fully represents a source chain
//! - [CasMetaBuffer]: (*unimplemented*) Uses a KvvBuffer to represent EAV-like relationships between CAS entries
//! - [Cascade]: (*unimplemented*) Unifies two [ChainCasBuffer] and two [CasMetaBuffer] references (one of each is a cache) in order to perform the complex metadata-aware queries for getting entries and links, including CRUD resolution
//!
//! The follow diagram shows the composition hierarchy.
//! The arrows mean "contains at least one of".
//!
//! ```none
//!               Cascade         SourceChain
//!                  |                 |
//!                  |                 V
//!                  |           SourceChainBuf
//!                  |                 |
//!                  |                 |
//!            +----------+      +-----+------+
//!            |          |      |            |
//!            |          V      V            |
//!            V         ChainCasBuf          V
//!        CasMetaBuf         |        ChainSequenceBuf
//!            |              V               |
//!            |           CasBuf             |
//!            |              |               |
//!            V              V               V
//!         KvvBuf          KvBuf          IntKvBuf
//!
//! source: https://textik.com/#d7907793784e17e9
//! ```

pub mod cascade;
pub mod chain_cas;
pub mod chain_meta;
pub mod chain_sequence;
pub mod source_chain;
pub mod wasm;
pub mod workspace;
