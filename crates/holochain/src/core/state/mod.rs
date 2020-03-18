//! # Persisted State building blocks
//!
//! This crate provides a few types for working with LMDB databases. The types build upon those found in [sx_state::buffer].
//!
//! - [ChainCasBuffer]: the union of two CasBuffers, one for Entries, one for Headers
//! - [ChainSequenceBuffer]: database representing the chain sequence DB, which provides a special method for accessing the chain head
//! - [SourceChainBuffer]: the union of a [ChainCasBuffer] and a [ChainSequenceBuffer], which fully represents a source chain
//! - [CasMetaBuffer]: (*unimplemented*) Uses a KvvBuffer to represent EAV-like relationships between CAS entries
//! - [Cascade]: (*unimplemented*) Unifies two [ChainCasBuffer] and two [CasMetaBuffer] references (one of each is a cache) in order to perform the complex metadata-aware queries for getting entries and links, including CRUD resolution


pub mod cascade;
pub mod chain_cas;
pub mod chain_sequence;
pub mod source_chain;
pub mod workspace;
