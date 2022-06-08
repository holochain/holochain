use proc_macro::TokenStream;
use syn::parse_macro_input;
use syn::Item;

pub fn build(attrs: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Item);
    let attr_args: proc_macro2::TokenStream = attrs.into();

    let output = quote::quote! {
        #[hdk_derive::hdk_entry_defs_name_registration(#attr_args)]
        #[hdk_derive::hdk_entry_defs_conversions]
        #[derive(Debug)]
        #input
    };
    output.into()
}
