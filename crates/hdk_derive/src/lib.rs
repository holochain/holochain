#![crate_type = "proc-macro"]
extern crate proc_macro;
use proc_macro::TokenStream;
use std::convert::TryFrom;

enum WrappedMacro {
    MapExtern,
}

impl TryFrom<String> for WrappedMacro {
    type Error = ();
    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.as_str() {
            "extern" => Ok(crate::WrappedMacro::MapExtern),
            _ => Err(()),
        }
    }
}

#[proc_macro_attribute]
pub fn hdk(attr: TokenStream, item: TokenStream) -> TokenStream {
    match crate::WrappedMacro::try_from(attr.to_string()).unwrap() {
        crate::WrappedMacro::MapExtern => {
            // extern mapping is only valid for functions
            let mut item_fn: syn::ItemFn = syn::parse(item).unwrap();

            // extract the ident of the fn
            // this will be exposed as the external facing extern
            let external_fn_ident = item_fn.sig.ident.clone();

            // build a new internal fn ident that is compatible with map_extern!
            // this needs to be sufficiently unlikely to have namespace collisions with other fns
            let internal_fn_ident = syn::Ident::new(
                &format!("{}_hdk_extern", external_fn_ident.to_string()),
                item_fn.sig.ident.span(),
            );

            // replace the ident in-place with the new internal ident
            item_fn.sig.ident = internal_fn_ident.clone();

            // add a map_extern! and include the modified item_fn
            (quote::quote! {
                map_extern!(#external_fn_ident, #internal_fn_ident);
                #item_fn
            })
            .into()
        }
    }
}
