use proc_macro::TokenStream;
use syn::parse_macro_input;
use syn::Item;
use syn::ItemEnum;

pub fn build(_attrs: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Item);

    let (ident, variants) = match &input {
        Item::Enum(ItemEnum {
            ident, variants, ..
        }) => (ident, variants),
        _ => todo!(),
    };

    let try_into_entry: proc_macro2::TokenStream = variants
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
                        quote::quote! {#ident::#v_ident (v) => Entry::try_from(v),}
                    }
                }
            },
        )
        .collect();

    let to_app_name: proc_macro2::TokenStream = variants
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
                        quote::quote! {#ident::#v_ident (v) => v.entry_def_name(),}
                    }
                }
            },
        )
        .collect();

    let get_entry_type: proc_macro2::TokenStream = variants
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
                        quote::quote! {
                            #ident::#v_ident (v) => {
                                EntryType::App(AppEntryType {
                                    id: v.entry_def_index(),
                                    zome_id,
                                    visibility: v.entry_visibility(),
                                })
                            }
                        }
                    }
                }
            },
        )
        .collect();

    let output = quote::quote! {
        #[derive(ToZomeName)]
        #input

        impl #ident {
            pub fn entry_type(&self) -> ExternResult<EntryType> {
                let zome_name = self.zome_name();
                let DnaInfo { zome_names, .. } = hdk::prelude::dna_info()?;
                let zome_id = zome_names
                    .iter()
                    .position(|name| *name == zome_name)
                    .map(|i| ZomeId(i as u8))
                    .ok_or_else(|| WasmError::Host(format!("Unable to find zome name {}", zome_name)))?;
                Ok(match self {
                    #get_entry_type
                })
            }
        }

        impl TryFrom<&#ident> for Entry {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                match value {
                    #try_into_entry
                }
            }
        }

        impl TryFrom<#ident> for Entry {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                Entry::try_from(&value)
            }
        }

        impl ToAppEntryDefName for &#ident {
            fn entry_def_name(&self) -> AppEntryDefName {
                match self {
                    #to_app_name
                }
            }
        }

        impl ToAppEntryDefName for #ident {
            fn entry_def_name(&self) -> AppEntryDefName {
                (&self).entry_def_name()
            }
        }

        impl From<&#ident> for AppEntryDefLocation {
            fn from(i: &#ident) -> Self {
                Self {
                    zome: i.zome_name(),
                    entry: i.entry_def_name(),
                }
            }
        }

        impl From<#ident> for AppEntryDefLocation {
            fn from(i: #ident) -> Self {
                (&i).into()
            }
        }

        impl From<&#ident> for EntryDefLocation {
            fn from(i: &#ident) -> Self {
                EntryDefLocation::App(i.into())
            }
        }

        impl From<#ident> for EntryDefLocation {
            fn from(i: #ident) -> Self {
                (&i).into()
            }
        }

    };
    output.into()
}
