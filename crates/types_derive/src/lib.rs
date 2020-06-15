//! ## Overview
//!
//! Derive macros for serialized bytes.
//!
//! ## Example
//!
//! ```rust
//! guse holochain_serialized_bytes::prelude::*;
//!
//! #[derive(Serialize, Deserialize, PartialEq, Debug, SerializedBytes)]
//! pub struct SomeData(String);
//!
//! ```

#![recursion_limit = "128"]
#![cfg_attr(tarpaulin, skip)]

extern crate proc_macro;

use quote::quote;

use proc_macro::TokenStream;
fn impl_default_serialized_bytes_address_macro(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let gen = quote! {
        addressable_serializable!(#name);
    };
    gen.into()
}

#[proc_macro_derive(SerializedBytesAddress)]
pub fn default_holochain_serialized_bytes_address_derive(input: TokenStream) -> TokenStream {
    // Construct a represntation of Rust code as a syntax tree
    // that we can manipulate
    let ast = syn::parse(input).unwrap();

    // Build the trait implementation
    impl_default_serialized_bytes_address_macro(&ast)
}
