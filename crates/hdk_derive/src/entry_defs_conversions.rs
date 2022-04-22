use proc_macro::TokenStream;
use syn::parse_macro_input;
use syn::Item;
use syn::ItemEnum;

pub fn build(_attrs: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Item);
    let ident = match &input {
        Item::Enum(ItemEnum { ident, .. }) => ident,
        _ => todo!(),
    };

    let try_from_sb: proc_macro2::TokenStream = match &input {
        Item::Enum(ItemEnum { variants, .. }) => {
            let inner: proc_macro2::TokenStream = variants
            .into_iter()
            .map(
                |syn::Variant {
                     ident: v_ident,
                     fields,
                     ..
                 }| {
                    match fields {
                        syn::Fields::Named(_) => todo!(),
                        syn::Fields::Unit => todo!(),
                        syn::Fields::Unnamed(_) => {
                            // TODO: Error if fields is longer then one.
                            quote::quote! {#ident::#v_ident (v) => SerializedBytes::try_from(v),}
                        }
                    }
                },
            )
            .collect();
            quote::quote! {
                let result = match t {
                    #inner
                };
            }
        }
        _ => todo!("Make a real error"),
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
