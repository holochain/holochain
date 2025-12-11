
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// Path to the file where generated typescript types will be written.
pub const TS_TYPES_FILE = "/tmp/exported-types.ts";

/// Export the Rust type to Typescript in the common file.
///
/// The Typescript types are all generated when `cargo test export_bindings` is run.
/// 
/// This simply applies two macros provided by ts-rs:
///
/// ```rust
/// #[derive(ts_rs::TS)]
/// #[ts(export, export_to=TS_TYPES_FILE)]
/// ```
#[proc_macro_derive(TsExport, attributes(ts))]
pub fn ts_export(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Expand into both TS derive and ts attribute
    let expanded = quote! {
        #[derive(ts_rs::TS)]
        #[ts(export, export_to=TS_TYPES_FILE)]
        #input
    };

    expanded.into()
}
