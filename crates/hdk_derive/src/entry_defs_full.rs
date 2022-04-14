use proc_macro::TokenStream;
use syn::parse_macro_input;
use syn::Item;

pub fn build(_attrs: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Item);

    let output = quote::quote! {
        #[hdk_derive::entry_defs_name_registration]
        #[hdk_derive::entry_defs_conversions]
        #input
    };
    // eprintln!("{}", output);
    output.into()
}
