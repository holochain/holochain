use proc_macro::TokenStream;
use proc_macro_error::abort;
use syn::parse_macro_input;
use syn::Item;
use syn::ItemEnum;

use crate::util::get_single_tuple_variant;
use crate::util::index_to_u8;

pub fn build(_attrs: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Item);

    let (ident, variants) = match &input {
        Item::Enum(ItemEnum {
            ident, variants, ..
        }) => (ident, variants),
        _ => abort!(input, "hdk_dependent_entry_types can only be used on Enums"),
    };

    let key_to_variant: proc_macro2::TokenStream = variants
        .iter()
        .enumerate()
        .map(|(i, v)| (index_to_u8(i), v))
        .map(
            |(
                i,
                syn::Variant {
                    ident: v_ident,
                    fields,
                    ..
                },
            )| {
                let ty = &get_single_tuple_variant(v_ident, fields).ty;
                quote::quote! {
                    ZomeTypesKey {
                        zome_index: ZomeDependencyIndex(#i),
                        type_index,
                    } => {
                        let key = ZomeTypesKey {
                            zome_index: 0.into(),
                            type_index,
                        };
                        <#ty as UnitEnum>::Unit::iter()
                            .find_map(|unit| (ZomeEntryTypesKey::from(unit) == key).then(|| unit))
                            .map_or(Ok(None), |unit| Ok(Some(Self::#v_ident(#ty::try_from((unit, entry))?))))
                    }
                }
            },
        )
        .collect();

    let try_into_entry: proc_macro2::TokenStream = variants
        .into_iter()
        .map(
            |syn::Variant {
                 ident: v_ident,
                 fields,
                 ..
             }| {
                get_single_tuple_variant(v_ident, fields);
                quote::quote! {#ident::#v_ident (v) => Entry::try_from(v),}
            },
        )
        .collect();

    let into_visibility: proc_macro2::TokenStream = variants
        .into_iter()
        .map(
            |syn::Variant {
                 ident: v_ident,
                 fields,
                 ..
             }| {
                get_single_tuple_variant(v_ident, fields);
                quote::quote! {#ident::#v_ident (v) => EntryVisibility::from(v),}
            },
        )
        .collect();

    let output = quote::quote! {
        #[hdk_to_coordinates(nested = true, entry = true)]
        #[derive(Debug)]
        #input

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

        impl TryFrom<&#ident> for ScopedEntryDefIndex {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                match zome_info()?.zome_types.entries.get(value) {
                    Some(t) => Ok(t),
                    _ => Err(wasm_error!(WasmErrorInner::Guest(format!(
                        "{:?} does not map to any ZomeId and EntryDefIndex that is in scope for this zome.",
                        value
                    )))),
                }
            }
        }

        impl TryFrom<#ident> for ScopedEntryDefIndex {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                Self::try_from(&value)
            }
        }

        impl TryFrom<&&#ident> for ScopedEntryDefIndex {
            type Error = WasmError;

            fn try_from(value: &&#ident) -> Result<Self, Self::Error> {
                Self::try_from(*value)
            }
        }

        impl From<&#ident> for EntryVisibility {
            fn from(v: &#ident) -> Self {
                match v {
                    #into_visibility
                }
            }
        }

        impl From<&&#ident> for EntryVisibility {
            fn from(v: &&#ident) -> Self {
                Self::from(*v)
            }
        }

        impl EntryTypesHelper for #ident {
            type Error = WasmError;
            fn deserialize_from_type<Z, I>(
                zome_id: Z,
                entry_def_index: I,
                entry: &Entry,
            ) -> Result<Option<Self>, Self::Error>
            where
                Z: Into<ZomeId>,
                I: Into<EntryDefIndex>
            {
                let scoped_type = ScopedEntryDefIndex {
                    zome_id: zome_id.into(),
                    zome_type: entry_def_index.into(),
                };
                let entries = zome_info()?.zome_types.entries;
                match entries.find_key(scoped_type) {
                    Some(key) => {
                        match key {
                            #key_to_variant
                            _ => Ok(None),
                        }
                    }
                    None => if entries.dependencies().any(|z| z == scoped_type.zome_id) {
                        Err(wasm_error!(WasmErrorInner::Guest(format!(
                            "Entry type: {:?} is out of range for this zome.",
                            scoped_type
                        ))))
                    } else {
                        Ok(None)
                    }
                }
            }
        }

    };
    output.into()
}
