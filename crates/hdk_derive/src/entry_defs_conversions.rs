use proc_macro::TokenStream;
use proc_macro_error::abort;
use syn::parse_macro_input;
use syn::Item;
use syn::ItemEnum;

use crate::util::get_single_tuple_variant;

pub fn build(_attrs: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Item);
    let (ident, variants) = match &input {
        Item::Enum(ItemEnum {
            ident, variants, ..
        }) => (ident, variants),
        _ => abort!(input, "hdk_entry_def_conversions can only be used on Enums"),
    };

    let inner: proc_macro2::TokenStream = variants
        .into_iter()
        .map(
            |syn::Variant {
                 ident: v_ident,
                 fields,
                 ..
             }| {
                get_single_tuple_variant(v_ident, fields);
                quote::quote! {#ident::#v_ident (v) => SerializedBytes::try_from(v),}
            },
        )
        .collect();
    let try_from_sb: proc_macro2::TokenStream = quote::quote! {
        let result = match t {
            #inner
        };
    };

    let output = quote::quote! {
        #input

        impl TryFrom<&#ident> for AppEntryBytes {
            type Error = WasmError;
            fn try_from(t: &#ident) -> Result<Self, Self::Error> {
                #try_from_sb
                AppEntryBytes::try_from(result?).map_err(|entry_error| match entry_error {
                    EntryError::SerializedBytes(serialized_bytes_error) => {
                        WasmError::Serialize(serialized_bytes_error)
                    }
                    EntryError::EntryTooLarge(_) => {
                        WasmError::Guest(entry_error.to_string())
                    }
                })
            }
        }
        impl TryFrom<#ident> for AppEntryBytes {
            type Error = WasmError;
            fn try_from(t: #ident) -> Result<Self, Self::Error> {
                Self::try_from(&t)
            }
        }

        impl TryFrom<&#ident> for Entry {
            type Error = WasmError;
            fn try_from(t: &#ident) -> Result<Self, Self::Error> {
                Ok(Self::App(AppEntryBytes::try_from(t)?))
            }
        }

        impl TryFrom<#ident> for Entry {
            type Error = WasmError;
            fn try_from(t: #ident) -> Result<Self, Self::Error> {
                Self::try_from(&t)
            }
        }

    };
    output.into()
}
